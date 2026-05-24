#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SitemapSubmitEndpoint {
    pub(super) endpoint: String,
    pub(super) request_url: String,
}

#[async_trait::async_trait]
pub(super) trait SitemapSubmissionAdapter: Send + Sync {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String>;
}

pub(super) struct HttpSitemapSubmissionAdapter {
    pub(super) client: reqwest::Client,
}

#[async_trait::async_trait]
impl SitemapSubmissionAdapter for HttpSitemapSubmissionAdapter {
    async fn submit_sitemap_index(&self, endpoint: SitemapSubmitEndpoint) -> Result<(), String> {
        let response = self
            .client
            .get(endpoint.request_url)
            .send()
            .await
            .map_err(|error| {
                format!(
                    "request failed for endpoint `{}` with error: {error}",
                    endpoint.endpoint
                )
            })?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "endpoint `{}` responded with status {}",
                endpoint.endpoint,
                response.status()
            ))
        }
    }
}
