use std::sync::Arc;

use tokio::sync::RwLock;

use crate::state::view::map::{MapState, source::MapSource};

pub mod map;

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum MapError {
    #[error("{0}")]
    TileServer(String),
}

#[derive(uniffi::Object)]
pub struct ViewState {
    base_path: String,
    map: RwLock<Option<Arc<MapState>>>,
}

#[uniffi::export]
impl ViewState {
    #[uniffi::constructor]
    pub fn new(base_path: String) -> Self {
        Self {
            base_path,
            map: RwLock::new(None),
        }
    }

    pub async fn get_map_state(&self) -> Result<Arc<MapState>, MapError> {
        if let Some(ref map) = *(self.map.read().await) {
            return Ok(Arc::clone(map));
        }

        let mut guard = self.map.write().await;
        let new_map = Arc::new(
            MapState::new(MapSource::nyc(self.base_path.clone()))
                .await
                .map_err(|e| MapError::TileServer(e.to_string()))?,
        );
        *guard = Some(Arc::clone(&new_map));

        Ok(new_map)
    }
}
