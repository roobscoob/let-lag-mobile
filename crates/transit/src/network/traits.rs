//! Pluggable networking traits.
//!
//! External crates implement these to provide data fetching capabilities.

use std::future::Future;
use std::pin::Pin;

use crate::models::types::Result;

/// Fetch raw bytes from a URL
pub trait DataFetcher: Send + Sync {
    fn fetch<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
}

/// Load bytes from local storage
pub trait StorageLoader: Send + Sync {
    fn load<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;

    fn save<'a>(
        &'a self,
        path: &'a str,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    fn exists<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;
}

/// Combined interface for managing bundles
pub trait BundleManager: Send + Sync {
    /// Download a bundle from remote server
    fn download_bundle<'a>(
        &'a self,
        network_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;

    /// Load a bundle from local storage
    fn load_bundle<'a>(
        &'a self,
        network_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;

    /// Save a bundle to local storage
    fn save_bundle<'a>(
        &'a self,
        network_id: &'a str,
        data: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

    /// Check if bundle exists locally
    fn has_bundle<'a>(
        &'a self,
        network_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + 'a>>;

    /// Fetch realtime updates (GTFS-RT)
    fn fetch_realtime<'a>(
        &'a self,
        network_id: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>>;
}
