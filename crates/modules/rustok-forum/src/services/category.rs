include!("category_mutation_support.rs");

/// Crate-private persistence seam shared by Forum category owner commands.
///
/// Canonical Category reads and localized copy no longer live here: public reads
/// are Taxonomy-backed and public mutations are owned by
/// `CategoryProjectionOwnerService`. This type remains only for transaction
/// helpers reused by topic/reply/import owners.
pub(super) struct CategoryService;

impl CategoryService {
    pub(crate) async fn ensure_exists_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<()> {
        Self::find_category_in_tx(txn, tenant_id, category_id).await?;
        Ok(())
    }

    pub(crate) async fn find_category_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<forum_category::Model> {
        let existing = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .one(txn)
            .await?;
        existing.ok_or(ForumError::CategoryNotFound(category_id))
    }

    pub(crate) async fn find_category_for_update_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
    ) -> ForumResult<forum_category::Model> {
        let query = forum_category::Entity::find_by_id(category_id)
            .filter(forum_category::Column::TenantId.eq(tenant_id));
        match txn.get_database_backend() {
            DatabaseBackend::Postgres => query
                .lock_exclusive()
                .one(txn)
                .await?
                .ok_or(ForumError::CategoryNotFound(category_id)),
            DatabaseBackend::Sqlite => {
                let statement = Statement::from_sql_and_values(
                    DatabaseBackend::Sqlite,
                    "UPDATE forum_categories SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
                    vec![tenant_id.into(), category_id.into()],
                );
                if txn.execute_raw(statement).await?.rows_affected() != 1 {
                    return Err(ForumError::CategoryNotFound(category_id));
                }
                query
                    .one(txn)
                    .await?
                    .ok_or(ForumError::CategoryNotFound(category_id))
            }
            backend => Err(ForumError::Validation(format!(
                "Forum category row locking does not support database backend {backend:?}"
            ))),
        }
    }

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        let now = Utc::now();
        let mut query = forum_category::Entity::update_many()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::Id.eq(category_id))
            .filter(forum_category::Column::TopicCount.gte(0))
            .filter(forum_category::Column::ReplyCount.gte(0));

        for (column, delta, label) in [
            (forum_category::Column::TopicCount, topic_delta, "topic"),
            (forum_category::Column::ReplyCount, reply_delta, "reply"),
        ] {
            if delta > 0 {
                query = query.col_expr(column, Expr::col(column).add(delta));
            } else if delta < 0 {
                let decrement = delta.checked_abs().ok_or_else(|| {
                    ForumError::Validation(format!("Forum category {label} counter delta overflow"))
                })?;
                query = query
                    .filter(column.gte(decrement))
                    .col_expr(column, Expr::col(column).sub(decrement));
            }
        }

        let updated = query
            .col_expr(forum_category::Column::UpdatedAt, Expr::val(now))
            .exec(txn)
            .await?;

        if updated.rows_affected != 1 {
            let exists = forum_category::Entity::find_by_id(category_id)
                .filter(forum_category::Column::TenantId.eq(tenant_id))
                .one(txn)
                .await?
                .is_some();
            if !exists {
                return Err(ForumError::CategoryNotFound(category_id));
            }
            return Err(ForumError::Validation(
                "Forum category counters do not permit the requested delta".to_string(),
            ));
        }

        Ok(())
    }
}
