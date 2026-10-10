use async_trait::async_trait;
use uuid::Uuid;

use crate::{ContentFetchRequest, ContentSourceSlug, NewsletterContentItem};

/// Trait that source modules (blog, forum, commerce) implement to supply
/// content for newsletter campaigns.
///
/// The newsletter module calls this trait when building campaign content,
/// without knowing the internal structure of the source module.
#[async_trait]
pub trait NewsletterContentProvider: Send + Sync {
    /// Source identifier (e.g., "blog", "forum", "commerce").
    fn source_slug(&self) -> ContentSourceSlug;

    /// Fetch recent content items suitable for newsletter inclusion.
    async fn fetch_content(
        &self,
        request: ContentFetchRequest,
    ) -> Result<Vec<NewsletterContentItem>, crate::NewsletterApiError>;

    /// Check if this provider has any content available for the given tenant.
    async fn has_content(&self, tenant_id: Uuid) -> bool;
}

/// Registry of content providers, keyed by source slug.
///
/// The newsletter module uses this registry to discover available content
/// sources at runtime. Source modules register themselves during their
/// runtime-extension registration phase.
#[derive(Default)]
pub struct ContentProviderRegistry {
    providers: Vec<Box<dyn NewsletterContentProvider>>,
}

impl ContentProviderRegistry {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    pub fn register(&mut self, provider: Box<dyn NewsletterContentProvider>) {
        self.providers.push(provider);
    }

    pub fn providers(&self) -> &[Box<dyn NewsletterContentProvider>] {
        &self.providers
    }

    pub fn find(&self, slug: &ContentSourceSlug) -> Option<&dyn NewsletterContentProvider> {
        self.providers
            .iter()
            .find(|p| p.source_slug() == *slug)
            .map(|p| p.as_ref())
    }

    pub fn source_slugs(&self) -> Vec<ContentSourceSlug> {
        self.providers.iter().map(|p| p.source_slug()).collect()
    }
}
