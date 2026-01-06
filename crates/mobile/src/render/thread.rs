use std::{collections::BTreeMap, future::pending};

use actix::{Actor, Addr, Context, Handler, Message, WrapFuture, fut::LocalBoxActorFuture};
use jet_lag_core::shape::{compiler::Register, instruction::SdfInstruction};
use tokio::sync::oneshot;
use wgpu::{Backends, Instance, InstanceFlags, ShaderModule, ShaderModuleDescriptor};

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

        Self {
            device,
            queue,
            shapes: BTreeMap::new(),
        }
    }
}

impl Actor for RenderThread {
    type Context = Context<Self>;
}

struct Shape {
    register: Register,
    shader_module: ShaderModule,
}

#[derive(Message)]
#[rtype(result = "usize")]
pub struct StartShapeCompilation {
    instruction: Vec<SdfInstruction>,
    register: Register,
}

impl Handler<StartShapeCompilation> for RenderThread {
    type Result = LocalBoxActorFuture<RenderThread, usize>;

    fn handle(&mut self, msg: StartShapeCompilation, ctx: &mut Self::Context) -> Self::Result {
        // let l = self.device.create_shader_module(&ShaderModuleDescriptor { source: wgpu::ShaderSource::Wgsl("".into()), label: "" });
        todo!()
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
        todo!()
    }
}
