use std::sync::RwLock;

const DEFAULT_STYLE: &str = include_str!("../assets/bright.json");

/// Map state with style information
#[derive(uniffi::Object)]
pub struct MapState {
    style_json: RwLock<String>,
}

#[uniffi::export]
impl MapState {
    pub fn get_style(&self) -> String {
        self.style_json.read().unwrap().clone()
    }
}

/// Main view state exposed to Kotlin
#[derive(uniffi::Object)]
pub struct ViewState {
    map: MapState,
}

#[uniffi::export]
impl ViewState {
    #[uniffi::constructor]
    pub fn new() -> Self {
        Self {
            map: MapState {
                style_json: RwLock::new(DEFAULT_STYLE.to_string()),
            },
        }
    }

    pub fn get_style(&self) -> String {
        self.map.get_style()
    }
}

uniffi::setup_scaffolding!();
