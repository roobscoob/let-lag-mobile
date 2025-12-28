use std::path::PathBuf;

use crate::tile_server::TileServer;

const DEFAULT_STYLE: &str = include_str!("../../../../assets/libre-theme.json");
const TILES_DIR: &str = "/path/to/tiles"; // Hardcoded for now

#[derive(uniffi::Object)]
pub struct MapState {
    style_json: String,
    #[allow(dead_code)]
    tile_server: TileServer,
}

impl MapState {
    pub async fn new() -> Self {
        let tile_server = TileServer::start(PathBuf::from(TILES_DIR)).unwrap();
        let style_json = rewrite_style_sources(DEFAULT_STYLE, tile_server.port());

        Self {
            style_json,
            tile_server,
        }
    }
}

#[uniffi::export]
impl MapState {
    pub fn get_style(&self) -> String {
        self.style_json.clone()
    }
}

fn rewrite_style_sources(base_style: &str, port: u16) -> String {
    let mut style: serde_json::Value = serde_json::from_str(base_style).unwrap();

    if let Some(sources) = style.get_mut("sources").and_then(|s| s.as_object_mut()) {
        for (_name, source) in sources.iter_mut() {
            if let Some(obj) = source.as_object_mut() {
                let source_type = obj.get("type").and_then(|t| t.as_str()).unwrap_or("");

                if source_type == "vector" {
                    obj.remove("url");
                    obj.insert(
                        "tiles".to_string(),
                        serde_json::json!([format!(
                            "http://localhost:{port}/tiles/{{z}}/{{x}}/{{y}}/tile.pbf"
                        )]),
                    );
                }
            }
        }
    }

    serde_json::to_string(&style).unwrap()
}
