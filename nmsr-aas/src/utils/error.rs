use thiserror::Error;

#[derive(Error, Debug)]
pub enum NMSRaaSError {
    #[error("Invalid player request: {0}")]
    RenderRequestError(#[from] RenderRequestError),
    #[error("Model cache error: {0}")]
    ModelCacheError(#[from] ModelCacheError),
    #[error("Mojang request error: {0}")]
    MojangRequestError(#[from] MojangRequestError),
}

#[derive(Error, Debug)]
pub enum RenderRequestError {
    #[error("Invalid UUID: {0}")]
    InvalidUUID(#[from] uuid::Error),
    #[error("The UUID you requested ({0}) has version {1} instead of version 4. Version 4 UUIDs are required for online player skins.")]
    InvalidPlayerUuidRequest(String, usize),
    #[error("Invalid player request: {0}")]
    InvalidPlayerRequest(String),
    #[error("Io error: {0}")]
    ExplainedIoError(std::io::Error, String),
}

#[derive(Error, Debug)]
pub enum ModelCacheError {
    #[error("Unable to read marker for entry {0:?}")]
    MarkerMetadataError(crate::model::request::entry::RenderRequestEntry),
    #[error("Invalid player request attempt: {0}")]
    InvalidRequestCacheAttempt(String),
    #[error("Invalid cache entry marker request: {0}")]
    InvalidCacheEntryMarkerRequest(String),
    #[error("Invalid cache bias configuration: {0}")]
    InvalidCacheBiasConfiguration(String),
}

#[derive(Error, Debug)]
pub enum MojangRequestError {
    #[error("Unable to decode game profile from base64: {0}")]
    Base64Error(#[from] base64::DecodeError),
    #[error("Unable to decode game profile from utf8: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
    #[error("Unable to decode game profile from json: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("Game profile is missing the textures property")]
    MissingTexturesProperty,
    #[error("Game profile has an invalid textures property: {0}")]
    InvalidTexturesProperty(serde_json::Error),
    #[error("Url parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Request error: {0}")]
    RequestError(#[from] reqwest::Error),
}

pub(crate) type Result<T> = std::result::Result<T, NMSRaaSError>;
pub(crate) type RenderRequestResult<T> = std::result::Result<T, RenderRequestError>;
pub(crate) type ModelCacheResult<T> = std::result::Result<T, ModelCacheError>;
pub(crate) type MojangRequestResult<T> = std::result::Result<T, MojangRequestError>;

pub trait ExplainableExt<T> {
    fn explain(self, message: String) -> Result<T>;
}

impl<T> ExplainableExt<T> for std::result::Result<T, std::io::Error> {
    fn explain(self, message: String) -> Result<T> {
        self.map_err(|e| RenderRequestError::ExplainedIoError(e, message).into())
    }
}
