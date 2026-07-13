use crate::{CacheKeyBuilder, CacheKeyError, CacheNamespaceGeneration};

impl CacheKeyBuilder {
    /// Add a namespace generation component to a canonical cache key.
    ///
    /// After `CacheNamespaceGenerationStore::bump`, callers rebuild the same logical key with
    /// the new generation. Old values remain bounded by their TTL but are no longer reachable.
    pub fn generation(self, generation: CacheNamespaceGeneration) -> Result<Self, CacheKeyError> {
        self.named_identity("generation", generation.key_component())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CacheGenerationSource, CacheService};

    #[tokio::test]
    async fn generation_changes_canonical_key_without_changing_logical_identity() {
        let service = CacheService::from_url(None);
        let generations = service.namespace_generations();
        let first = generations.read("catalog-products").await.unwrap();
        let second = generations.bump("catalog-products").await.unwrap();

        assert_eq!(first.source(), CacheGenerationSource::LocalOnly);
        let first_key =
            CacheKeyBuilder::new("rustok", "prod", "tenant-a", "catalog", "v1", "product")
                .unwrap()
                .generation(first)
                .unwrap()
                .named_identity("id", "42")
                .unwrap()
                .build();
        let second_key =
            CacheKeyBuilder::new("rustok", "prod", "tenant-a", "catalog", "v1", "product")
                .unwrap()
                .generation(second)
                .unwrap()
                .named_identity("id", "42")
                .unwrap()
                .build();

        assert_ne!(first_key, second_key);
        assert!(first_key.contains(":generation:g-0:"));
        assert!(second_key.contains(":generation:g-1:"));
    }
}
