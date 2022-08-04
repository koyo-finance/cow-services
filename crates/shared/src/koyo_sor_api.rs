//! Module for interacting with the Koyo SOR HTTP API.

use crate::balancer_sor_api::{Query, Quote};
use anyhow::{ensure, Result};
use reqwest::{Client, IntoUrl, Url};

/// Trait for mockable Koyo SOR API.
#[mockall::automock]
#[async_trait::async_trait]
pub trait KoyoSorApi: Send + Sync + 'static {
    /// Quotes a price.
    async fn quote(&self, query: Query) -> Result<Option<Quote>>;
}

/// Koyo SOR API.
pub struct DefaultKoyoSorApi {
    client: Client,
    url: Url,
}

impl DefaultKoyoSorApi {
    /// Creates a new Koyo SOR API instance.
    pub fn new(
        client: Client,
        base_url: impl IntoUrl,
        chain_id: u64,
        supported_chain_ids: Option<&Vec<u64>>,
    ) -> Result<Self> {
        ensure!(
            supported_chain_ids
                .unwrap_or(&[288].to_vec())
                .contains(&chain_id),
            "Koyo SOR API is not supported on this chain",
        );

        let url = base_url.into_url()?.join(&chain_id.to_string())?;
        Ok(Self { client, url })
    }
}

#[async_trait::async_trait]
impl KoyoSorApi for DefaultKoyoSorApi {
    async fn quote(&self, query: Query) -> Result<Option<Quote>> {
        tracing::debug!(url =% self.url, ?query, "querying Koyo SOR");
        let response = self
            .client
            .post(self.url.clone())
            .json(&query)
            .send()
            .await?
            .text()
            .await?;
        tracing::debug!(%response, "received Koyo SOR quote");

        let quote = serde_json::from_str::<Quote>(&response)?;
        if quote.is_empty() {
            return Ok(None);
        }

        Ok(Some(quote))
    }
}
