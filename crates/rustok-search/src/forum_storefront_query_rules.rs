use std::collections::HashSet;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement};
use uuid::Uuid;

use crate::{ForumStorefrontSearchExecutionError, SearchResultItem};

const MAX_FORUM_STOREFRONT_PIN_RULES: usize = 32;

/// Applies bounded pin ordering only to rows already returned by the bounded raw
/// Forum Search. Unlike the generic Search rule path, this helper never loads a
/// missing document from `search_documents`; every reordered row still passes
/// Forum owner eligibility afterwards.
pub(crate) async fn apply_existing_forum_query_rules(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    normalized_query: &str,
    mut items: Vec<SearchResultItem>,
) -> Result<Vec<SearchResultItem>, ForumStorefrontSearchExecutionError> {
    if normalized_query.is_empty() || items.is_empty() {
        return Ok(items);
    }
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(ForumStorefrontSearchExecutionError::Invariant(
            "Forum storefront Search query rules require PostgreSQL",
        ));
    }

    let rows = db
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            SELECT document_id, pinned_position
            FROM search_query_rules
            WHERE tenant_id = $1
              AND query_normalized = $2
              AND rule_kind = 'pin_document'
            ORDER BY pinned_position ASC, updated_at DESC
            LIMIT $3
            "#,
            vec![
                tenant_id.into(),
                normalized_query.to_string().into(),
                ((MAX_FORUM_STOREFRONT_PIN_RULES + 1) as i64).into(),
            ],
        ))
        .await
        .map_err(ForumStorefrontSearchExecutionError::Database)?;
    if rows.len() > MAX_FORUM_STOREFRONT_PIN_RULES {
        return Err(ForumStorefrontSearchExecutionError::Invariant(
            "Forum storefront Search query rule count exceeded its bound",
        ));
    }

    let mut seen_rule_documents = HashSet::new();
    let mut pinned = Vec::new();
    for row in rows {
        let (document_id, pinned_position) = map_rule(row)?;
        if !seen_rule_documents.insert(document_id) {
            return Err(ForumStorefrontSearchExecutionError::Invariant(
                "Forum storefront Search query rules contained a duplicate document",
            ));
        }
        if let Some(index) = items.iter().position(|item| item.id == document_id) {
            pinned.push((pinned_position, items.remove(index)));
        }
    }

    for (pinned_position, item) in pinned.into_iter().rev() {
        let index = pinned_position.saturating_sub(1) as usize;
        items.insert(index.min(items.len()), item);
    }
    Ok(items)
}

fn map_rule(
    row: QueryResult,
) -> Result<(Uuid, u32), ForumStorefrontSearchExecutionError> {
    let document_id = row
        .try_get("", "document_id")
        .map_err(ForumStorefrontSearchExecutionError::Database)?;
    let pinned_position = row
        .try_get::<i32>("", "pinned_position")
        .map_err(ForumStorefrontSearchExecutionError::Database)?
        .max(1) as u32;
    Ok((document_id, pinned_position))
}
