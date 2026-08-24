impl CategoryService {
    pub(crate) async fn insert_import_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        record: &crate::import_write_preparation::ForumPreparedImportCategory,
    ) -> ForumResult<()> {
        if tenant_id.is_nil() || record.id.is_nil() {
            return Err(ForumError::Validation(
                "Forum import category insert requires non-nil tenant and category IDs".to_string(),
            ));
        }
        validate_category_name(&record.name)?;
        let locale = normalize_locale(&record.locale)?;
        if locale != record.locale {
            return Err(ForumError::Validation(
                "Forum import category locale must already be normalized".to_string(),
            ));
        }
        let slug = normalize_required_slug(&record.slug)?;
        if slug != record.slug {
            return Err(ForumError::Validation(
                "Forum import category slug must already be owner-normalized".to_string(),
            ));
        }
        if record.position < 0 {
            return Err(ForumError::Validation(
                "Forum import category position cannot be negative".to_string(),
            ));
        }
        let created_at = chrono::DateTime::<Utc>::from_timestamp_millis(record.created_at_ms)
            .ok_or_else(|| {
                ForumError::Validation(
                    "Forum import category creation timestamp is outside owner range".to_string(),
                )
            })?;

        lock_category_tree_in_tx(txn, tenant_id).await?;
        if let Some(parent_id) = record.parent_id {
            Self::find_category_in_tx(txn, tenant_id, parent_id).await?;
        }
        shift_siblings_for_insert_in_tx(
            txn,
            tenant_id,
            record.parent_id,
            record.position,
            Utc::now(),
        )
        .await?;

        forum_category::ActiveModel {
            id: Set(record.id),
            tenant_id: Set(tenant_id),
            parent_id: Set(record.parent_id),
            position: Set(record.position),
            icon: Set(record.icon.clone()),
            color: Set(record.color.clone()),
            moderated: Set(record.moderated),
            topic_count: Set(0),
            reply_count: Set(0),
            created_at: Set(created_at.into()),
            updated_at: Set(created_at.into()),
        }
        .insert(txn)
        .await?;

        forum_category_translation::ActiveModel {
            id: Set(Uuid::new_v4()),
            category_id: Set(record.id),
            tenant_id: Set(tenant_id),
            locale: Set(locale.clone()),
            name: Set(record.name.clone()),
            slug: Set(slug.clone()),
            description: Set(record.description.clone()),
        }
        .insert(txn)
        .await?;

        taxonomy_sync::sync_category_copy_in_tx(
            txn,
            tenant_id,
            record.id,
            locale,
            record.name.clone(),
            slug,
            record.description.clone(),
        )
        .await?;
        taxonomy_sync::sync_siblings_for_parent_in_tx(txn, tenant_id, record.parent_id).await?;

        Ok(())
    }
}
