#[uniffi::export]
pub fn hello_from_rust() -> String {
    "Hello from Rust! 🦀".to_string()
}

uniffi::setup_scaffolding!();
