pub mod gl {
    use std::{
        sync::LazyLock,
        thread::{self, ThreadId},
    };

    use khronos_egl::DynamicInstance;

    type EGLVersion = khronos_egl::EGL1_4;

    pub fn get_egl_instance() -> &'static DynamicInstance<EGLVersion> {
        static DYNAMIC: LazyLock<DynamicInstance<EGLVersion>> = LazyLock::new(|| unsafe {
            DynamicInstance::<EGLVersion>::load_required().expect("failed to obtain egl instance")
        });

        &DYNAMIC
    }

    pub fn get_gl_context() -> &'static glow::Context {
        static CONTEXT: LazyLock<glow::Context> = LazyLock::new(|| unsafe {
            glow::Context::from_loader_function(move |str| {
                get_egl_instance()
                    .get_proc_address(str)
                    .map(|x| x as *const _)
                    .unwrap_or_default()
            })
        });

        static CONTEXT_THREAD: LazyLock<ThreadId> = LazyLock::new(|| thread::current().id());

        if *CONTEXT_THREAD != thread::current().id() {
            panic!("accessed gl context on a different thread from normal")
        }

        &CONTEXT
    }

    pub trait GlResult<T> {
        fn wrap_gl(self) -> eyre::Result<T>;
    }

    impl<T> GlResult<T> for Result<T, String> {
        fn wrap_gl(self) -> eyre::Result<T> {
            self.map_err(|error| eyre::Error::msg(error))
        }
    }
}
