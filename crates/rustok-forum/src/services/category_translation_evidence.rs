use chrono::Utc;
use rustok_api::PortError;
use rustok_translation_targets::{
    OpaqueCursor, OpaqueRevision, TranslationResourceLifecycle, TranslationTargetChange,
    TranslationTargetChangePage, TranslationTargetChangesRequest,
};
use sea_orm::{
    ActiveModelTrait,
    ActiveValue::{NotSet, Set},
    ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use crate::{
    ForumError,
    entities::{
        forum_category::{Column as CategoryColumn, Entity as CategoryEntity},
        forum_category_translation::{
            Column as TranslationColumn, Entity as TranslationEntity,
        },
    },
};

use super::category_translation_target::{
    TRANSLATION_RESOURCE_KIND, category_revision, forum_category_identity,
    forum_database_error_to_port_error,
};

mod change_row {
    use sea_orm::entity::prelude::*;
    use serde::{Deserialize, Serialize};
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "forum_translation_changes")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i64,
        pub tenant_id: Uuid,
        pub resource_kind: String,
        pub resource_id: Uuid,
        pub resource_revision: String,
        pub operation: String,
        pub lifecycle: String,
        pub created_at: DateTimeWithTimeZone,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

pub(super) async fn record_category_translation_change_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    category_id: Uuid,
    operation: &str,
    lifecycle: TranslationResourceLifecycle,
) -> Result<OpaqueRevision, ForumError> {
    let category = CategoryEntity::find_by_id(category_id)
        .filter(CategoryColumn::TenantId.eq(tenant_id))
        .one(txn)
        .await?
        .ok_or(ForumError::CategoryNotFound(category_id))?;
    let translations = TranslationEntity::find()
        .filter(TranslationColumn::TenantId.eq(tenant_id))
        .filter(TranslationColumn::CategoryId.eq(category_id))
        .order_by_asc(TranslationColumn::Locale)
        .order_by_asc(TranslationColumn::Id)
        .all(txn)
        .await?;
    let resource_revision = category_revision(&category, &translations);

    change_row::ActiveModel {
        id: NotSet,
        tenant_id: Set(tenant_id),
        resource_kind: Set(TRANSLATION_RESOURCE_KIND.to_string()),
        resource_id: Set(category_id),
        resource_revision: Set(resource_revision.as_str().to_string()),
        operation: Set(operation.to_string()),
        lifecycle: Set(lifecycle_name(lifecycle).to_string()),
        created_at: Set(Utc::now().fixed_offset()),
    }
    .insert(txn)
    .await?;

    Ok(resource_revision)
}

pub(super) async fn read_category_translation_changes(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    request: &TranslationTargetChangesRequest,
) -> Result<TranslationTargetChangePage, PortError> {
    let after = request
        .after
        .as_ref()
        .map(|cursor| parse_change_cursor(cursor.as_str()))
        .transpose()?;

    let mut query = change_row::Entity::find()
        .filter(change_row::Column::TenantId.eq(tenant_id))
        .filter(change_row::Column::ResourceKind.eq(TRANSLATION_RESOURCE_KIND))
        .order_by_asc(change_row::Column::Id);
    if let Some(after) = after {
        query = query.filter(change_row::Column::Id.gt(after));
    }
    let rows = query
        .limit(u64::from(request.limit))
        .all(db)
        .await
        .map_err(forum_database_error_to_port_error)?;
    let next_cursor = rows.last().map(|change| {
        OpaqueCursor::new(change.id.to_string())
            .expect("Forum change sequence must satisfy the opaque cursor contract")
    });
    let changes = rows
        .into_iter()
        .map(|change| {
            let resource_revision = OpaqueRevision::new(change.resource_revision).map_err(|error| {
                PortError::invariant_violation(
                    "forum.translation_change_revision_invalid",
                    error.to_string(),
                )
            })?;
            let lifecycle = parse_lifecycle(&change.lifecycle)?;
            Ok(TranslationTargetChange {
                identity: forum_category_identity(change.resource_id),
                resource_revision,
                lifecycle,
            })
        })
        .collect::<Result<Vec<_>, PortError>>()?;

    Ok(TranslationTargetChangePage {
        changes,
        next_cursor,
    })
}

fn parse_change_cursor(value: &str) -> Result<i64, PortError> {
    match value.parse::<i64>() {
        Ok(cursor) if cursor > 0 => Ok(cursor),
        _ => Err(PortError::validation(
            "forum.translation_change_cursor_invalid",
            "Forum category translation change cursor must be a positive sequence",
        )),
    }
}

fn lifecycle_name(lifecycle: TranslationResourceLifecycle) -> &'static str {
    match lifecycle {
        TranslationResourceLifecycle::Active => "active",
        TranslationResourceLifecycle::Archived => "archived",
        TranslationResourceLifecycle::Deleted => "deleted",
        TranslationResourceLifecycle::Unavailable => "unavailable",
    }
}

fn parse_lifecycle(value: &str) -> Result<TranslationResourceLifecycle, PortError> {
    match value {
        "active" => Ok(TranslationResourceLifecycle::Active),
        "archived" => Ok(TranslationResourceLifecycle::Archived),
        "deleted" => Ok(TranslationResourceLifecycle::Deleted),
        "unavailable" => Ok(TranslationResourceLifecycle::Unavailable),
        _ => Err(PortError::invariant_violation(
            "forum.translation_change_lifecycle_invalid",
            "Forum translation change lifecycle is invalid",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_round_trip_covers_cursor_states() {
        for lifecycle in [
            TranslationResourceLifecycle::Active,
            TranslationResourceLifecycle::Archived,
            TranslationResourceLifecycle::Deleted,
            TranslationResourceLifecycle::Unavailable,
        ] {
            assert_eq!(parse_lifecycle(lifecycle_name(lifecycle)), Ok(lifecycle));
        }
    }

    #[test]
    fn change_cursor_requires_positive_sequence() {
        assert_eq!(parse_change_cursor("17").expect("cursor"), 17);
        assert!(parse_change_cursor("0").is_err());
        assert!(parse_change_cursor("not-a-sequence").is_err());
    }
}
