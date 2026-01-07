use std::{
    collections::BTreeMap,
    default::Default,
    future::pending,
    option::Option::None,
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
use jet_lag_core::shape::{compiler::Register, instruction::SdfInstruction};
use replace_with::replace_with_or_abort;
use tokio::sync::oneshot;
use wgpu::{
    Backends, ColorTargetState, ColorWrites, CommandEncoder, CommandEncoderDescriptor, Extent3d,
    FragmentState, Instance, LoadOpDontCare, MultisampleState, Operations,
    PipelineCompilationOptions, PollType, PrimitiveState, PrimitiveTopology,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor,
    ShaderModule, ShaderModuleDescriptor, Texture, TextureDescriptor, TextureFormat, TextureUsages,
    TextureViewDescriptor, VertexAttribute, VertexBufferLayout, VertexState, VertexStepMode,
};
use wgpu_hal::{Attachment, AttachmentOps, ColorAttachment};
use zerocopy::IntoBytes;

pub fn start_render_thread() -> Addr<RenderThread> {
    let (sender, receiver) = oneshot::channel();

    #[cfg(target_os = "android")]
    let context = {
        let instance = crate::android::gl::get_egl_instance();
        Some(
            instance
                .get_current_context()
                .expect("no egl context was available")
                .as_ptr()
                .expose_provenance(),
        )
    };
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

pub struct RenderThread {
    device: wgpu::Device,
    queue: wgpu::Queue,
    vertex_shader: ShaderModule,

    shapes: BTreeMap<Register, Shape>,
}

impl RenderThread {
    async fn new(context: Option<usize>) -> Self {
        #[cfg(target_os = "android")]
        let (device, queue) = {
            use crate::android::gl::get_egl_instance;
            use khronos_egl as egl;

            let egl = get_egl_instance();

            unsafe {
                use std::{ffi, ptr};

                use wgpu::{GlBackendOptions, wgt::DeviceDescriptor};

                let display = egl
                    .get_display(egl::DEFAULT_DISPLAY)
                    .expect("failed to get default display");

                let context = egl
                    .create_context(
                        display,
                        egl::Config::from_ptr(ptr::null_mut()),
                        Some(egl::Context::from_ptr(
                            context.expect("no egl context was sent") as _,
                        )),
                        &[],
                    )
                    .expect("failed to create context");

                egl.make_current(display, None, None, Some(context))
                    .expect("failed to set current context");

                let adapter = wgpu_hal::gles::Adapter::new_external(
                    |proc| {
                        egl.get_proc_address(proc)
                            .map(|func| func as *mut ffi::c_void)
                            .unwrap_or_default() as _
                    },
                    GlBackendOptions::default(),
                )
                .expect("failed to create adapter");

                let instance = Instance::new(&wgpu::InstanceDescriptor {
                    backends: Backends::GL,
                    ..Default::default()
                });
                let adapter = instance.create_adapter_from_hal(adapter);
                adapter
                    .request_device(&DeviceDescriptor::default())
                    .await
                    .expect("failed to obtain device")
            }
        };

        let vertex_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(include_str!("./vertex.wgsl").into()),
        });

        Self {
            device,
            queue,
            vertex_shader,
            shapes: BTreeMap::new(),
        }
    }
}

impl Actor for RenderThread {
    type Context = Context<Self>;
}

struct Shape {
    register: Register,
    needs_iterations: bool,
    render_pipeline: RenderPipeline,
}

#[derive(Message)]
#[rtype(result = "()")]
pub struct StartShapeCompilation {
    instruction: Vec<SdfInstruction>,
    register: Register,
}

impl Handler<StartShapeCompilation> for RenderThread {
    type Result = ();

    fn handle(&mut self, msg: StartShapeCompilation, ctx: &mut Self::Context) -> Self::Result {
        ctx.run_interval(Duration::from_millis(15), |actor, _| {
            let _ = actor.device.poll(PollType::Poll);
        });
        let l = self.device.create_shader_module(ShaderModuleDescriptor {
            source: wgpu::ShaderSource::Wgsl(include_str!("./fragment.wgsl").into()),
            label: None,
        });

        let render_pipeline = self
            .device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: None,
                layout: None,
                vertex: VertexState {
                    buffers: &[VertexBufferLayout {
                        array_stride: size_of::<f32>() as u64 * 2,
                        attributes: &[VertexAttribute {
                            offset: 0,
                            format: wgpu::VertexFormat::Float32,
                            shader_location: 0,
                        }],
                        step_mode: VertexStepMode::Vertex,
                    }],
                    compilation_options: PipelineCompilationOptions::default(),
                    entry_point: Some("vertex"),
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
                    module: &l,
                    entry_point: Some("fragment"),
                    compilation_options: PipelineCompilationOptions::default(),
                    targets: &[Some(ColorTargetState {
                        format: TextureFormat::R32Uint,
                        blend: None,
                        write_mask: ColorWrites::RED,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        self.shapes.insert(
            msg.register,
            Shape {
                register: msg.register,
                needs_iterations: false,
                render_pipeline,
            },
        );
    }
}

#[derive(Message)]
#[rtype(result = "wgpu::Texture")]
pub struct RequestTile {
    x: u32,
    y: u32,
    z: u32,
    shape: Register,
}

impl Handler<RequestTile> for RenderThread {
    type Result = LocalBoxActorFuture<Self, wgpu::Texture>;

    fn handle(&mut self, msg: RequestTile, ctx: &mut Self::Context) -> Self::Result {
        let shape: &Shape = self.shapes.get(&msg.shape).expect("shape wasn't compiled");
        let texture = self.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::R32Uint,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[TextureFormat::R32Uint],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor::default());
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &[Some(RenderPassColorAttachment {
                ops: Operations {
                    load: wgpu::LoadOp::DontCare(unsafe { LoadOpDontCare::enabled() }),
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

        pass.set_pipeline(&shape.render_pipeline);
        pass.draw(0..4, 0..1);

        enum State {
            ToSubmit(CommandEncoder),
            Waiting(Arc<AtomicBool>),
        }
        #[pin_project::pin_project]
        struct Impl {
            texture: Option<Texture>,
            state: State,
        }
        impl ActorFuture<RenderThread> for Impl {
            type Output = Texture;

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
                        if let Some(texture) =
                            this.texture.take_if(|_| done.load(Ordering::Acquire))
                        {
                            Poll::Ready(texture)
                        } else {
                            Poll::Pending
                        }
                    }
                }
            }
        }

        drop(pass);
        Box::pin(Impl {
            state: State::ToSubmit(encoder),
            texture: Some(texture),
        })
    }
}
