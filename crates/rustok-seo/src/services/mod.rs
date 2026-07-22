include!("services_base.rs");

/// Durable cursor for the transactionally persisted SEO redirect delivery log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoRedirectCacheCursor {
    pub created_at: chrono::DateTime<chrono::FixedOffset>,
    pub id: Uuid,
}

/// One persisted redirect transition that requires tenant-local cache invalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeoRedirectCacheChange {
    pub tenant_id: Uuid,
    pub cursor: SeoRedirectCacheCursor,
}

/// Count all persisted redirect delivery rows.
///
/// The delivery log is append-only in runtime code. Reconciliation compares this independent
/// count with cursor-ordered rows processed since the previous poll. A mismatch detects a late or
/// out-of-order commit whose timestamp/UUID sorts behind the current cursor and forces a safe full
/// cache clear plus cursor reseed.
pub async fn redirect_cache_change_count(db: &DatabaseConnection) -> SeoResult<u64> {
    use sea_orm::PaginatorTrait as _;

    crate::entities::seo_event_delivery::Entity::find()
        .filter(crate::entities::seo_event_delivery::Column::SourceKind.eq("redirect"))
        .count(db)
        .await
        .map_err(SeoError::from)
}

/// Read the newest persisted redirect delivery cursor.
///
/// Callers should read this cursor before clearing the complete process-local redirect cache on
/// startup. Any transaction committed after the returned cursor can then be consumed with
/// [`redirect_cache_changes_after`] without leaving a startup race.
pub async fn latest_redirect_cache_cursor(
    db: &DatabaseConnection,
) -> SeoResult<Option<SeoRedirectCacheCursor>> {
    use sea_orm::QueryOrder as _;

    crate::entities::seo_event_delivery::Entity::find()
        .filter(crate::entities::seo_event_delivery::Column::SourceKind.eq("redirect"))
        .order_by_desc(crate::entities::seo_event_delivery::Column::CreatedAt)
        .order_by_desc(crate::entities::seo_event_delivery::Column::Id)
        .one(db)
        .await
        .map(|row| {
            row.map(|row| SeoRedirectCacheCursor {
                created_at: row.created_at,
                id: row.id,
            })
        })
        .map_err(SeoError::from)
}

/// Read a bounded ordered page of redirect transitions after the supplied durable cursor.
pub async fn redirect_cache_changes_after(
    db: &DatabaseConnection,
    cursor: Option<&SeoRedirectCacheCursor>,
    limit: u64,
) -> SeoResult<Vec<SeoRedirectCacheChange>> {
    use sea_orm::{Condition, QueryOrder as _, QuerySelect as _};

    let mut query = crate::entities::seo_event_delivery::Entity::find()
        .filter(crate::entities::seo_event_delivery::Column::SourceKind.eq("redirect"));

    if let Some(cursor) = cursor {
        let created_at = cursor.created_at.clone();
        query = query.filter(
            Condition::any()
                .add(crate::entities::seo_event_delivery::Column::CreatedAt.gt(created_at.clone()))
                .add(
                    Condition::all()
                        .add(crate::entities::seo_event_delivery::Column::CreatedAt.eq(created_at))
                        .add(crate::entities::seo_event_delivery::Column::Id.gt(cursor.id)),
                ),
        );
    }

    query
        .order_by_asc(crate::entities::seo_event_delivery::Column::CreatedAt)
        .order_by_asc(crate::entities::seo_event_delivery::Column::Id)
        .limit(limit.clamp(1, 1_000))
        .all(db)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|row| SeoRedirectCacheChange {
                    tenant_id: row.tenant_id,
                    cursor: SeoRedirectCacheCursor {
                        created_at: row.created_at,
                        id: row.id,
                    },
                })
                .collect()
        })
        .map_err(SeoError::from)
}

/// Invalidate one tenant's process-local redirect cache.
pub async fn invalidate_redirect_cache(tenant_id: Uuid) {
    REDIRECT_CACHE.invalidate(&tenant_id).await;
}

/// Invalidate every process-local redirect cache entry during startup or cursor recovery.
pub async fn invalidate_all_redirect_cache() {
    REDIRECT_CACHE.invalidate_all();
    REDIRECT_CACHE.run_pending_tasks().await;
}

#[cfg(test)]
mod redirect_cache_cursor_tests {
    use super::*;
    use chrono::TimeZone as _;

    #[test]
    fn cursor_orders_timestamp_before_uuid_tiebreaker() {
        let at = chrono::FixedOffset::east_opt(0)
            .unwrap()
            .with_ymd_and_hms(2026, 7, 16, 0, 0, 0)
            .single()
            .unwrap();
        let first = SeoRedirectCacheCursor {
            created_at: at.clone(),
            id: Uuid::from_u128(1),
        };
        let second = SeoRedirectCacheCursor {
            created_at: at,
            id: Uuid::from_u128(2),
        };

        assert_ne!(first, second);
        assert!(first.id < second.id);
    }
}
