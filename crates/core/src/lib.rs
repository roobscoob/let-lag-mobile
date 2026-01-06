use crate::hide_and_seek::HideAndSeekGame;

pub mod hide_and_seek;
pub mod map;
pub mod resource;
pub mod shape;

// Re-export transit from the transit crate
pub use jet_lag_transit as transit;

pub enum Game {
    HideAndSeek(HideAndSeekGame),
}
