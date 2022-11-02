use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct ServerConfiguration {
    /// The address to bind the server to.
    pub(crate) address: String,

    /// The port to bind the server to.
    pub(crate) port: u16,

    /// The path to the root directory of the parts folder.
    pub(crate) parts: String,

    /// Tls keys
    pub(crate) tls: Option<TlsConfiguration>,

    /// Cache configuration
    pub(crate) cache: CacheConfiguration,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct TlsConfiguration {
    pub(crate) private_key: PathBuf,
    pub(crate) certificate_chain: PathBuf,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct CacheConfiguration {
    /// How long, in seconds, it takes to expire the skins and renders cache (defaults to 1 day [86400 seconds])
    /// These images are cached based on the hash of the skin, so if the skin changes, the file will no longer be cached and will be re-rendered.
    pub(crate) image_cache_expiry: u64,

    /// How long, in seconds, it takes to expire the cache of the uuid to skin hash (defaults to 15 minutes [900 seconds])
    /// This cache is used to prevent the same uuid from being resolved multiple times in a short period of time.
    /// Setting this to a big value will reduce the amount of requests to the Mojang API, but will increase the time it takes to update the skin for a player.
    pub(crate) mojang_profile_request_expiry: u64,

    /// How long, in seconds, to run the cleanup task (defaults to 1 hour [3600 seconds])
    /// This task will remove any files in the cache that are older than the image cache expiry.
    /// This task will run on startup, and then every time the interval has passed.
    pub(crate) cleanup_interval: u64,
}

impl Default for ServerConfiguration {
    fn default() -> Self {
        ServerConfiguration {
            address: "0.0.0.0".to_string(),
            port: 8080,
            parts: "parts".to_string(),
            tls: None,
            cache: CacheConfiguration::default(),
        }
    }
}

impl Default for CacheConfiguration {
    fn default() -> Self {
        CacheConfiguration {
            image_cache_expiry: 86400,
            mojang_profile_request_expiry: 900,
            cleanup_interval: 3600,
        }
    }
}
