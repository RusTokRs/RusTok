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
    /// Reserves one current route key for category create or a new translation.
    ///
    /// Historical route keys are never reusable, including by the category that
    /// originally owned them. This keeps old localized URLs deterministic.
    pub(crate) async fn ensure_current_route_key_available_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
        slug: &str,
    ) -> ForumResult<()> {
        let locale = normalize_route_locale(locale)?;
        let slug = normalize_route_slug_for_write(slug)?;
        lock_category_route_key_in_tx(txn, tenant_id, &locale, &slug).await?;

        let current = load_exact_current_route_owners(txn, tenant_id, &locale, &slug).await?;
        match current.as_slice() {
            [] => {}
            [owner] if *owner == category_id => {}
            _ => return Err(ForumError::CategoryRouteResolutionConflict),
        }
        if !load_exact_category_route_aliases(txn, tenant_id, &locale, &slug)
            .await?
            .is_empty()
        {
            return Err(ForumError::CategoryRouteResolutionConflict);
        }
        Ok(())
    }

    /// Locks and validates both sides of one localized slug rename.
    pub(crate) async fn prepare_slug_rename_in_tx(
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        category_id: Uuid,
        locale: &str,
        previous_slug: &str,
        slug: &str,
    ) -> ForumResult<()> {
        let locale = normalize_route_locale(locale)?;
        let previous_slug = normalize_route_slug(previous_slug)?;
        let slug = normalize_route_slug_for_write(slug)?;
        if previous_slug == slug {
            return Ok(());
        }

        let mut keys = [previous_slug.as_str(), slug.as_str()];
        keys.sort_unstable();
        for key in keys {
            lock_category_route_key_in_tx(txn, tenant_id, &locale, key).await?;
        }

        match load_exact_current_route_owners(txn, tenant_id, &locale, &previous_slug)
            .await?
            .as_slice()
        {
            [owner] if *owner == category_id => {}
            _ => return Err(ForumError::CategoryRouteResolutionConflict),
        }
        if !load_exact_current_route_owners(txn, tenant_id, &locale, &slug)
            .await?
            .is_empty()
            || !load_exact_category_route_aliases(txn, tenant_id, &locale, &slug)
                .await?
                .is_empty()
            || !load_exact_category_route_aliases(txn, tenant_id, &locale, &previous_slug)
                .await?
                .is_empty()
        {
            return Err(ForumError::CategoryRouteResolutionConflict);
        }
        Ok(())
    }

    /// Records one immutable self-target category route redirect after the
    /// translation row has moved to its new slug in the same transaction.
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

async fn load_alias_route_candidates(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    slug: &str,
) -> ForumResult<Vec<CategoryRouteCandidate>> {
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT alias_id, category_id, locale, slug, reason
            FROM forum_category_route_aliases
            WHERE tenant_id = $1 AND slug = $2
            ORDER BY locale, alias_id
            LIMIT 65
            "#,
            vec![tenant_id.into(), slug.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT alias_id, category_id, locale, slug, reason
            FROM forum_category_route_aliases
            WHERE tenant_id = ? AND slug = ?
            ORDER BY locale, alias_id
            LIMIT 65
            "#,
            vec![tenant_id.into(), slug.into()],
        ),
        backend => return Err(unsupported_category_route_backend(backend)),
    };
    let aliases = db
        .query_all(statement)
        .await?
        .into_iter()
        .map(stored_category_route_alias_from_row)
        .collect::<ForumResult<Vec<_>>>()?;
    if aliases.len() > MAX_FORUM_CATEGORY_ROUTE_CANDIDATES as usize {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }
    if aliases.is_empty() {
        return Ok(Vec::new());
    }

    let category_ids = aliases
        .iter()
        .map(|alias| alias.category_id)
        .collect::<HashSet<_>>();
    let (existing_ids, archived_ids) =
        load_category_route_state(db, tenant_id, &category_ids).await?;
    if existing_ids != category_ids {
        return Err(ForumError::CategoryRouteResolutionConflict);
    }

    aliases
        .into_iter()
        .map(|alias| {
            if alias.slug != slug {
                return Err(ForumError::CategoryRouteResolutionConflict);
            }
            Ok(CategoryRouteCandidate {
                category_id: alias.category_id,
                locale: alias.locale,
                active: !archived_ids.contains(&alias.category_id),
                alias_id: Some(alias.alias_id),
            })
        })
        .collect()
}

async fn load_exact_current_route_owners<C>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> ForumResult<Vec<Uuid>>
where
    C: ConnectionTrait,
{
    let statement = match db.get_database_backend() {
        DatabaseBackend::Postgres => Statement::from_sql_and_values(
            DatabaseBackend::Postgres,
            r#"
            SELECT category_id
            FROM forum_category_translations
            WHERE tenant_id = $1 AND locale = $2 AND slug = $3
            ORDER BY category_id
            LIMIT 2
            "#,
            vec![tenant_id.into(), locale.into(), slug.into()],
        ),
        DatabaseBackend::Sqlite => Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            r#"
            SELECT category_id
            FROM forum_category_translations
            WHERE tenant_id = ? AND locale = ? AND slug = ?
            ORDER BY category_id
            LIMIT 2
            "#,
            vec![tenant_id.into(), locale.into(), slug.into()],
        ),
        backend => return Err(unsupported_category_route_backend(backend)),
    };
    db.query_all(statement)
        .await?
        .into_iter()
        .map(|row| row.try_get("", "category_id").map_err(ForumError::from))
        .collect()
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

async fn lock_category_route_key_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    slug: &str,
) -> ForumResult<()> {
    match txn.get_database_backend() {
        DatabaseBackend::Postgres => {
            let key = format!("forum-category-route:{tenant_id}:{locale}:{slug}");
            txn.execute(Statement::from_sql_and_values(
                DatabaseBackend::Postgres,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                [key.into()],
            ))
            .await?;
            Ok(())
        }
        DatabaseBackend::Sqlite => Ok(()),
        backend => Err(unsupported_category_route_backend(backend)),
    }
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
