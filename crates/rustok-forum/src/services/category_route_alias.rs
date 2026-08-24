use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, QueryResult, Statement};

pub const MAX_FORUM_CATEGORY_ROUTE_ALIAS_REASON_LEN: usize = 500;
pub const FORUM_CATEGORY_RENAMED_ROUTE_REASON: &str = "Category slug changed";

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredCategoryRouteAlias {
    alias_id: Uuid,
    category_id: Uuid,
    locale: String,
    slug: String,
    reason: String,
}

impl ForumCategoryRouteService {
    /// Records one immutable self-target category route redirect after the
    /// compatibility translation row has moved to its new slug in the same
    /// transaction.
    ///
    /// During the remaining CAT-5 compatibility phase this table is only an
    /// append-only history donor for Taxonomy owner-sync. Taxonomy owns route
    /// namespace validation and decides whether the transaction may commit.
    pub(crate) async fn record_slug_rename_alias_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
        previous_slug: &str,
        reason: &str,
    ) -> ForumResult<Uuid> {
        let locale = normalize_route_locale(locale)?;
        let previous_slug = normalize_route_slug(previous_slug)?;
        let reason = normalize_category_route_alias_reason(reason)?;
        let alias_id = Uuid::new_v4();
        let statement = match txn.get_database_backend() {
            DatabaseBackend::Postgres => Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                r#"
                INSERT INTO forum_category_route_aliases (
                    tenant_id, alias_id, category_id, locale, slug, reason, created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
                ON CONFLICT (tenant_id, locale, slug) DO NOTHING
                "#,
                vec![
                    tenant_id.into(),
                    alias_id.into(),
                    category_id.into(),
                    locale.clone().into(),
                    previous_slug.clone().into(),
                    reason.clone().into(),
                ],
            ),
            DatabaseBackend::Sqlite => Statement::from_sql_and_values(
                DatabaseBackend::Sqlite,
                r#"
                INSERT INTO forum_category_route_aliases (
                    tenant_id, alias_id, category_id, locale, slug, reason, created_at
                )
                VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                ON CONFLICT (tenant_id, locale, slug) DO NOTHING
                "#,
                vec![
                    tenant_id.into(),
                    alias_id.into(),
                    category_id.into(),
                    locale.clone().into(),
                    previous_slug.clone().into(),
                    reason.clone().into(),
                ],
            ),
            backend => return Err(unsupported_category_route_backend(backend)),
        };
        txn.execute(statement).await?;

        let aliases =
            load_exact_category_route_aliases(txn, tenant_id, &locale, &previous_slug).await?;
        let existing = match aliases.as_slice() {
            [alias] => alias,
            _ => return Err(ForumError::CategoryRouteResolutionConflict),
        };
        if existing.category_id != category_id
            || existing.locale != locale
            || existing.slug != previous_slug
            || existing.reason != reason
        {
            return Err(ForumError::CategoryRouteResolutionConflict);
        }
        Ok(existing.alias_id)
    }
}

async fn load_exact_category_route_aliases<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> ForumResult<Vec<StoredCategoryRouteAlias>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT alias_id, category_id, locale, slug, reason
            FROM forum_category_route_aliases
            WHERE tenant_id = $1 AND locale = $2 AND slug = $3
            ORDER BY alias_id
            LIMIT 2
            "#,
            vec![tenant_id.into(), locale.into(), slug.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT alias_id, category_id, locale, slug, reason
            FROM forum_category_route_aliases
            WHERE tenant_id = ? AND locale = ? AND slug = ?
            ORDER BY alias_id
            LIMIT 2
            "#,
            vec![tenant_id.into(), locale.into(), slug.into()],
        ),
        backend => return Err(unsupported_category_route_backend(backend)),
    };
    db.query_all(statement)
        .await?
        .into_iter()
        .map(stored_category_route_alias_from_row)
        .collect()
}

fn stored_category_route_alias_from_row(row: QueryResult) -> ForumResult<StoredCategoryRouteAlias> {
    let alias_id: Uuid = row.try_get("", "alias_id")?;
    let category_id: Uuid = row.try_get("", "category_id")?;
    let locale: String = row.try_get("", "locale")?;
    let slug: String = row.try_get("", "slug")?;
    let reason: String = row.try_get("", "reason")?;
    if alias_id.is_nil() || category_id.is_nil() {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }
    Ok(StoredCategoryRouteAlias {
        alias_id,
        category_id,
        locale: normalize_stored_locale(&locale)?,
        slug: normalize_stored_slug(&slug)?,
        reason: normalize_category_route_alias_reason(&reason)?,
    })
}

fn normalize_category_route_alias_reason(reason: &str) -> ForumResult<String> {
    let reason = reason.trim();
    if reason.is_empty()
        || reason.chars().count() > MAX_FORUM_CATEGORY_ROUTE_ALIAS_REASON_LEN
        || reason.chars().any(char::is_control)
    {
        return Err(ForumError::Validation(
            "Forum category route alias reason is invalid".to_string(),
        ));
    }
    Ok(reason.to_string())
}

fn unsupported_category_route_backend(backend: DatabaseBackend) -> ForumError {
    ForumError::Validation(format!(
        "Forum category route identity does not support database backend {backend:?}"
    ))
}
