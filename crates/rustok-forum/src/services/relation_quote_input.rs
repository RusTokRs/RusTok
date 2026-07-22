use std::collections::BTreeSet;

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::dto::{ForumQuoteReferenceInput, ForumQuoteTargetKindInput};
use crate::entities::{forum_quote, forum_relation_revision};
use crate::error::{ForumError, ForumResult};
use crate::mentions::{
    ForumContentTarget, ForumQuoteReference, FORUM_MAX_QUOTE_REFERENCES_PER_REVISION,
};

pub(crate) fn normalize_quote_inputs(
    inputs: Vec<ForumQuoteReferenceInput>,
) -> ForumResult<Vec<ForumQuoteReference>> {
    if inputs.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
        return Err(ForumError::Validation(format!(
            "Forum revision exceeds the {FORUM_MAX_QUOTE_REFERENCES_PER_REVISION}-quote limit"
        )));
    }

    inputs
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|input| {
            let target = match input.target_kind {
                ForumQuoteTargetKindInput::Topic => ForumContentTarget::topic(input.target_id),
                ForumQuoteTargetKindInput::Reply => ForumContentTarget::reply(input.target_id),
            };
            ForumQuoteReference::new(target, input.revision_id)
        })
        .collect()
}

pub(crate) async fn resolve_inline_update_quotes(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    source: ForumContentTarget,
    locale: &str,
    input: Option<Vec<ForumQuoteReferenceInput>>,
) -> ForumResult<Vec<ForumQuoteReference>> {
    match input {
        Some(input) => normalize_quote_inputs(input),
        None => load_latest_quotes(db, tenant_id, source, locale).await,
    }
}

async fn load_latest_quotes(
    db: &DatabaseConnection,
    tenant_id: uuid::Uuid,
    source: ForumContentTarget,
    locale: &str,
) -> ForumResult<Vec<ForumQuoteReference>> {
    let source_kind = match source.kind() {
        crate::mentions::ForumContentTargetKind::Topic => "topic",
        crate::mentions::ForumContentTargetKind::Reply => "reply",
    };
    let Some(revision) = forum_relation_revision::Entity::find()
        .filter(forum_relation_revision::Column::TenantId.eq(tenant_id))
        .filter(forum_relation_revision::Column::TargetKind.eq(source_kind))
        .filter(forum_relation_revision::Column::TargetId.eq(source.id()))
        .filter(forum_relation_revision::Column::Locale.eq(locale))
        .order_by_desc(forum_relation_revision::Column::RevisionId)
        .one(db)
        .await?
    else {
        return Ok(Vec::new());
    };

    let rows = forum_quote::Entity::find()
        .filter(forum_quote::Column::TenantId.eq(tenant_id))
        .filter(forum_quote::Column::SourceKind.eq(source_kind))
        .filter(forum_quote::Column::SourceId.eq(source.id()))
        .filter(forum_quote::Column::SourceLocale.eq(locale))
        .filter(forum_quote::Column::SourceRevisionId.eq(revision.revision_id))
        .order_by_asc(forum_quote::Column::QuotedKind)
        .order_by_asc(forum_quote::Column::QuotedId)
        .order_by_asc(forum_quote::Column::QuotedRevisionId)
        .limit((FORUM_MAX_QUOTE_REFERENCES_PER_REVISION + 1) as u64)
        .all(db)
        .await?;
    if rows.len() > FORUM_MAX_QUOTE_REFERENCES_PER_REVISION {
        return Err(ForumError::Validation(
            "Persisted Forum quote snapshot exceeds owner command limits".to_string(),
        ));
    }

    rows.into_iter()
        .map(|row| {
            let target = match row.quoted_kind.as_str() {
                "topic" => ForumContentTarget::topic(row.quoted_id),
                "reply" => ForumContentTarget::reply(row.quoted_id),
                _ => return Err(ForumError::relation_revision_unavailable()),
            };
            ForumQuoteReference::new(target, row.quoted_revision_id)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::normalize_quote_inputs;
    use crate::dto::{ForumQuoteReferenceInput, ForumQuoteTargetKindInput};
    use crate::mentions::FORUM_MAX_QUOTE_REFERENCES_PER_REVISION;
    use uuid::Uuid;

    #[test]
    fn inline_quote_inputs_are_deduplicated_inside_the_raw_bound() {
        let quote = ForumQuoteReferenceInput {
            target_kind: ForumQuoteTargetKindInput::Reply,
            target_id: Uuid::new_v4(),
            revision_id: 7,
        };
        let normalized = normalize_quote_inputs(vec![quote.clone(), quote])
            .expect("duplicate quotes should normalize");
        assert_eq!(normalized.len(), 1);
    }

    #[test]
    fn inline_quote_inputs_reject_oversized_raw_payloads() {
        let oversized = (0..=FORUM_MAX_QUOTE_REFERENCES_PER_REVISION)
            .map(|index| ForumQuoteReferenceInput {
                target_kind: ForumQuoteTargetKindInput::Topic,
                target_id: Uuid::new_v4(),
                revision_id: index as i64 + 1,
            })
            .collect();
        assert!(normalize_quote_inputs(oversized).is_err());
    }
}
