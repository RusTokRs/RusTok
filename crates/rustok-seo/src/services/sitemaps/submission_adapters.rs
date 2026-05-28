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

pub(super) struct SitemapSubmissionRuntime {
    adapter: Box<dyn SitemapSubmissionAdapter>,
}

impl SitemapSubmissionRuntime {
    pub(super) fn default_with_timeout(timeout_secs: u64) -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build()
            .map_err(|error| format!("failed to create sitemap submission client: {error}"))?;
        Ok(Self {
            adapter: Box::new(HttpSitemapSubmissionAdapter { client }),
        })
    }

    pub(super) fn adapter(&self) -> &dyn SitemapSubmissionAdapter {
        self.adapter.as_ref()
    }
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
                if error.is_timeout() {
                    format!(
                        "request timeout for endpoint `{}`: {error}",
                        endpoint.endpoint
                    )
                } else {
                    format!(
                        "request failed for endpoint `{}` with error: {error}",
                        endpoint.endpoint
                    )
                }
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
