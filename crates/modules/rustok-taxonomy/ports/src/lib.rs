//! Published Taxonomy contracts for owner-scoped vocabulary reads.

use std::collections::BTreeMap;

use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const TAXONOMY_TERM_NAMES_READ_CLAIM: &str = "taxonomy.term_names.read";
pub const MAX_TAXONOMY_TERM_NAMES_PER_CALL: usize = 256;

#[cfg(any(feature = "grpc-client", feature = "grpc-server"))]
pub mod grpc;

pub fn authorize_term_names_read(context: &PortContext) -> Result<(), PortError> {
    context.require_policy(PortCallPolicy::read())?;
    if context.actor.kind == PortActorKind::User
        || context.actor.id.trim().is_empty()
        || !context
            .claims
            .iter()
            .any(|claim| claim == TAXONOMY_TERM_NAMES_READ_CLAIM)
    {
        return Err(PortError::forbidden(
            "taxonomy.term_names_forbidden",
            "Taxonomy term name access is denied",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadTaxonomyTermNamesRequest {
    pub term_ids: Vec<Uuid>,
    pub fallback_locale: Option<String>,
}

/// Resolves names for terms attached by another owner. Taxonomy scopes the read
/// to the authenticated tenant and uses its canonical locale fallback rules.
/// Missing and foreign-tenant IDs are both absent from the result.
#[async_trait]
pub trait TaxonomyTermNamesReadPort: Send + Sync {
    async fn read_term_names(
        &self,
        context: PortContext,
        request: ReadTaxonomyTermNamesRequest,
    ) -> Result<BTreeMap<Uuid, String>, PortError>;
}
