use super::model::GameProfile;
use crate::{
    config::MojankConfiguration,
    error::{MojangRequestError, MojangRequestResult},
    utils::http_client::NmsrHttpClient,
};
use hyper::{body::Bytes, Method};
use std::sync::Arc;
use tracing::{instrument, Span};
use uuid::Uuid;

pub struct MojangClient {
    client: NmsrHttpClient,
    mojank_config: Arc<MojankConfiguration>,
}

#[test]
fn owo() {
    println!(env!("CARGO_PKG_AUTHORS"));
}

impl MojangClient {
    pub fn new(mojank: Arc<MojankConfiguration>) -> MojangRequestResult<Self> {
        Ok(Self {
            client: NmsrHttpClient::new(mojank.session_server_rate_limit),
            mojank_config: mojank,
        })
    }

    #[instrument(skip(self, parent_span, on_error), parent = parent_span)]
    pub(crate) async fn do_request(
        &self,
        url: &str,
        method: Method,
        parent_span: &Span,
        on_error: impl FnOnce() -> Option<MojangRequestError>,
    ) -> MojangRequestResult<Bytes> {
        self.client
            .do_request(url, method, parent_span, on_error)
            .await
    }

    pub async fn resolve_uuid_to_game_profile(
        &self,
        id: &Uuid,
    ) -> MojangRequestResult<GameProfile> {
        let url = format!(
            "{session_server}/session/minecraft/profile/{id}",
            session_server = self.mojank_config.session_server
        );

        let bytes = self
            .do_request(&url, Method::GET, &Span::current(), || {
                Some(MojangRequestError::GameProfileNotFound(id.to_owned()))
            })
            .await?;

        Ok(serde_json::from_slice(&bytes)?)
    }

    #[instrument(skip(self, parent_span), parent = parent_span)]
    pub async fn fetch_texture_from_mojang(
        &self,
        texture_id: &str,
        parent_span: &Span,
    ) -> MojangRequestResult<Vec<u8>> {
        let url = format!(
            "{textures_server}/texture/{texture_id}",
            textures_server = self.mojank_config.textures_server
        );

        let bytes = self
            .do_request(&url, Method::GET, &Span::current(), || {
                Some(MojangRequestError::InvalidTextureHashError(
                    texture_id.to_string(),
                ))
            })
            .await?;

        Ok(bytes.to_vec())
    }

    pub fn mojank_config(&self) -> &MojankConfiguration {
        self.mojank_config.as_ref()
    }
}
