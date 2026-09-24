use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

use rustok_api::{PLATFORM_FALLBACK_LOCALE, RichTextDocument};
use rustok_content::{RichTextProfile, plain_text};
use rustok_core::{Error, Result};
use rustok_telemetry::metrics;

#[derive(Clone)]
pub(crate) struct BlogSearchProjector {
    db: DatabaseConnection,
}

const AUTHOR_PROJECTION_REFRESH_SQL: &str = r#"
UPDATE search_documents AS sd
SET
    keywords_text = CONCAT_WS(
        ' ',
        COALESCE(sd.payload->>'category_name', ''),
        COALESCE((
            SELECT CASE
                WHEN LOWER(u.status::text) = 'active' THEN COALESCE(u.name, '')
                ELSE ''
            END
            FROM users AS u
            WHERE u.tenant_id = sd.tenant_id
              AND u.id = $2
        ), ''),
        COALESCE(sd.payload->>'seo_title', ''),
        COALESCE(sd.payload->>'seo_description', ''),
        COALESCE((
            SELECT string_agg(value, ' ' ORDER BY value)
            FROM jsonb_array_elements_text(
                CASE
                    WHEN jsonb_typeof(COALESCE(sd.payload->'tags', '[]'::jsonb)) = 'array'
                        THEN COALESCE(sd.payload->'tags', '[]'::jsonb)
                    ELSE '[]'::jsonb
                END
            ) AS tags(value)
        ), '')
    ),
    payload = jsonb_set(
        sd.payload,
        '{author_name}',
        COALESCE((
            SELECT CASE
                WHEN LOWER(u.status::text) = 'active'
                    THEN COALESCE(to_jsonb(u.name), 'null'::jsonb)
                ELSE 'null'::jsonb
            END
            FROM users AS u
            WHERE u.tenant_id = sd.tenant_id
              AND u.id = $2
        ), 'null'::jsonb),
        true
    ),
    indexed_at = NOW()
WHERE sd.tenant_id = $1
  AND sd.source_module = 'blog'
  AND sd.entity_type = 'blog_post'
  AND sd.payload->>'author_id' = $2::text
"#;

impl BlogSearchProjector {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.ensure_blog_tables_available(&tx).await?;
            self.delete_tenant_documents_in(&tx, tenant_id).await?;
            self.upsert_documents_in(&tx, tenant_id, None).await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "rebuild_blog_scope",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub(crate) async fn upsert_post(&self, tenant_id: Uuid, post_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.ensure_blog_tables_available(&tx).await?;
            self.delete_post_in(&tx, tenant_id, post_id).await?;
            self.upsert_documents_in(&tx, tenant_id, Some(post_id))
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation("upsert_blog_post", tenant_id, &result, started_at.elapsed());
        result
    }

    pub(crate) async fn refresh_author_projection(
        &self,
        tenant_id: Uuid,
        author_id: Uuid,
    ) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let result = self
            .refresh_author_projection_in(&self.db, tenant_id, author_id)
            .await;
        record_projector_operation(
            "refresh_blog_author_projection",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    async fn refresh_author_projection_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        author_id: Uuid,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let statement = Statement::from_sql_and_values(
            DbBackend::Postgres,
            AUTHOR_PROJECTION_REFRESH_SQL,
            vec![tenant_id.into(), author_id.into()],
        );
        conn.execute_raw(statement).await.map_err(Error::Database)?;
        Ok(())
    }

    pub(crate) async fn delete_post(&self, tenant_id: Uuid, post_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let result = self.delete_post_in(&self.db, tenant_id, post_id).await;
        record_projector_operation("delete_blog_post", tenant_id, &result, started_at.elapsed());
        result
    }

    pub(crate) async fn delete_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let result = self.delete_tenant_documents_in(&self.db, tenant_id).await;
        record_projector_operation(
            "delete_blog_scope",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    fn ensure_postgres(&self) -> Result<()> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "BlogSearchProjector requires PostgreSQL backend".to_string(),
            ));
        }
        Ok(())
    }

    async fn begin_transaction(&self) -> Result<DatabaseTransaction> {
        self.db.begin().await.map_err(Error::Database)
    }

    async fn commit_transaction(&self, tx: DatabaseTransaction) -> Result<()> {
        tx.commit().await.map_err(Error::Database)
    }

    async fn ensure_blog_tables_available<C>(&self, conn: &C) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let stmt = Statement::from_string(
            DbBackend::Postgres,
            r#"
            SELECT
                to_regclass('blog_posts') IS NOT NULL
                AND to_regclass('blog_post_translations') IS NOT NULL
                AND to_regclass('blog_post_channel_visibility') IS NOT NULL
                AND to_regclass('blog_post_tags') IS NOT NULL
                AND to_regclass('taxonomy_terms') IS NOT NULL
                AND to_regclass('taxonomy_term_translations') IS NOT NULL
                AS available
            "#
            .to_string(),
        );
        let row = conn
            .query_one_raw(stmt)
            .await
            .map_err(Error::Database)?
            .ok_or_else(|| {
                Error::External(
                    "Blog Search projection schema availability query returned no row"
                        .to_string(),
                )
            })?;
        let available = row
            .try_get::<bool>("", "available")
            .map_err(Error::Database)?;
        if !available {
            return Err(Error::External(
                "Blog Search projection source tables are unavailable".to_string(),
            ));
        }

        Ok(())
    }

    async fn delete_tenant_documents_in<C>(&self, conn: &C, tenant_id: Uuid) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.delete_documents_in(
            conn,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = 'blog' AND entity_type = 'blog_post'",
            vec![tenant_id.into()],
        )
        .await
    }

    async fn delete_post_in<C>(&self, conn: &C, tenant_id: Uuid, post_id: Uuid) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.delete_documents_in(
            conn,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND source_module = 'blog' AND entity_type = 'blog_post' AND document_id = $2",
            vec![tenant_id.into(), post_id.into()],
        )
        .await
    }

    async fn delete_documents_in<C>(
        &self,
        conn: &C,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        conn.execute_raw(stmt).await.map_err(Error::Database)?;
        Ok(())
    }

    async fn upsert_documents_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        post_id: Option<Uuid>,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let mut values = vec![tenant_id.into()];
        let mut where_clause = String::from("WHERE p.tenant_id = $1");
        if let Some(post_id) = post_id {
            where_clause.push_str(" AND p.id = $2");
            values.push(post_id.into());
        }
        let fallback_locale = PLATFORM_FALLBACK_LOCALE;

        let sql = format!(
            r#"
            INSERT INTO search_documents (
                document_key,
                tenant_id,
                document_id,
                source_module,
                entity_type,
                locale,
                status,
                is_public,
                title,
                subtitle,
                slug,
                handle,
                body,
                keywords_text,
                facets,
                payload,
                published_at,
                created_at,
                updated_at,
                indexed_at
            )
            SELECT
                CONCAT('blog_post:', p.id::text, ':', bt.locale) AS document_key,
                p.tenant_id,
                p.id AS document_id,
                'blog'::text AS source_module,
                'blog_post'::text AS entity_type,
                bt.locale,
                p.status::text AS status,
                (LOWER(p.status::text) = 'published') AS is_public,
                COALESCE(bt.title, '') AS title,
                COALESCE(bct.name, bct_fallback.name, bct_term.canonical_key) AS subtitle,
                p.slug,
                NULL::text AS handle,
                COALESCE(bt.excerpt, '') AS body,
                CONCAT_WS(
                    ' ',
                    COALESCE(bct.name, bct_fallback.name, bct_term.canonical_key, ''),
                    CASE
                        WHEN LOWER(u.status::text) = 'active' THEN COALESCE(u.name, '')
                        ELSE ''
                    END,
                    COALESCE(bt.seo_title, ''),
                    COALESCE(bt.seo_description, ''),
                    COALESCE(tags.tag_names, '')
                ) AS keywords_text,
                jsonb_build_object(
                    'has_category', (p.category_id IS NOT NULL),
                    'has_tags', (COALESCE(tags.tag_count, 0) > 0),
                    'has_channels', (COALESCE(channels.channel_count, 0) > 0),
                    'channel_slugs', COALESCE(channels.channel_slugs, '[]'::jsonb)
                ) AS facets,
                jsonb_build_object(
                    'slug', p.slug,
                    'excerpt', bt.excerpt,
                    'seo_title', bt.seo_title,
                    'seo_description', bt.seo_description,
                    'featured_image_url', p.featured_image_url,
                    'category_id', p.category_id,
                    'category_name', COALESCE(bct.name, bct_fallback.name, bct_term.canonical_key),
                    'category_slug', COALESCE(bct.slug, bct_fallback.slug, bct_term.canonical_key),
                    'author_id', p.author_id,
                    'author_name', CASE
                        WHEN LOWER(u.status::text) = 'active' THEN u.name
                        ELSE NULL
                    END,
                    'tags', COALESCE(tags.tag_list, '[]'::jsonb),
                    'channel_slugs', COALESCE(channels.channel_slugs, '[]'::jsonb),
                    'comment_count', p.comment_count,
                    'view_count', p.view_count,
                    'version', p.version,
                    'published_at', p.published_at,
                    'archived_at', p.archived_at
                ) AS payload,
                p.published_at,
                p.created_at,
                GREATEST(p.updated_at, bt.updated_at) AS updated_at,
                NOW()
            FROM blog_posts p
            JOIN blog_post_translations bt
                ON bt.post_id = p.id
            LEFT JOIN taxonomy_terms bct_term
                ON bct_term.id = p.category_id
               AND bct_term.tenant_id = p.tenant_id
               AND bct_term.kind = 'category'
               AND bct_term.scope_type = 'module'
               AND bct_term.scope_value = 'blog'
            LEFT JOIN taxonomy_term_translations bct
                ON bct.term_id = p.category_id
               AND bct.tenant_id = p.tenant_id
               AND bct.locale = bt.locale
               AND bct_term.id IS NOT NULL
            LEFT JOIN taxonomy_term_translations bct_fallback
                ON bct_fallback.term_id = p.category_id
               AND bct_fallback.tenant_id = p.tenant_id
               AND bct_fallback.locale = '{fallback_locale}'
               AND bct_term.id IS NOT NULL
            LEFT JOIN users u
                ON u.id = p.author_id
               AND u.tenant_id = p.tenant_id
            LEFT JOIN LATERAL (
                SELECT
                    COUNT(*)::bigint AS tag_count,
                    string_agg(tag.tag_name, ' ' ORDER BY tag.tag_name) AS tag_names,
                    COALESCE(jsonb_agg(tag.tag_name ORDER BY tag.tag_name), '[]'::jsonb) AS tag_list
                FROM (
                    SELECT DISTINCT BTRIM(
                        COALESCE(localized.name, fallback.name, term.canonical_key)
                    ) AS tag_name
                    FROM blog_post_tags relation
                    JOIN taxonomy_terms term
                      ON term.id = relation.tag_id
                     AND term.tenant_id = p.tenant_id
                     AND term.kind = 'tag'
                     AND (
                         term.scope_type = 'global'
                         OR (
                             term.scope_type = 'module'
                             AND term.scope_value = 'blog'
                         )
                     )
                    LEFT JOIN taxonomy_term_translations localized
                      ON localized.term_id = term.id
                     AND localized.tenant_id = p.tenant_id
                     AND localized.locale = bt.locale
                    LEFT JOIN taxonomy_term_translations fallback
                      ON fallback.term_id = term.id
                     AND fallback.tenant_id = p.tenant_id
                     AND fallback.locale = '{fallback_locale}'
                    WHERE relation.post_id = p.id
                      AND relation.tenant_id = p.tenant_id
                      AND BTRIM(
                          COALESCE(localized.name, fallback.name, term.canonical_key)
                      ) <> ''
                ) tag
            ) tags ON TRUE
            LEFT JOIN LATERAL (
                SELECT
                    COUNT(*)::bigint AS channel_count,
                    COALESCE(
                        jsonb_agg(visibility.channel_slug ORDER BY visibility.channel_slug),
                        '[]'::jsonb
                    ) AS channel_slugs
                FROM blog_post_channel_visibility visibility
                WHERE visibility.tenant_id = p.tenant_id
                  AND visibility.post_id = p.id
            ) channels ON TRUE
            {where_clause}
            ON CONFLICT (document_key) DO UPDATE SET
                status = EXCLUDED.status,
                is_public = EXCLUDED.is_public,
                title = EXCLUDED.title,
                subtitle = EXCLUDED.subtitle,
                slug = EXCLUDED.slug,
                handle = EXCLUDED.handle,
                body = EXCLUDED.body,
                keywords_text = EXCLUDED.keywords_text,
                facets = EXCLUDED.facets,
                payload = EXCLUDED.payload,
                published_at = EXCLUDED.published_at,
                updated_at = EXCLUDED.updated_at,
                indexed_at = NOW()
            "#
        );

        let stmt = Statement::from_sql_and_values(DbBackend::Postgres, sql, values);
        conn.execute_raw(stmt).await.map_err(Error::Database)?;
        self.refresh_article_bodies_in(conn, tenant_id, post_id)
            .await?;
        Ok(())
    }

    async fn refresh_article_bodies_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        post_id: Option<Uuid>,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        const BATCH_SIZE: i64 = 128;
        let mut cursor: Option<String> = None;

        loop {
            let mut values = vec![tenant_id.into()];
            let mut where_clause = String::from("WHERE p.tenant_id = $1");
            let mut next_parameter = 2;

            if let Some(post_id) = post_id {
                where_clause.push_str(" AND p.id = $2");
                values.push(post_id.into());
                next_parameter = 3;
            }

            if let Some(cursor) = cursor.as_deref() {
                where_clause.push_str(&format!(" AND CONCAT('blog_post:', p.id::text, ':', bt.locale) > ${next_parameter}"));
                values.push(cursor.to_owned().into());
                next_parameter += 1;
            }

            let limit_parameter = next_parameter;
            values.push(BATCH_SIZE.into());

            let sql = format!(
                r#"
                SELECT
                    CONCAT('blog_post:', p.id::text, ':', bt.locale) AS document_key,
                    bt.excerpt,
                    bt.body
                FROM blog_posts p
                JOIN blog_post_translations bt
                    ON bt.post_id = p.id
                {where_clause}
                ORDER BY CONCAT('blog_post:', p.id::text, ':', bt.locale) ASC
                LIMIT ${limit_parameter}
                "#
            );

            let rows = conn
                .query_all_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    sql,
                    values,
                ))
                .await
                .map_err(Error::Database)?;

            if rows.is_empty() {
                break;
            }

            for row in &rows {
                let document_key = row
                    .try_get::<String>("", "document_key")
                    .map_err(Error::Database)?;
                let excerpt = row
                    .try_get::<Option<String>>("", "excerpt")
                    .map_err(Error::Database)?;
                let body = row
                    .try_get::<String>("", "body")
                    .map_err(Error::Database)?;
                let article_text = project_canonical_article_plain_text(&body)?;
                let search_body = compose_search_body(excerpt.as_deref(), &article_text);

                conn.execute_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "UPDATE search_documents SET body = $1 WHERE tenant_id = $2 AND document_key = $3 AND source_module = 'blog' AND entity_type = 'blog_post'",
                    vec![search_body.into(), tenant_id.into(), document_key.into()],
                ))
                .await
                .map_err(Error::Database)?;
            }

            let last_row = rows.last().ok_or_else(|| {
                Error::Internal("Blog Search body refresh returned an empty batch".to_string())
            })?;
            cursor = Some(
                last_row
                    .try_get::<String>("", "document_key")
                    .map_err(Error::Database)?,
            );

            if rows.len() < BATCH_SIZE as usize {
                break;
            }
        }

        Ok(())
    }
}

fn project_canonical_article_plain_text(body: &str) -> Result<String> {
    let document: RichTextDocument = serde_json::from_str(body).map_err(|error| {
        Error::Validation(format!(
            "Stored Blog article content is not a RichTextDocument: {error}"
        ))
    })?;
    plain_text(&document, RichTextProfile::Article)
        .map_err(|error| Error::Validation(error.to_string()))
}

fn compose_search_body(excerpt: Option<&str>, article_text: &str) -> String {
    [excerpt.unwrap_or_default(), article_text]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn record_projector_operation(
    operation: &str,
    tenant_id: Uuid,
    result: &Result<()>,
    duration: Duration,
) {
    let status = if result.is_ok() { "success" } else { "error" };
    metrics::record_search_indexing_operation(
        operation,
        "blog_post",
        status,
        duration.as_secs_f64(),
    );

    if let Err(error) = result {
        metrics::record_module_error("search", classify_error(error), "error");
        tracing::error!(
            operation,
            entity = "blog_post",
            tenant_id = %tenant_id,
            error = %error,
            duration_ms = duration.as_millis() as u64,
            "Blog search projector operation failed"
        );
    } else {
        tracing::info!(
            operation,
            entity = "blog_post",
            tenant_id = %tenant_id,
            duration_ms = duration.as_millis() as u64,
            "Blog search projector operation completed"
        );
    }
}

fn classify_error(error: &Error) -> &'static str {
    match error {
        Error::Database(_) => "database",
        Error::Validation(_) => "validation",
        Error::External(_) => "external",
        Error::NotFound(_) => "not_found",
        Error::Forbidden(_) => "forbidden",
        Error::Auth(_) => "auth",
        Error::Cache(_) => "cache",
        Error::Serialization(_) => "serialization",
        Error::Scripting(_) => "scripting",
        Error::InvalidIdFormat(_) => "invalid_id",
        Error::Internal(_) => "internal",
    }
}

#[cfg(test)]
mod tests {
    use rustok_api::RichTextDocument;

    use super::{
        compose_search_body, project_canonical_article_plain_text, AUTHOR_PROJECTION_REFRESH_SQL,
    };

    #[test]
    fn author_projection_refresh_is_tenant_scoped_and_in_place() {
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("UPDATE search_documents AS sd"));
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("sd.tenant_id = $1"));
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("sd.payload->>'author_id' = $2::text"));
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("sd.source_module = 'blog'"));
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("sd.entity_type = 'blog_post'"));
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("LOWER(u.status::text) = 'active'"));
        assert!(AUTHOR_PROJECTION_REFRESH_SQL.contains("payload = jsonb_set"));
        assert!(!AUTHOR_PROJECTION_REFRESH_SQL.contains("INSERT INTO search_documents"));
        assert!(!AUTHOR_PROJECTION_REFRESH_SQL.contains("DELETE FROM search_documents"));
    }

    #[test]
    fn canonical_article_search_text_uses_article_policy() {
        let body = serde_json::to_string(&RichTextDocument::single_paragraph("Indexed article"))
            .expect("serialize canonical article");

        assert_eq!(
            project_canonical_article_plain_text(&body).expect("article plain text"),
            "Indexed article"
        );
    }

    #[test]
    fn search_body_composition_omits_blank_sections() {
        assert_eq!(
            compose_search_body(Some("  Summary  "), "  Article body  "),
            "Summary\n\nArticle body"
        );
        assert_eq!(
            compose_search_body(None, "  Article body  "),
            "Article body"
        );
        assert_eq!(compose_search_body(Some("  "), "  "), "");
    }
}
