pub mod tile;

use std::sync::Arc;

use crate::resource::bundle::ResourceBundle;
use jet_lag_transit::TransitProvider;

pub struct Map {
    id: Arc<str>,
    name: Arc<str>,
    geography: ResourceBundle,
    transit: Arc<dyn TransitProvider>,
}
