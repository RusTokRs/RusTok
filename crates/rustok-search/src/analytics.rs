use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use std::collections::BTreeMap;
use uuid::Uuid;

use rustok_core::{Error, Result};

use crate::engine::SearchEngineKind;

const CLICK_EVAL_LAG_MINUTES: i32 = 2;
pub const SLOW_QUERY_THRESHOLD_MS: u64 = 250;

#[derive(Debug, Clone, PartialEq)]
pub struct SearchAnalyticsSummary {
    pub window_days: u32,
    pub total_queries: u64,
    pub successful_queries: u64,
    pub zero_result_queries: u64,
    pub zero_result_rate: f64,
    pub slow_queries: u64,
    pub slow_query_rate: f64,
    pub avg_took_ms: f64,
    pub avg_results_per_query: f64,
    pub unique_queries: u64,
    pub clicked_queries: u64,
    pub total_clicks: u64,
    pub click_through_rate: f64,
    pub abandonment_queries: u64,
    pub abandonment_rate: f64,
    pub last_query_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchAnalyticsQueryRow {
    pub query: String,
    pub hits: u64,
    pub zero_result_hits: u64,
    pub clicks: u64,
    pub avg_took_ms: f64,
    pub avg_results: f64,
    pub click_through_rate: f64,
    pub abandonment_rate: f64,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchAnalyticsInsightRow {
    pub query: String,
    pub hits: u64,
    pub zero_result_hits: u64,
    pub clicks: u64,
    pub click_through_rate: f64,
    pub abandonment_rate: f64,
    pub recommendation: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchAnalyticsSnapshot {
    pub summary: SearchAnalyticsSummary,
    pub top_queries: Vec<SearchAnalyticsQueryRow>,
    pub zero_result_queries: Vec<SearchAnalyticsQueryRow>,
    pub slow_queries: Vec<SearchAnalyticsQueryRow>,
    pub low_ctr_queries: Vec<SearchAnalyticsQueryRow>,
    pub abandonment_queries: Vec<SearchAnalyticsQueryRow>,
    pub intelligence_candidates: Vec<SearchAnalyticsInsightRow>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchQueryLogRecord {
    pub tenant_id: Uuid,
    pub surface: String,
    pub query: String,
    pub locale: Option<String>,
    pub engine: SearchEngineKind,
    pub result_count: u64,
    pub took_ms: u64,
    pub status: String,
    pub entity_types: Vec<String>,
    pub source_modules: Vec<String>,
    pub statuses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SearchClickRecord {
    pub tenant_id: Uuid,
    pub query_log_id: i64,
    pub document_id: Uuid,
    pub position: Option<u32>,
    pub href: Option<String>,
}

pub struct SearchAnalyticsService;

impl SearchAnalyticsService {
    pub async fn record_query(
        db: &DatabaseConnection,
        record: SearchQueryLogRecord,
    ) -> Result<Option<i64>> {
        ensure_postgres(db)?;

        let normalized_query = normalize_query_text(&record.query);
        if normalized_query.is_empty() {
            return Ok(None);
        }

        let filters = serde_json::json!({
            "entity_types": record.entity_types,
            "source_modules": record.source_modules,
            "statuses": record.statuses,
        });

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_query_logs (
                tenant_id,
                surface,
                query_text,
                query_normalized,
                locale,
                engine,
                result_count,
                took_ms,
                status,
                filters
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id
            "#,
            vec![
                record.tenant_id.into(),
                record.surface.into(),
                record.query.trim().to_string().into(),
                normalized_query.into(),
                record.locale.into(),
                record.engine.as_str().to_string().into(),
                (record.result_count.min(i64::MAX as u64) as i64).into(),
                (record.took_ms.min(i64::MAX as u64) as i64).into(),
                record.status.into(),
                filters.into(),
            ],
        );

        let row = db.query_one(stmt).await.map_err(Error::Database)?;
        Ok(row.map(|row| read_i64(&row, "id")))
    }

    pub async fn record_click(db: &DatabaseConnection, record: SearchClickRecord) -> Result<()> {
        ensure_postgres(db)?;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO search_query_clicks (
                tenant_id,
                query_log_id,
                document_id,
                position,
                href
            )
            SELECT
                $1,
                q.id,
                $3,
                $4,
                $5
            FROM search_query_logs q
            WHERE q.id = $2
              AND q.tenant_id = $1
            "#,
            vec![
                record.tenant_id.into(),
                record.query_log_id.into(),
                record.document_id.into(),
                record.position.map(|value| value as i32).into(),
                record.href.into(),
            ],
        );

        let result = db.execute(stmt).await.map_err(Error::Database)?;
        if result.rows_affected() == 0 {
            return Err(Error::NotFound(
                "search query log not found for click tracking".to_string(),
            ));
        }

        Ok(())
    }

    pub async fn snapshot(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        days: u32,
        limit: usize,
    ) -> Result<SearchAnalyticsSnapshot> {
        ensure_postgres(db)?;

        let summary = Self::summary(db, tenant_id, days).await?;
        let top_queries = Self::query_rows(db, tenant_id, days, limit, QueryRowMode::Top).await?;
        let zero_result_queries =
            Self::query_rows(db, tenant_id, days, limit, QueryRowMode::ZeroResults).await?;
        let slow_queries = Self::query_rows(db, tenant_id, days, limit, QueryRowMode::Slow).await?;
        let low_ctr_queries =
            Self::query_rows(db, tenant_id, days, limit, QueryRowMode::LowCtr).await?;
        let abandonment_queries =
            Self::query_rows(db, tenant_id, days, limit, QueryRowMode::Abandonment).await?;
        let intelligence_candidates = build_intelligence_candidates(
            &top_queries,
            &zero_result_queries,
            &slow_queries,
            &low_ctr_queries,
            &abandonment_queries,
            limit,
        );

        Ok(SearchAnalyticsSnapshot {
            summary,
            top_queries,
            zero_result_queries,
            slow_queries,
            low_ctr_queries,
            abandonment_queries,
            intelligence_candidates,
        })
    }

    async fn summary(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        days: u32,
    ) -> Result<SearchAnalyticsSummary> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            summary_sql(),
            vec![
                tenant_id.into(),
                (days.min(i32::MAX as u32) as i32).into(),
                CLICK_EVAL_LAG_MINUTES.into(),
                (SLOW_QUERY_THRESHOLD_MS.min(i64::MAX as u64) as i64).into(),
            ],
        );

        let row = db
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| Error::NotFound("search analytics summary row".to_string()))?;

        Ok(SearchAnalyticsSummary {
            window_days: days,
            total_queries: read_i64(&row, "total_queries") as u64,
            successful_queries: read_i64(&row, "successful_queries") as u64,
            zero_result_queries: read_i64(&row, "zero_result_queries") as u64,
            zero_result_rate: read_f64(&row, "zero_result_rate"),
            slow_queries: read_i64(&row, "slow_queries") as u64,
            slow_query_rate: read_f64(&row, "slow_query_rate"),
            avg_took_ms: read_f64(&row, "avg_took_ms"),
            avg_results_per_query: read_f64(&row, "avg_results_per_query"),
            unique_queries: read_i64(&row, "unique_queries") as u64,
            clicked_queries: read_i64(&row, "clicked_queries") as u64,
            total_clicks: read_i64(&row, "total_clicks") as u64,
            click_through_rate: read_f64(&row, "click_through_rate"),
            abandonment_queries: read_i64(&row, "abandonment_queries") as u64,
            abandonment_rate: read_f64(&row, "abandonment_rate"),
            last_query_at: row
                .try_get::<Option<DateTime<Utc>>>("", "last_query_at")
                .map_err(Error::Database)?,
        })
    }

    async fn query_rows(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        days: u32,
        limit: usize,
        mode: QueryRowMode,
    ) -> Result<Vec<SearchAnalyticsQueryRow>> {
        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            query_rows_sql(mode),
            match mode {
                QueryRowMode::Slow => vec![
                    tenant_id.into(),
                    (days.min(i32::MAX as u32) as i32).into(),
                    CLICK_EVAL_LAG_MINUTES.into(),
                    (limit.clamp(1, 50) as i64).into(),
                    (SLOW_QUERY_THRESHOLD_MS.min(i64::MAX as u64) as i64).into(),
                ],
                _ => vec![
                    tenant_id.into(),
                    (days.min(i32::MAX as u32) as i32).into(),
                    CLICK_EVAL_LAG_MINUTES.into(),
                    (limit.clamp(1, 50) as i64).into(),
                ],
            },
        );

        db.query_all(stmt)
            .await
            .map_err(Error::Database)?
            .into_iter()
            .map(|row| {
                Ok(SearchAnalyticsQueryRow {
                    query: row
                        .try_get::<String>("", "query_text")
                        .map_err(Error::Database)?,
                    hits: read_i64(&row, "hits") as u64,
                    zero_result_hits: read_i64(&row, "zero_result_hits") as u64,
                    clicks: read_i64(&row, "clicks") as u64,
                    avg_took_ms: read_f64(&row, "avg_took_ms"),
                    avg_results: read_f64(&row, "avg_results"),
                    click_through_rate: read_f64(&row, "click_through_rate"),
                    abandonment_rate: read_f64(&row, "abandonment_rate"),
                    last_seen_at: row
                        .try_get::<DateTime<Utc>>("", "last_seen_at")
                        .map_err(Error::Database)?,
                })
            })
            .collect()
    }
}

#[derive(Clone, Copy)]
enum QueryRowMode {
    Top,
    ZeroResults,
    Slow,
    LowCtr,
    Abandonment,
}

fn ensure_postgres(db: &DatabaseConnection) -> Result<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(Error::External(
            "SearchAnalyticsService requires PostgreSQL backend".to_string(),
        ));
    }

    Ok(())
}

fn normalize_query_text(value: &str) -> String {
    value
        .split_whitespace()
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn build_intelligence_candidates(
    top_queries: &[SearchAnalyticsQueryRow],
    zero_result_queries: &[SearchAnalyticsQueryRow],
    slow_queries: &[SearchAnalyticsQueryRow],
    low_ctr_queries: &[SearchAnalyticsQueryRow],
    abandonment_queries: &[SearchAnalyticsQueryRow],
    limit: usize,
) -> Vec<SearchAnalyticsInsightRow> {
    let mut merged: BTreeMap<String, SearchAnalyticsQueryRow> = BTreeMap::new();

    for row in top_queries
        .iter()
        .chain(zero_result_queries.iter())
        .chain(slow_queries.iter())
        .chain(low_ctr_queries.iter())
        .chain(abandonment_queries.iter())
    {
        merged
            .entry(row.query.clone())
            .and_modify(|existing| {
                if row.hits > existing.hits {
                    *existing = row.clone();
                }
            })
            .or_insert_with(|| row.clone());
    }

    let mut candidates = merged
        .into_values()
        .filter_map(|row| {
            let recommendation = recommend_action(&row)?;
            Some(SearchAnalyticsInsightRow {
                query: row.query,
                hits: row.hits,
                zero_result_hits: row.zero_result_hits,
                clicks: row.clicks,
                click_through_rate: row.click_through_rate,
                abandonment_rate: row.abandonment_rate,
                recommendation,
            })
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        intelligence_score(right)
            .partial_cmp(&intelligence_score(left))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    candidates.truncate(limit.clamp(1, 25));
    candidates
}

fn recommend_action(row: &SearchAnalyticsQueryRow) -> Option<String> {
    if row.hits < 2 {
        return None;
    }

    if row.zero_result_hits == row.hits {
        return Some("add_synonyms_or_redirect".to_string());
    }

    if row.abandonment_rate >= 0.75 {
        return Some("pin_best_match_or_reduce_noise".to_string());
    }

    if row.click_through_rate <= 0.2 && row.avg_results >= 5.0 {
        return Some("tune_ranking_or_boost_exact_match".to_string());
    }

    if row.avg_took_ms >= SLOW_QUERY_THRESHOLD_MS as f64 {
        return Some("narrow_filters_or_review_indexes".to_string());
    }

    None
}

fn intelligence_score(row: &SearchAnalyticsInsightRow) -> f64 {
    let hit_score = row.hits as f64;
    let zero_result_penalty = row.zero_result_hits as f64 * 2.0;
    let abandonment_penalty = row.abandonment_rate * 10.0;
    let ctr_penalty = (1.0 - row.click_through_rate).max(0.0) * 5.0;
    hit_score + zero_result_penalty + abandonment_penalty + ctr_penalty
}

fn summary_sql() -> &'static str {
    r#"
    WITH click_counts AS (
        SELECT query_log_id, COUNT(*)::bigint AS click_count
        FROM search_query_clicks
        GROUP BY query_log_id
    ),
    successful_queries AS (
        SELECT
            q.id,
            q.query_normalized,
            q.created_at,
            q.result_count,
            q.took_ms,
            COALESCE(c.click_count, 0) AS click_count
        FROM search_query_logs q
        LEFT JOIN click_counts c ON c.query_log_id = q.id
        WHERE q.tenant_id = $1
          AND q.created_at >= NOW() - make_interval(days => $2)
          AND q.status = 'success'
    )
    SELECT
        (
            SELECT COUNT(*)::bigint
            FROM search_query_logs q
            WHERE q.tenant_id = $1
              AND q.created_at >= NOW() - make_interval(days => $2)
        ) AS total_queries,
        COUNT(*)::bigint AS successful_queries,
        COUNT(*) FILTER (WHERE result_count = 0)::bigint AS zero_result_queries,
        COALESCE(
            COUNT(*) FILTER (WHERE result_count = 0)::double precision
            / NULLIF(COUNT(*)::double precision, 0),
            0
        ) AS zero_result_rate,
        COUNT(*) FILTER (WHERE took_ms >= $4)::bigint AS slow_queries,
        COALESCE(
            COUNT(*) FILTER (WHERE took_ms >= $4)::double precision
            / NULLIF(COUNT(*)::double precision, 0),
            0
        ) AS slow_query_rate,
        COALESCE(AVG(took_ms)::double precision, 0) AS avg_took_ms,
        COALESCE(AVG(result_count)::double precision, 0) AS avg_results_per_query,
        COUNT(DISTINCT query_normalized)::bigint AS unique_queries,
        COUNT(*) FILTER (
            WHERE created_at < NOW() - make_interval(mins => $3)
              AND click_count > 0
        )::bigint AS clicked_queries,
        COALESCE(SUM(click_count), 0)::bigint AS total_clicks,
        COALESCE(
            COUNT(*) FILTER (
                WHERE created_at < NOW() - make_interval(mins => $3)
                  AND click_count > 0
            )::double precision
            / NULLIF(
                COUNT(*) FILTER (
                    WHERE created_at < NOW() - make_interval(mins => $3)
                )::double precision,
                0
            ),
            0
        ) AS click_through_rate,
        COUNT(*) FILTER (
            WHERE created_at < NOW() - make_interval(mins => $3)
              AND click_count = 0
        )::bigint AS abandonment_queries,
        COALESCE(
            COUNT(*) FILTER (
                WHERE created_at < NOW() - make_interval(mins => $3)
                  AND click_count = 0
            )::double precision
            / NULLIF(
                COUNT(*) FILTER (
                    WHERE created_at < NOW() - make_interval(mins => $3)
                )::double precision,
                0
            ),
            0
        ) AS abandonment_rate,
        MAX(created_at) AS last_query_at
    FROM successful_queries
    "#
}

fn query_rows_sql(mode: QueryRowMode) -> &'static str {
    match mode {
        QueryRowMode::Top => {
            r#"
            WITH click_counts AS (
                SELECT query_log_id, COUNT(*)::bigint AS click_count
                FROM search_query_clicks
                GROUP BY query_log_id
            ),
            successful_queries AS (
                SELECT
                    q.query_text,
                    q.query_normalized,
                    q.created_at,
                    q.result_count,
                    q.took_ms,
                    COALESCE(c.click_count, 0) AS click_count
                FROM search_query_logs q
                LEFT JOIN click_counts c ON c.query_log_id = q.id
                WHERE q.tenant_id = $1
                  AND q.created_at >= NOW() - make_interval(days => $2)
                  AND q.status = 'success'
            )
            SELECT
                MIN(query_text) AS query_text,
                COUNT(*)::bigint AS hits,
                COUNT(*) FILTER (WHERE result_count = 0)::bigint AS zero_result_hits,
                COALESCE(SUM(click_count), 0)::bigint AS clicks,
                COALESCE(AVG(took_ms)::double precision, 0) AS avg_took_ms,
                COALESCE(AVG(result_count)::double precision, 0) AS avg_results,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count > 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS click_through_rate,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count = 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS abandonment_rate,
                MAX(created_at) AS last_seen_at
            FROM successful_queries
            GROUP BY query_normalized
            ORDER BY hits DESC, last_seen_at DESC
            LIMIT $4
            "#
        }
        QueryRowMode::ZeroResults => {
            r#"
            WITH click_counts AS (
                SELECT query_log_id, COUNT(*)::bigint AS click_count
                FROM search_query_clicks
                GROUP BY query_log_id
            ),
            successful_queries AS (
                SELECT
                    q.query_text,
                    q.query_normalized,
                    q.created_at,
                    q.result_count,
                    q.took_ms,
                    COALESCE(c.click_count, 0) AS click_count
                FROM search_query_logs q
                LEFT JOIN click_counts c ON c.query_log_id = q.id
                WHERE q.tenant_id = $1
                  AND q.created_at >= NOW() - make_interval(days => $2)
                  AND q.status = 'success'
                  AND q.result_count = 0
            )
            SELECT
                MIN(query_text) AS query_text,
                COUNT(*)::bigint AS hits,
                COUNT(*)::bigint AS zero_result_hits,
                COALESCE(SUM(click_count), 0)::bigint AS clicks,
                COALESCE(AVG(took_ms)::double precision, 0) AS avg_took_ms,
                COALESCE(AVG(result_count)::double precision, 0) AS avg_results,
                0::double precision AS click_through_rate,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS abandonment_rate,
                MAX(created_at) AS last_seen_at
            FROM successful_queries
            GROUP BY query_normalized
            ORDER BY hits DESC, last_seen_at DESC
            LIMIT $4
            "#
        }
        QueryRowMode::LowCtr => {
            r#"
            WITH click_counts AS (
                SELECT query_log_id, COUNT(*)::bigint AS click_count
                FROM search_query_clicks
                GROUP BY query_log_id
            ),
            successful_queries AS (
                SELECT
                    q.query_text,
                    q.query_normalized,
                    q.created_at,
                    q.result_count,
                    q.took_ms,
                    COALESCE(c.click_count, 0) AS click_count
                FROM search_query_logs q
                LEFT JOIN click_counts c ON c.query_log_id = q.id
                WHERE q.tenant_id = $1
                  AND q.created_at >= NOW() - make_interval(days => $2)
                  AND q.status = 'success'
            )
            SELECT
                MIN(query_text) AS query_text,
                COUNT(*)::bigint AS hits,
                COUNT(*) FILTER (WHERE result_count = 0)::bigint AS zero_result_hits,
                COALESCE(SUM(click_count), 0)::bigint AS clicks,
                COALESCE(AVG(took_ms)::double precision, 0) AS avg_took_ms,
                COALESCE(AVG(result_count)::double precision, 0) AS avg_results,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count > 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS click_through_rate,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count = 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS abandonment_rate,
                MAX(created_at) AS last_seen_at
            FROM successful_queries
            GROUP BY query_normalized
            HAVING COUNT(*) >= 2
            ORDER BY click_through_rate ASC, hits DESC, last_seen_at DESC
            LIMIT $4
            "#
        }
        QueryRowMode::Slow => {
            r#"
            WITH click_counts AS (
                SELECT query_log_id, COUNT(*)::bigint AS click_count
                FROM search_query_clicks
                GROUP BY query_log_id
            ),
            successful_queries AS (
                SELECT
                    q.query_text,
                    q.query_normalized,
                    q.created_at,
                    q.result_count,
                    q.took_ms,
                    COALESCE(c.click_count, 0) AS click_count
                FROM search_query_logs q
                LEFT JOIN click_counts c ON c.query_log_id = q.id
                WHERE q.tenant_id = $1
                  AND q.created_at >= NOW() - make_interval(days => $2)
                  AND q.status = 'success'
            )
            SELECT
                MIN(query_text) AS query_text,
                COUNT(*)::bigint AS hits,
                COUNT(*) FILTER (WHERE result_count = 0)::bigint AS zero_result_hits,
                COALESCE(SUM(click_count), 0)::bigint AS clicks,
                COALESCE(AVG(took_ms)::double precision, 0) AS avg_took_ms,
                COALESCE(AVG(result_count)::double precision, 0) AS avg_results,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count > 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS click_through_rate,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count = 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS abandonment_rate,
                MAX(created_at) AS last_seen_at
            FROM successful_queries
            GROUP BY query_normalized
            HAVING AVG(took_ms) >= $5
            ORDER BY avg_took_ms DESC, hits DESC, last_seen_at DESC
            LIMIT $4
            "#
        }
        QueryRowMode::Abandonment => {
            r#"
            WITH click_counts AS (
                SELECT query_log_id, COUNT(*)::bigint AS click_count
                FROM search_query_clicks
                GROUP BY query_log_id
            ),
            successful_queries AS (
                SELECT
                    q.query_text,
                    q.query_normalized,
                    q.created_at,
                    q.result_count,
                    q.took_ms,
                    COALESCE(c.click_count, 0) AS click_count
                FROM search_query_logs q
                LEFT JOIN click_counts c ON c.query_log_id = q.id
                WHERE q.tenant_id = $1
                  AND q.created_at >= NOW() - make_interval(days => $2)
                  AND q.status = 'success'
            )
            SELECT
                MIN(query_text) AS query_text,
                COUNT(*)::bigint AS hits,
                COUNT(*) FILTER (WHERE result_count = 0)::bigint AS zero_result_hits,
                COALESCE(SUM(click_count), 0)::bigint AS clicks,
                COALESCE(AVG(took_ms)::double precision, 0) AS avg_took_ms,
                COALESCE(AVG(result_count)::double precision, 0) AS avg_results,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count > 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS click_through_rate,
                COALESCE(
                    COUNT(*) FILTER (
                        WHERE created_at < NOW() - make_interval(mins => $3)
                          AND click_count = 0
                    )::double precision
                    / NULLIF(
                        COUNT(*) FILTER (
                            WHERE created_at < NOW() - make_interval(mins => $3)
                        )::double precision,
                        0
                    ),
                    0
                ) AS abandonment_rate,
                MAX(created_at) AS last_seen_at
            FROM successful_queries
            GROUP BY query_normalized
            HAVING COUNT(*) >= 2
            ORDER BY abandonment_rate DESC, hits DESC, last_seen_at DESC
            LIMIT $4
            "#
        }
    }
}

fn read_i64(row: &sea_orm::QueryResult, column: &str) -> i64 {
    row.try_get::<i64>("", column).unwrap_or(0).max(0)
}

fn read_f64(row: &sea_orm::QueryResult, column: &str) -> f64 {
    row.try_get::<f64>("", column).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::{
        intelligence_score, normalize_query_text, recommend_action, SearchAnalyticsInsightRow,
        SearchAnalyticsQueryRow, SLOW_QUERY_THRESHOLD_MS,
    };
    use chrono::Utc;

    #[test]
    fn normalize_query_text_collapses_whitespace_and_case() {
        assert_eq!(
            normalize_query_text("  Hello   WORLD  "),
            "hello world".to_string()
        );
    }

    #[test]
    fn recommend_action_prioritizes_zero_results() {
        let row = SearchAnalyticsQueryRow {
            query: "iphone".to_string(),
            hits: 4,
            zero_result_hits: 4,
            clicks: 0,
            avg_took_ms: 10.0,
            avg_results: 0.0,
            click_through_rate: 0.0,
            abandonment_rate: 1.0,
            last_seen_at: Utc::now(),
        };

        assert_eq!(
            recommend_action(&row).as_deref(),
            Some("add_synonyms_or_redirect")
        );
    }

    #[test]
    fn intelligence_score_grows_for_problematic_high_volume_queries() {
        let low = SearchAnalyticsInsightRow {
            query: "a".to_string(),
            hits: 2,
            zero_result_hits: 0,
            clicks: 1,
            click_through_rate: 0.5,
            abandonment_rate: 0.2,
            recommendation: "tune_ranking_or_boost_exact_match".to_string(),
        };
        let high = SearchAnalyticsInsightRow {
            query: "b".to_string(),
            hits: 10,
            zero_result_hits: 5,
            clicks: 0,
            click_through_rate: 0.0,
            abandonment_rate: 1.0,
            recommendation: "add_synonyms_or_redirect".to_string(),
        };

        assert!(intelligence_score(&high) > intelligence_score(&low));
    }

    #[test]
    fn recommend_action_detects_slow_queries_after_relevance_issues() {
        let row = SearchAnalyticsQueryRow {
            query: "large catalog".to_string(),
            hits: 5,
            zero_result_hits: 0,
            clicks: 2,
            avg_took_ms: (SLOW_QUERY_THRESHOLD_MS + 25) as f64,
            avg_results: 12.0,
            click_through_rate: 0.5,
            abandonment_rate: 0.3,
            last_seen_at: Utc::now(),
        };

        assert_eq!(
            recommend_action(&row).as_deref(),
            Some("narrow_filters_or_review_indexes")
        );
    }
}
