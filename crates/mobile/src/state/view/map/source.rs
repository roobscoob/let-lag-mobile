use std::ffi::OsString;
use std::path::PathBuf;

pub struct MapSource {
    pub(crate) pmtiles_path: OsString,
    pub(crate) bounds_path: OsString,
    pub(crate) complexes_path: OsString,
}

impl MapSource {
    pub(crate) fn nyc(base_path: String) -> Self {
        let base = PathBuf::from(&base_path);
        MapSource {
            pmtiles_path: base.join("nyc_tiles.pmtiles").into_os_string(),
            bounds_path: base.join("nyc_bounds.geojson").into_os_string(),
            complexes_path: base.join("complexes.geojson").into_os_string(),
        }
    }
}
