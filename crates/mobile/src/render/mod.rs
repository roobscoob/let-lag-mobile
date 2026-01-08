use actix::Addr;
use geo::Point;
use jet_lag_core::shape::{
    Shape,
    compiler::{Register, SdfCompiler},
};

use crate::render::{
    style::Style,
    thread::{RenderThread, StartShapeCompilation, start_render_thread},
};

pub mod style;
pub mod thread;

pub struct RenderHandle {
    id: u64,
    style: Style,
}

pub struct RenderSession {
    pub render_thread: Addr<RenderThread>,
    

}

impl RenderSession {
    pub fn new() -> Self {
        let render_thread = start_render_thread();
        let compiler = SdfCompiler::new();
        Self {
            render_thread,
        }
    }

    // pub fn test(&mut self) -> Register {
    //     // let register = self.compiler.point(Point::new(0.0, 0.0));

    //     // register
    // }

    pub async fn append_shape(&mut self, shape: Box<dyn Shape>, style: Style) -> RenderHandle {
        let id = self.render_thread.send(StartShapeCompilation { shape }).await.expect("render thread shut down unexpectedly");

        RenderHandle {
            id,
            style,
        }
    }
}

impl RenderHandle {
    pub fn update_style(&mut self, style: Style) {
        todo!()
    }

    pub fn remove(self) {
        todo!()
    }
}
