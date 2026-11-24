use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_taxonomy_ports::{
    MAX_TAXONOMY_TERM_NAMES_PER_CALL, ReadTaxonomyTermNamesRequest,
    TAXONOMY_TERM_NAMES_READ_CLAIM, TaxonomyTermNamesReadPort,
};
use uuid::Uuid;

use crate::{TaxonomyError, TaxonomyService};

#[async_trait]
impl TaxonomyTermNamesReadPort for TaxonomyService {
    async fn read_term_names(
        &self,
        context: PortContext,
        request: ReadTaxonomyTermNamesRequest,
    ) -> Result<BTreeMap<Uuid, String>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if context.actor.kind == PortActorKind::User
            || !context.claims.iter().any(|claim| claim == TAXONOMY_TERM_NAMES_READ_CLAIM)
        {
            return Err(PortError::forbidden(
                "taxonomy.term_names_forbidden",
                "Taxonomy term name access is denied",
            ));
        }
        let tenant_id = Uuid::parse_str(&context.tenant_id)
            .ok()
            .filter(|tenant_id| !tenant_id.is_nil())
            .ok_or_else(|| PortError::validation(
                "taxonomy.tenant_id_invalid",
                "Taxonomy tenant identity is invalid",
            ))?;
        if request.term_ids.len() > MAX_TAXONOMY_TERM_NAMES_PER_CALL
            || request.term_ids.iter().any(Uuid::is_nil)
        {
            return Err(PortError::validation(
                "taxonomy.term_name_ids_invalid",
                "Taxonomy term identities are invalid",
            ));
        }

        self.resolve_term_names(
            tenant_id,
            &request.term_ids,
            &context.locale,
            request.fallback_locale.as_deref(),
        )
        .await
        .map(|names| names.into_iter().collect())
        .map_err(|error| match error {
            TaxonomyError::Database(_) => PortError::unavailable(
                "taxonomy.term_names_unavailable",
                "Taxonomy term names are temporarily unavailable",
            ),
            TaxonomyError::Validation(_) => PortError::validation(
                "taxonomy.term_names_context_invalid",
                "Taxonomy term name locale is invalid",
            ),
            TaxonomyError::Forbidden(_) => PortError::forbidden(
                "taxonomy.term_names_forbidden",
                "Taxonomy term name access is denied",
            ),
            TaxonomyError::Internal(_)
            | TaxonomyError::TermNotFound(_)
            | TaxonomyError::DuplicateCanonicalKey(_)
            | TaxonomyError::DuplicateSlug(_)
            | TaxonomyError::DuplicateAlias(_)
            | TaxonomyError::Conflict(_)
            | TaxonomyError::TranslationRevisionExhausted { .. } => PortError::invariant_violation(
                "taxonomy.term_names_invariant",
                "Taxonomy term names could not be read safely",
            ),
        })
    }
}
