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

    pub(crate) async fn adjust_counters_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        topic_delta: i32,
        reply_delta: i32,
    ) -> ForumResult<()> {
        let now = Utc::now();
        let topic_count = clamped_counter_expr(forum_category::Column::TopicCount, topic_delta)?;
        let reply_count = clamped_counter_expr(forum_category::Column::ReplyCount, reply_delta)?;

        let updated = forum_category::Entity::update_many()
            .filter(forum_category::Column::TenantId.eq(tenant_id))
            .filter(forum_category::Column::Id.eq(category_id))
            .col_expr(forum_category::Column::TopicCount, topic_count)
            .col_expr(forum_category::Column::ReplyCount, reply_count)
            .col_expr(forum_category::Column::UpdatedAt, Expr::val(now))
            .exec(txn)
            .await?;

        if updated.rows_affected != 1 {
            return Err(ForumError::CategoryNotFound(category_id));
        }

        Ok(())
    }

    fn clamped_counter_expr(
        column: forum_category::Column,
        delta: i32,
    ) -> ForumResult<sea_orm::sea_query::SimpleExpr> {
        if delta >= 0 {
            return Ok(Expr::col(column).add(delta).into());
        }

        let decrement = delta.checked_abs().ok_or_else(|| {
            ForumError::Validation("Forum category counter delta overflow".to_string())
        })?;
        Ok(Expr::case(
            Expr::col(column).gt(decrement),
            Expr::col(column).sub(decrement),
        )
        .finally(0)
        .into())
    }
}
