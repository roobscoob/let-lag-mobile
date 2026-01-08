use core::{iter::{self, Iterator}, mem::size_of, num::NonZero, option::Option::None};
use std::{
    collections::BTreeMap,
    default::Default,
    future::pending,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::Poll,
    time::Duration,
};

use actix::{
    Actor, ActorFuture, Addr, AsyncContext, Context, Handler, Message, WrapFuture,
    fut::LocalBoxActorFuture,
};
use ash::vk::{self, ExternalMemoryHandleTypeFlags, StructureType};
use jet_lag_core::{
    map::tile::Tile,
    shape::{
        Shape,
        compiled::{
            CompiledShape,
            shader::{TileBounds, cache::ShaderCache},
        },
        instruction::SdfInstruction,
    },
};
use ndk::{
    hardware_buffer::{HardwareBuffer, HardwareBufferDesc, HardwareBufferRef, HardwareBufferUsage},
    hardware_buffer_format::HardwareBufferFormat,
};
use replace_with::replace_with_or_abort;
use tokio::sync::oneshot;
use wgpu::{
    Backends, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BufferBinding, BufferUsages, ColorTargetState, ColorWrites, CommandEncoder,
    CommandEncoderDescriptor, Extent3d, FragmentState, Instance, LoadOpDontCare, MultisampleState,
    Operations, PipelineCompilationOptions, PipelineLayout, PipelineLayoutDescriptor, PollType,
    PrimitiveState, PrimitiveTopology, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderStages,
    Texture, TextureDescriptor, TextureFormat, TextureUsages, TextureUses, TextureViewDescriptor,
    VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode, naga,
    util::{BufferInitDescriptor, DeviceExt},
    wgt::ExternalTextureDescriptor,
};
use wgpu_hal::{Attachment, AttachmentOps, ColorAttachment, MemoryFlags, vulkan::TextureMemory};
use zerocopy::IntoBytes;

pub fn start_render_thread() -> Addr<RenderThread> {
    let (sender, receiver) = oneshot::channel();

    #[cfg(target_os = "android")]
    let context = { None };
    #[cfg(target_os = "ios")]
    let context = None;

    std::thread::spawn(move || {
        actix::run(async move {
            let thread = RenderThread::new(context).await;
            sender
                .send(thread.start())
                .expect("failed to send addr back");
            pending().await
        })
    });

    receiver
        .blocking_recv()
        .expect("failed to receive address from thread")
}

#[cfg(target_os = "android")]
unsafe fn find_memory_type_index(
    memory_requirements: &vk::MemoryRequirements,
    memory_properties: &vk::PhysicalDeviceMemoryProperties,
    required_properties: vk::MemoryPropertyFlags,
) -> Option<u32> {
    for i in 0..memory_properties.memory_type_count {
        let memory_type = &memory_properties.memory_types[i as usize];

        // Check if this memory type is allowed by the memory requirements
        let type_supported = (memory_requirements.memory_type_bits & (1 << i)) != 0;

        // Check if this memory type has the required properties
        let properties_match = memory_type.property_flags.contains(required_properties);

        if type_supported && properties_match {
            return Some(i);
        }
    }
    None
}

pub struct RenderThread {
    instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    vertex_shader: ShaderModule,

    shader_cache: ShaderCache,
    shapes: Vec<ShapeObj>,
}

impl RenderThread {
    async fn new(context: Option<usize>) -> Self {
        #[cfg(target_os = "android")]
        let (instance, (device, queue), adapter) = {
            use wgpu::{
                DeviceDescriptor, FeaturesWGPU, FeaturesWebGPU, Limits, RequestAdapterOptions,
            };

            let instance = Instance::new(&wgpu::InstanceDescriptor {
                backends: Backends::VULKAN,
                ..Default::default()
            });

            let adapter = instance
                .request_adapter(&RequestAdapterOptions {
                    ..Default::default()
                })
                .await
                .unwrap();

            (
                instance,
                adapter
                    .request_device(&DeviceDescriptor {
                        required_features: wgpu::Features {
                            features_wgpu: FeaturesWGPU::empty(),
                            features_webgpu: FeaturesWebGPU::empty(),
                        },
                        required_limits: Limits {
                            max_storage_buffers_per_shader_stage: 4,
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .await
                    .expect("failed to obtain device"),
                adapter,
            )
        };

        let vertex_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("./vertex.wgsl").into()),
        });

        Self {
            instance,
            adapter,
            device,
            queue,
            vertex_shader,

            shader_cache: ShaderCache::new(),
            shapes: Vec::new(),
        }
    }
}

impl Actor for RenderThread {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(Duration::from_millis(15), |actor, _| {
            let _ = actor.device.poll(PollType::Poll);
        });
    }
}

struct ShapeObj {
    compiled_shape: CompiledShape,
    bind_group_layout: BindGroupLayout,
    // no_ellipsoid_low_prec: RenderPipeline,
    // no_ellipsoid_high_prec: RenderPipeline,
    use_ellipsoid_low_prec: RenderPipeline,
    // use_ellipsoid_high_prec: RenderPipeline,
}

#[derive(Message)]
#[rtype(result = "u64")]
pub struct StartShapeCompilation {
    pub shape: Box<dyn Shape>,
}

impl Handler<StartShapeCompilation> for RenderThread {
    type Result = u64;

    fn handle(&mut self, msg: StartShapeCompilation, _ctx: &mut Self::Context) -> Self::Result {
        let shape = CompiledShape::compile(&self.device, &mut self.shader_cache, &*msg.shape);
        let bind_group_layout = self
            .device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 2,
                        visibility: ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: None,
                immediate_size: 0,
                bind_group_layouts: &[&bind_group_layout],
            });

        let create_render_pipeline = |ellipsoid: bool, high_prec: bool| {
            self.device
                .create_render_pipeline(&RenderPipelineDescriptor {
                    label: None,
                    layout: Some(&pipeline_layout),
                    vertex: VertexState {
                        buffers: &[],
                        compilation_options: PipelineCompilationOptions::default(),
                        entry_point: Some("vtx_main"),
                        module: &self.vertex_shader,
                    },
                    primitive: PrimitiveState {
                        topology: PrimitiveTopology::TriangleStrip,
                        strip_index_format: None,
                        front_face: wgpu::FrontFace::Cw,
                        cull_mode: None, // todo: face culling
                        unclipped_depth: false,
                        polygon_mode: wgpu::PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: None,
                    multisample: MultisampleState::default(),
                    fragment: Some(FragmentState {
                        module: shape.shader(),
                        entry_point: Some("main"),
                        compilation_options: PipelineCompilationOptions {
                            constants: &[
                                ("USE_ELLIPSOID", if ellipsoid { 1.0 } else { 0.0 }),
                                ("USE_HIGH_PRECISION", if high_prec { 1.0 } else { 0.0 }),
                            ],
                            zero_initialize_workgroup_memory: false,
                        },
                        targets: &[Some(ColorTargetState {
                            format: TextureFormat::Rgba8Unorm,
                            blend: None,
                            write_mask: ColorWrites::RED,
                        })],
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        };

        let id = shape.id();
        self.shapes.push(ShapeObj {
            // no_ellipsoid_low_prec: create_render_pipeline(false, false),
            // no_ellipsoid_high_prec: create_render_pipeline(false, true),
            use_ellipsoid_low_prec: create_render_pipeline(true, false),
            // use_ellipsoid_high_prec: create_render_pipeline(true, true),
            bind_group_layout,
            compiled_shape: shape,
        });

        id
    }
}

pub struct WrapBufferRef(pub HardwareBufferRef);
unsafe impl Send for WrapBufferRef {}

type RequestReturnType = WrapBufferRef;
#[derive(Message)]
#[rtype(result = "RequestReturnType")]
pub struct RequestTile {
    pub tile: Tile,
}

impl Handler<RequestTile> for RenderThread {
    type Result = LocalBoxActorFuture<Self, RequestReturnType>;

    fn handle(&mut self, msg: RequestTile, _ctx: &mut Self::Context) -> Self::Result {
        let hardware_buffer: HardwareBufferRef =
            ndk::hardware_buffer::HardwareBuffer::allocate(HardwareBufferDesc {
                width: 256,
                height: 256,
                layers: 1,
                stride: 2,
                usage: HardwareBufferUsage::GPU_FRAMEBUFFER
                    | HardwareBufferUsage::GPU_SAMPLED_IMAGE,
                format: HardwareBufferFormat::R8G8B8A8_UNORM,
            })
            .unwrap();

        let ext_info = vk::ExternalMemoryImageCreateInfo {
            handle_types: ExternalMemoryHandleTypeFlags::ANDROID_HARDWARE_BUFFER_ANDROID,
            ..Default::default()
        };

        let image_create_info = vk::ImageCreateInfo {
            p_next: &ext_info as *const _ as *const _,
            extent: vk::Extent3D {
                width: 256,
                height: 256,
                depth: 1,
            },
            mip_levels: 1,
            array_layers: 1,
            image_type: vk::ImageType::TYPE_2D,
            format: vk::Format::R16_SINT,
            tiling: vk::ImageTiling::OPTIMAL,
            initial_layout: vk::ImageLayout::UNDEFINED,
            usage: vk::ImageUsageFlags::COLOR_ATTACHMENT
                | vk::ImageUsageFlags::TRANSFER_SRC
                | vk::ImageUsageFlags::TRANSFER_DST,
            samples: vk::SampleCountFlags::TYPE_1,
            sharing_mode: vk::SharingMode::EXCLUSIVE,
            flags: vk::ImageCreateFlags::MUTABLE_FORMAT,
            ..Default::default()
        };

        let hal_device = unsafe { self.device.as_hal::<wgpu_hal::api::Vulkan>().unwrap() };
        let device = hal_device.raw_device();

        let texture = unsafe {
            let image = device
                .create_image(&image_create_info, None)
                .expect("failed to create image");
            let import_memory_info = vk::ImportAndroidHardwareBufferInfoANDROID {
                buffer: hardware_buffer.as_ptr() as _,
                ..Default::default()
            };
            let memory_requirements = device.get_image_memory_requirements(image);

            let memory_allocate_info = {
                let adapter = self.adapter.as_hal::<wgpu_hal::api::Vulkan>().unwrap();
                let phy = adapter.raw_physical_device();
                let memory_properties = adapter
                    .shared_instance()
                    .raw_instance()
                    .get_physical_device_memory_properties(phy);
                let memory_type_index = find_memory_type_index(
                    &memory_requirements,
                    &memory_properties,
                    vk::MemoryPropertyFlags::DEVICE_LOCAL,
                )
                .unwrap();
                vk::MemoryAllocateInfo {
                    p_next: &import_memory_info as *const _ as *const _,
                    allocation_size: memory_requirements.size,
                    memory_type_index,
                    ..Default::default()
                }
            };

            let device_memory = device.allocate_memory(&memory_allocate_info, None).unwrap();

            device.bind_image_memory(image, device_memory, 0).unwrap();

            let desc = wgpu_hal::TextureDescriptor {
                label: None,
                usage: TextureUses::COLOR_TARGET | TextureUses::COPY_SRC | TextureUses::COPY_DST,
                size: Extent3d {
                    width: 256,
                    height: 256,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                memory_flags: MemoryFlags::empty(),
                format: TextureFormat::Rgba8Unorm,
                view_formats: vec![TextureFormat::Rgba8Unorm],
            };
            let texture = hal_device.texture_from_raw(
                image,
                &desc,
                None,
                TextureMemory::Dedicated(device_memory),
            );

            self.device
                .create_texture_from_hal::<wgpu_hal::api::Vulkan>(
                    texture,
                    &TextureDescriptor {
                        label: None,
                        size: desc.size,
                        mip_level_count: desc.mip_level_count,
                        sample_count: desc.sample_count,
                        dimension: desc.dimension,
                        format: desc.format,
                        usage: TextureUsages::RENDER_ATTACHMENT
                            | TextureUsages::COPY_SRC
                            | TextureUsages::COPY_DST,
                        view_formats: &desc.view_formats,
                    },
                )
        };

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());

        for shape in &self.shapes {
            let mut contents: Vec<u8> = vec![];
            let arguments = shape
                .compiled_shape
                .fill_arguments(&mut contents, &msg.tile);

            let alignment = self.device.limits().min_storage_buffer_offset_alignment as usize;
            contents.extend(iter::repeat(0).take(alignment - (contents.len() % alignment)));
            let arguments_offset: u64 = contents.len() as u64;
            contents.extend(arguments.as_bytes());

            let storage_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: &contents,
                usage: BufferUsages::STORAGE,
            });
            let uniform_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                label: None,
                contents: msg.tile.into_bounds().as_bytes(),
                usage: BufferUsages::UNIFORM,
            });
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &shape.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: &storage_buffer,
                            offset: 0,
                            size: NonZero::new(arguments_offset),
                        }),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: &storage_buffer,
                            offset: arguments_offset as u64,
                            size: NonZero::new(arguments.as_bytes().len() as u64),
                        }),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Buffer(BufferBinding {
                            buffer: &uniform_buffer,
                            offset: 0,
                            size: NonZero::new(size_of::<TileBounds>() as u64),
                        }),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[Some(RenderPassColorAttachment {
                    ops: Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    view: &texture.create_view(&TextureViewDescriptor {
                        ..Default::default()
                    }),
                    depth_slice: None,
                    resolve_target: None,
                })],
                label: None,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&shape.use_ellipsoid_low_prec);
            pass.set_bind_group(0, Some(&bind_group), &[]);
            pass.draw(0..4, 0..1);
        }

        enum State {
            ToSubmit(CommandEncoder),
            Waiting(Arc<AtomicBool>),
        }
        #[pin_project::pin_project]
        struct Impl {
            return_value: Option<RequestReturnType>,
            state: State,
        }
        impl ActorFuture<RenderThread> for Impl {
            type Output = RequestReturnType;

            fn poll(
                self: Pin<&mut Self>,
                thread: &mut RenderThread,
                _ctx: &mut <RenderThread as Actor>::Context,
                task: &mut std::task::Context,
            ) -> Poll<Self::Output> {
                let this = self.project();
                match this.state {
                    State::ToSubmit(_) => {
                        let done = Arc::new(AtomicBool::new(false));
                        let mut encoder_val = None;
                        replace_with_or_abort(this.state, |state| {
                            let State::ToSubmit(encoder) = state else {
                                unreachable!()
                            };

                            encoder_val = Some(encoder);
                            State::Waiting(done.clone())
                        });
                        let encoder = encoder_val.unwrap();
                        let waker = task.waker().clone();
                        encoder.on_submitted_work_done(move || {
                            done.store(true, Ordering::Release);
                            waker.wake();
                        });
                        let command_buffer = encoder.finish();
                        thread.queue.submit([command_buffer]);
                        Poll::Pending
                    }
                    State::Waiting(done) => {
                        if let Some(return_value) =
                            this.return_value.take_if(|_| done.load(Ordering::Acquire))
                        {
                            Poll::Ready(return_value)
                        } else {
                            Poll::Pending
                        }
                    }
                }
            }
        }

        Box::pin(Impl {
            state: State::ToSubmit(encoder),
            return_value: Some(WrapBufferRef(hardware_buffer)),
        })
    }
}
