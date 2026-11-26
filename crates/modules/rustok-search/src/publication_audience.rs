use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::search_publication::{TenantSearchPublication, TenantSearchPublicationSources};
use rustok_api::{PortError, SharedStaticModuleSettingsReader, is_valid_module_slug};
use rustok_core::ModuleRuntimeExtensions;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{SearchAudiencePolicyPort, SearchAudienceRegistry, SearchAudienceRequest};

impl SearchAudienceRegistry {
    /// Bind each explicit native publication contribution as a gated source.
    pub fn register_publication_sources(
        &mut self,
        db: &DatabaseConnection,
        extensions: &ModuleRuntimeExtensions,
    ) -> Result<(), PortError> {
        let Some(sources) = extensions
            .get::<TenantSearchPublicationSources>()
            .map(TenantSearchPublicationSources::sources)
            .filter(|sources| !sources.is_empty())
        else {
            return Ok(());
        };
        let settings = extensions
            .get::<SharedStaticModuleSettingsReader>()
            .ok_or_else(|| {
                PortError::unavailable(
                    "search.publication.settings_reader_missing",
                    "Search publication sources require the tenant settings reader",
                )
            })?;

        let mut registry = self.clone();
        for source in sources {
            if !is_valid_module_slug(source.module_slug)
                || !is_valid_module_slug(source.entity_type)
            {
                return Err(PortError::invariant_violation(
                    "search.publication.invalid_source",
                    "Invalid Search publication source or entity kind",
                ));
            }
            registry.register(PublicationAudience {
                slug: source.module_slug,
                entity_type: source.entity_type,
                owner: (source.build)(db.clone(), settings.clone()),
            })?;
        }
        *self = registry;
        Ok(())
    }
}

struct PublicationAudience {
    slug: &'static str,
    entity_type: &'static str,
    owner: Arc<dyn TenantSearchPublication>,
}

#[async_trait]
impl SearchAudiencePolicyPort for PublicationAudience {
    fn source_module(&self) -> &'static str {
        self.slug
    }

    async fn public_discovery_allowed(&self, tenant_id: Uuid) -> Result<bool, PortError> {
        self.owner.public_discovery_allowed(tenant_id).await
    }

    async fn filter_eligible_candidates(
        &self,
        request: SearchAudienceRequest,
    ) -> Result<Vec<Uuid>, PortError> {
        let context_matches = request.request_context.as_ref().is_some_and(|context| {
            context.tenant_id == request.tenant_id
                && request.auth.as_ref().is_none_or(|auth| {
                    auth.tenant_id == request.tenant_id && context.user_id == Some(auth.user_id)
                })
        });
        if request.tenant_id.is_nil() || !context_matches {
            return Err(PortError::forbidden(
                "search.publication.request_context_mismatch",
                "Search requires matching tenant and actor context",
            ));
        }

        let ids = request
            .candidates
            .into_iter()
            .filter(|candidate| {
                candidate.source_module == self.slug && candidate.entity_type == self.entity_type
            })
            .map(|candidate| candidate.document_id)
            .collect::<Vec<_>>();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.owner
            .filter_published_ids(request.tenant_id, ids)
            .await
    }
}
