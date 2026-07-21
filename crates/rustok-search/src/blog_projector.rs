use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_telemetry::metrics;

#[derive(Clone)]
pub(crate) struct BlogSearchProjector {
    db: DatabaseConnection,
}

impl BlogSearchProjector {
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub(crate) async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_tenant_documents_in(&tx, tenant_id).await?;
            if self.blog_tables_available(&tx).await? {
                self.upsert_documents_in(&tx, tenant_id, None).await?;
            }
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
            self.delete_post_in(&tx, tenant_id, post_id).await?;
            if self.blog_tables_available(&tx).await? {
                self.upsert_documents_in(&tx, tenant_id, Some(post_id))
                    .await?;
            }
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation("upsert_blog_post", tenant_id, &result, started_at.elapsed());
        result
    }

    pub(crate) async fn delete_post(&self, tenant_id: Uuid, post_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let result = self.delete_post_in(&self.db, tenant_id, post_id).await;
        record_projector_operation("delete_blog_post", tenant_id, &result, started_at.elapsed());
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

    async fn blog_tables_available<C>(&self, conn: &C) -> Result<bool>
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
                AND to_regclass('blog_category_translations') IS NOT NULL
                AS available
            "#
            .to_string(),
        );
        let available = conn
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .and_then(|row| row.try_get::<bool>("", "available").ok())
            .unwrap_or(false);
        Ok(available)
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
        conn.execute(stmt).await.map_err(Error::Database)?;
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
                bct.name AS subtitle,
                p.slug,
                NULL::text AS handle,
                CONCAT_WS(E'\n\n', COALESCE(bt.excerpt, ''), COALESCE(bt.body, '')) AS body,
                CONCAT_WS(
                    ' ',
                    COALESCE(bct.name, ''),
                    COALESCE(u.name, ''),
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
                    'category_name', bct.name,
                    'category_slug', bct.slug,
                    'author_id', p.author_id,
                    'author_name', u.name,
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
            LEFT JOIN blog_category_translations bct
                ON bct.category_id = p.category_id
               AND bct.tenant_id = p.tenant_id
               AND bct.locale = bt.locale
            LEFT JOIN users u
                ON u.id = p.author_id
            LEFT JOIN LATERAL (
                SELECT
                    COUNT(*)::bigint AS tag_count,
                    string_agg(tag.tag_name, ' ' ORDER BY tag.tag_name) AS tag_names,
                    COALESCE(jsonb_agg(tag.tag_name ORDER BY tag.tag_name), '[]'::jsonb) AS tag_list
                FROM (
                    SELECT DISTINCT BTRIM(tag_value) AS tag_name
                    FROM jsonb_array_elements_text(
                        CASE
                            WHEN jsonb_typeof(p.metadata -> 'tags') = 'array' THEN p.metadata -> 'tags'
                            ELSE '[]'::jsonb
                        END
                    ) AS tag_values(tag_value)
                    WHERE BTRIM(tag_value) <> ''
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
        conn.execute(stmt).await.map_err(Error::Database)?;
        Ok(())
    }
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
    }
}
