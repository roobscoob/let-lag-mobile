#[cfg(target_os = "android")]
mod android;
pub mod layers;
pub mod render;
pub mod state;
pub(crate) mod tile_server;

uniffi::setup_scaffolding!();

/// Initialize the library with proper panic handling
/// Call this once at startup from Kotlin/Swift
#[uniffi::export]
pub fn init_panic_handler() {
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        eprintln!("=== RUST PANIC ===");
        eprintln!("{panic_info}");
        eprintln!("Backtrace:\n{backtrace}");
        eprintln!("=== END PANIC ===");
    }));
}
