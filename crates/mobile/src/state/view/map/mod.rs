pub mod source;

use std::path::PathBuf;

use crate::{
    state::view::map::source::MapSource,
    tile_server::{TileServer, TileServerError},
};

const DEFAULT_STYLE: &str = include_str!("../../../../assets/libre-theme.json");

#[derive(uniffi::Object)]
pub struct MapState {
    style_json: String,
    #[allow(dead_code)]
    tile_server: TileServer,
}

impl MapState {
    pub async fn new(source: MapSource) -> Result<Self, TileServerError> {
        let tile_server = TileServer::start(PathBuf::from(source.pmtiles_path))?;
        let mask_geojson = std::fs::read_to_string(&source.bounds_path).map_err(|e| {
            std::io::Error::new(e.kind(), format!("{e} (file: {:?})", source.bounds_path))
        })?;
        let complexes_geojson = std::fs::read_to_string(&source.complexes_path).map_err(|e| {
            std::io::Error::new(e.kind(), format!("{e} (file: {:?})", source.complexes_path))
        })?;
        let style_json = build_style(
            DEFAULT_STYLE,
            tile_server.port(),
            &mask_geojson,
            &complexes_geojson,
        );

        Ok(Self {
            style_json,
            tile_server,
        })
    }
}

#[uniffi::export]
impl MapState {
    pub fn get_style(&self) -> String {
        self.style_json.clone()
    }
}

fn build_style(base_style: &str, port: u16, mask_geojson: &str, complexes_geojson: &str) -> String {
    let mut style: serde_json::Value = serde_json::from_str(base_style).unwrap();

    // Set initial map center to Central Park, NYC
    if let Some(obj) = style.as_object_mut() {
        obj.insert(
            "center".to_string(),
            serde_json::json!([-73.9805655, 40.7571418]),
        );
        obj.insert("zoom".to_string(), serde_json::json!(12));
    }

    if let Some(sources) = style.get_mut("sources").and_then(|s| s.as_object_mut()) {
        // Update only the local openmaptiles source, not the world one
        if let Some(source) = sources.get_mut("openmaptiles") {
            if let Some(obj) = source.as_object_mut() {
                obj.insert(
                    "url".to_string(),
                    serde_json::json!(format!("http://localhost:{port}/tiles.json")),
                );
            }
        }

        // Add the complexes source
        if let Some(complexes_data) = parse_mask_geojson(complexes_geojson) {
            sources.insert(
                "complexes".to_string(),
                serde_json::json!({
                    "type": "geojson",
                    "data": complexes_data
                }),
            );
        }

        // Add the play area mask source
        if let Some(playarea_geojson) = parse_mask_geojson(mask_geojson) {
            sources.insert(
                "playarea".to_string(),
                serde_json::json!({
                    "type": "geojson",
                    "data": playarea_geojson
                }),
            );
        }
    }

    // Parse the playarea geometry for use in "within" filters
    let playarea_geometry: Option<serde_json::Value> = extract_geometry_for_filter(mask_geojson);

    // Insert playarea-fill layer after world-water but before local water
    if let Some(layers) = style.get_mut("layers").and_then(|l| l.as_array_mut()) {
        // Find the index of world-water layer
        if let Some(idx) = layers
            .iter()
            .position(|l| l.get("id").and_then(|id| id.as_str()) == Some("world-water"))
        {
            // Insert playarea background right after world-water
            layers.insert(
                idx + 1,
                serde_json::json!({
                    "id": "playarea-background",
                    "type": "fill",
                    "source": "playarea",
                    "paint": {
                        "fill-color": "#faf7f8"
                    }
                }),
            );
        }

        // Add "within" filter to line layers using the local openmaptiles source
        // This clips them to only render within the playarea bounds
        // Note: "within" only supports Point/LineString, not Polygon geometries,
        // so we skip fill layers (water, buildings, etc.)
        if let Some(ref geometry) = playarea_geometry {
            for layer in layers.iter_mut() {
                let source = layer.get("source").and_then(|s| s.as_str());
                let layer_type = layer.get("type").and_then(|t| t.as_str());

                // Only apply "within" to line layers - fill layers have polygon geometries
                // which MapLibre's "within" expression doesn't support
                if source == Some("openmaptiles") && layer_type == Some("line") {
                    if let Some(obj) = layer.as_object_mut() {
                        let within_filter = serde_json::json!(["within", geometry]);

                        // Combine with existing filter if present
                        if let Some(existing_filter) = obj.get("filter").cloned() {
                            obj.insert(
                                "filter".to_string(),
                                serde_json::json!(["all", existing_filter, within_filter]),
                            );
                        } else {
                            obj.insert("filter".to_string(), within_filter);
                        }
                    }
                }
            }
        }
    }

    serde_json::to_string(&style).unwrap()
}

/// Parse the mask GeoJSON and convert to a serde_json::Value for use as a MapLibre source
fn parse_mask_geojson(geojson_str: &str) -> Option<serde_json::Value> {
    let geojson: geojson::GeoJson = geojson_str.parse().ok()?;
    Some(serde_json::to_value(&geojson).ok()?)
}

/// Extract just the geometry from a GeoJSON for use in "within" filter expressions
/// MapLibre's "within" expects a Geometry or Feature, not a FeatureCollection
fn extract_geometry_for_filter(geojson_str: &str) -> Option<serde_json::Value> {
    let geojson: geojson::GeoJson = geojson_str.parse().ok()?;

    match geojson {
        geojson::GeoJson::Geometry(geom) => serde_json::to_value(&geom).ok(),
        geojson::GeoJson::Feature(feat) => {
            // Return the whole feature - MapLibre accepts Feature objects in "within"
            serde_json::to_value(&feat).ok()
        }
        geojson::GeoJson::FeatureCollection(fc) => {
            // Extract the first feature from the collection
            fc.features
                .into_iter()
                .next()
                .and_then(|feat| serde_json::to_value(&feat).ok())
        }
    }
}
