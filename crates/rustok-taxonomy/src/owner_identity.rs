use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    TaxonomyResult, TaxonomyTermKind,
    entities::taxonomy_term,
};

/// Returns whether an exact Taxonomy identity exists inside the requested
/// tenant and term kind without exposing Taxonomy persistence to consumers.
///
/// This is intentionally narrower than the owner read API: callers that only
/// need to validate an attached donor identity should not have to materialize
/// localized copy, routes, aliases or hierarchy state.
pub async fn taxonomy_term_identity_exists<C>(
    db: &C,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    term_id: Uuid,
) -> TaxonomyResult<bool>
where
    C: ConnectionTrait,
{
    Ok(taxonomy_term::Entity::find_by_id(term_id)
        .filter(taxonomy_term::Column::TenantId.eq(tenant_id))
        .filter(taxonomy_term::Column::Kind.eq(kind))
        .one(db)
        .await?
        .is_some())
}
