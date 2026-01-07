mod oob {
    #[cfg(target_os = "android")]
    pub mod android;
}
pub mod test_square {
    // #[cfg(target_os = "android")]
    pub mod android;
    pub mod traverse_quadtree;
}

mod android {
    use std::{panic::PanicHookInfo, ptr, sync::Once};

    use glam::DMat4;
    use tracing::Level;
    use tracing_logcat::{LogcatMakeWriter, LogcatTag};
    use tracing_subscriber::fmt::format::Format;

    use crate::layers::{oob::android::OutOfBoundsLayer, test_square::android::TestSquare};

    fn setup_logging() {
        static LOGGING_SETUP: Once = Once::new();

        LOGGING_SETUP.call_once(|| {
            let tag = LogcatTag::Fixed("JetLag-Rust".to_owned());
            let writer = LogcatMakeWriter::new(tag).expect("Failed to initialize logcat writer");
            tracing_subscriber::fmt()
                .event_format(Format::default().with_level(false).without_time())
                .with_writer(writer)
                .with_ansi(false)
                .with_max_level(Level::TRACE)
                .init();

            std::panic::set_hook(Box::new(panic_hook));
        })
    }

    fn panic_hook(info: &PanicHookInfo) {
        tracing::error!("{info}")
    }

    #[derive(Debug)]
    #[repr(C)]
    pub struct Parameters {
        pub width: f64,
        pub height: f64,
        pub latitude: f64,
        pub longitude: f64,
        pub zoom: f64,
        pub bearing: f64,
        pub pitch: f64,
        pub field_of_view: f64,
        pub projection_matrix: DMat4,
    }

    pub trait CustomLayer: Sized {
        fn new() -> eyre::Result<Self>;
        fn render(&mut self, parameters: &Parameters) -> eyre::Result<()>;
        fn context_lost(&mut self);
        fn cleanup(self);
    }

    #[repr(C)]
    struct CustomLayerVTable {
        pub initialize: extern "C" fn(*mut CustomLayerVTable),
        pub render: extern "C" fn(*mut CustomLayerVTable, *const Parameters),
        pub context_lost: extern "C" fn(*mut CustomLayerVTable),
        pub deinitialize: extern "C" fn(*mut CustomLayerVTable),
        pub boxed_value: *mut (),
    }

    extern "C" fn initialize<T: CustomLayer>(vtable: *mut CustomLayerVTable) {
        (unsafe { &mut *vtable }).boxed_value =
            Box::into_raw(Box::new(T::new().expect("failed to construct type"))).cast()
    }

    extern "C" fn render<T: CustomLayer>(
        vtable: *mut CustomLayerVTable,
        parameters: *const Parameters,
    ) {
        let value = unsafe { &mut *(*vtable).boxed_value.cast::<T>() };
        value
            .render(unsafe { &*parameters })
            .expect("failed to render a frame")
    }

    extern "C" fn context_lost<T: CustomLayer>(vtable: *mut CustomLayerVTable) {
        let value = unsafe { &mut *(*vtable).boxed_value.cast::<T>() };
        value.context_lost();
    }

    extern "C" fn deinitialize<T: CustomLayer>(vtable: *mut CustomLayerVTable) {
        let value = unsafe { Box::from_raw((*vtable).boxed_value.cast::<T>()) };
        value.cleanup();
    }

    const fn custom<T: CustomLayer>() -> CustomLayerVTable {
        CustomLayerVTable {
            initialize: initialize::<T>,
            render: render::<T>,
            context_lost: context_lost::<T>,
            deinitialize: deinitialize::<T>,
            boxed_value: ptr::null_mut(),
        }
    }

    // to allow it to remain in a static
    unsafe impl Sync for CustomLayerVTable {}

    static OUT_OF_BOUNDS_LAYER: CustomLayerVTable = custom::<OutOfBoundsLayer>();
    static TEST_SQUARE_LAYER: CustomLayerVTable = custom::<TestSquare>();

    #[unsafe(export_name = "fetchCustomLayerVtable")]
    extern "C" fn fetch_custom_layer_vtable(kind: u32) -> *const CustomLayerVTable {
        setup_logging();
        tracing::info!("fetching custom layer vtable: {kind}");
        match kind {
            0 => &raw const OUT_OF_BOUNDS_LAYER,
            1 => &raw const TEST_SQUARE_LAYER,
            _ => {
                panic!("picked an invalid layer")
            }
        }
    }
}
