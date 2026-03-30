use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

use rustok_core::{Error, Result};
use rustok_telemetry::metrics;

#[derive(Clone)]
pub struct SearchProjector {
    db: DatabaseConnection,
}

impl SearchProjector {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn ensure_bootstrap(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;

        let stmt = Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS total FROM search_documents WHERE tenant_id = $1",
            vec![tenant_id.into()],
        );

        let total = self
            .db
            .query_one(stmt)
            .await
            .map_err(Error::Database)?
            .and_then(|row| row.try_get::<i64>("", "total").ok())
            .unwrap_or(0);

        if total == 0 {
            self.rebuild_tenant(tenant_id).await?;
        }

        Ok(())
    }

    pub async fn rebuild_tenant(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_tenant_documents_in(&tx, tenant_id).await?;
            self.upsert_content_documents_in(&tx, tenant_id, None, None, None)
                .await?;
            self.upsert_product_documents_in(&tx, tenant_id, None)
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "rebuild_tenant",
            "tenant",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn rebuild_content_scope(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_documents_in(
                &tx,
                "DELETE FROM search_documents WHERE tenant_id = $1 AND entity_type = 'node'",
                vec![tenant_id.into()],
            )
            .await?;
            self.upsert_content_documents_in(&tx, tenant_id, None, None, None)
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "rebuild_content_scope",
            "node",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn rebuild_product_scope(&self, tenant_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_documents_in(
                &tx,
                "DELETE FROM search_documents WHERE tenant_id = $1 AND entity_type = 'product'",
                vec![tenant_id.into()],
            )
            .await?;
            self.upsert_product_documents_in(&tx, tenant_id, None)
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "rebuild_product_scope",
            "product",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn upsert_node(&self, tenant_id: Uuid, node_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_node_in(&tx, tenant_id, node_id).await?;
            self.upsert_content_documents_in(&tx, tenant_id, Some(node_id), None, None)
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "upsert_node",
            "node",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn upsert_node_locale(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        locale: &str,
    ) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_node_locale_in(&tx, tenant_id, node_id, locale)
                .await?;
            self.upsert_content_documents_in(&tx, tenant_id, Some(node_id), Some(locale), None)
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "upsert_node_locale",
            "node",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn delete_node(&self, tenant_id: Uuid, node_id: Uuid) -> Result<()> {
        self.delete_node_in(&self.db, tenant_id, node_id).await
    }

    pub async fn delete_node_locale(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
        locale: &str,
    ) -> Result<()> {
        self.delete_node_locale_in(&self.db, tenant_id, node_id, locale)
            .await
    }

    pub async fn reindex_category(&self, tenant_id: Uuid, category_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_documents_in(
                &tx,
                r#"
                DELETE FROM search_documents
                WHERE tenant_id = $1
                  AND entity_type = 'node'
                  AND document_id IN (
                      SELECT id FROM nodes
                      WHERE tenant_id = $1
                        AND category_id = $2
                        AND deleted_at IS NULL
                  )
                "#,
                vec![tenant_id.into(), category_id.into()],
            )
            .await?;
            self.upsert_content_documents_in(&tx, tenant_id, None, None, Some(category_id))
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "reindex_category",
            "node",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn upsert_product(&self, tenant_id: Uuid, product_id: Uuid) -> Result<()> {
        self.ensure_postgres()?;
        let started_at = Instant::now();
        let tx = self.begin_transaction().await?;
        let result = async {
            self.delete_product_in(&tx, tenant_id, product_id).await?;
            self.upsert_product_documents_in(&tx, tenant_id, Some(product_id))
                .await?;
            self.commit_transaction(tx).await
        }
        .await;
        record_projector_operation(
            "upsert_product",
            "product",
            tenant_id,
            &result,
            started_at.elapsed(),
        );
        result
    }

    pub async fn delete_product(&self, tenant_id: Uuid, product_id: Uuid) -> Result<()> {
        self.delete_product_in(&self.db, tenant_id, product_id)
            .await
    }

    fn ensure_postgres(&self) -> Result<()> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(Error::External(
                "SearchProjector requires PostgreSQL backend".to_string(),
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

    async fn delete_tenant_documents_in<C>(&self, conn: &C, tenant_id: Uuid) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.delete_documents_in(
            conn,
            "DELETE FROM search_documents WHERE tenant_id = $1",
            vec![tenant_id.into()],
        )
        .await
    }

    async fn delete_node_in<C>(&self, conn: &C, tenant_id: Uuid, node_id: Uuid) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.delete_documents_in(
            conn,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND entity_type = 'node' AND document_id = $2",
            vec![tenant_id.into(), node_id.into()],
        )
        .await
    }

    async fn delete_node_locale_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        node_id: Uuid,
        locale: &str,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.delete_documents_in(
            conn,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND entity_type = 'node' AND document_id = $2 AND locale = $3",
            vec![tenant_id.into(), node_id.into(), locale.to_string().into()],
        )
        .await
    }

    async fn delete_product_in<C>(&self, conn: &C, tenant_id: Uuid, product_id: Uuid) -> Result<()>
    where
        C: ConnectionTrait,
    {
        self.delete_documents_in(
            conn,
            "DELETE FROM search_documents WHERE tenant_id = $1 AND entity_type = 'product' AND document_id = $2",
            vec![tenant_id.into(), product_id.into()],
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

    async fn upsert_content_documents_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        node_id: Option<Uuid>,
        locale: Option<&str>,
        category_id: Option<Uuid>,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let mut values = vec![tenant_id.into()];
        let mut param = 2;
        let mut where_clause = String::from("WHERE n.tenant_id = $1 AND n.deleted_at IS NULL");

        if let Some(node_id) = node_id {
            where_clause.push_str(&format!(" AND n.id = ${param}"));
            values.push(node_id.into());
            param += 1;
        }

        if let Some(locale) = locale {
            where_clause.push_str(&format!(" AND nt.locale = ${param}"));
            values.push(locale.to_string().into());
            param += 1;
        }

        if let Some(category_id) = category_id {
            where_clause.push_str(&format!(" AND n.category_id = ${param}"));
            values.push(category_id.into());
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
                CONCAT('node:', n.id::text, ':', nt.locale) AS document_key,
                n.tenant_id,
                n.id AS document_id,
                COALESCE(NULLIF(n.kind::text, ''), 'content') AS source_module,
                'node'::text AS entity_type,
                nt.locale,
                n.status::text AS status,
                (LOWER(n.status::text) = 'published') AS is_public,
                COALESCE(nt.title, '') AS title,
                ct.name AS subtitle,
                nt.slug,
                NULL::text AS handle,
                CONCAT_WS(E'\n\n', COALESCE(nt.excerpt, ''), COALESCE(b.body, '')) AS body,
                CONCAT_WS(
                    ' ',
                    COALESCE(ct.name, ''),
                    COALESCE(u.name, ''),
                    COALESCE(tags.tag_names, '')
                ) AS keywords_text,
                jsonb_build_object(
                    'has_category', (ct.slug IS NOT NULL),
                    'has_tags', (COALESCE(tags.tag_count, 0) > 0)
                ) AS facets,
                jsonb_build_object(
                    'slug', nt.slug,
                    'excerpt', nt.excerpt,
                    'category_id', n.category_id,
                    'category_name', ct.name,
                    'category_slug', ct.slug,
                    'author_name', u.name,
                    'tags', COALESCE(tags.tag_list, '[]'::jsonb),
                    'published_at', n.published_at
                ) AS payload,
                n.published_at,
                n.created_at,
                n.updated_at,
                NOW()
            FROM nodes n
            JOIN node_translations nt
                ON nt.node_id = n.id
            LEFT JOIN bodies b
                ON b.node_id = n.id AND b.locale = nt.locale
            LEFT JOIN category_translations ct
                ON ct.category_id = n.category_id AND ct.locale = nt.locale
            LEFT JOIN users u
                ON u.id = n.author_id
            LEFT JOIN LATERAL (
                SELECT
                    COUNT(*)::bigint AS tag_count,
                    string_agg(tag.tag_name, ' ') AS tag_names,
                    COALESCE(
                        jsonb_agg(
                            jsonb_build_object(
                                'id',
                                LOWER(
                                    SUBSTRING(md5(tag.slug) FROM 1 FOR 8) || '-' ||
                                    SUBSTRING(md5(tag.slug) FROM 9 FOR 4) || '-' ||
                                    SUBSTRING(md5(tag.slug) FROM 13 FOR 4) || '-' ||
                                    SUBSTRING(md5(tag.slug) FROM 17 FOR 4) || '-' ||
                                    SUBSTRING(md5(tag.slug) FROM 21 FOR 12)
                                ),
                                'name', tag.tag_name,
                                'slug', tag.slug
                            )
                            ORDER BY tag.tag_name
                        ) FILTER (WHERE tag.slug <> ''),
                        '[]'::jsonb
                    ) AS tag_list
                FROM (
                    SELECT DISTINCT
                        BTRIM(tag_value) AS tag_name,
                        LOWER(
                            BTRIM(
                                REGEXP_REPLACE(
                                    BTRIM(tag_value),
                                    '[^[:alnum:]]+',
                                    '-',
                                    'g'
                                ),
                                '-'
                            )
                        ) AS slug
                    FROM jsonb_array_elements_text(
                        CASE
                            WHEN jsonb_typeof(n.metadata -> 'tags') = 'array' THEN n.metadata -> 'tags'
                            ELSE '[]'::jsonb
                        END
                    ) AS tag_values(tag_value)
                    WHERE BTRIM(tag_value) <> ''
                ) tag
                WHERE tag.slug <> ''
            ) tags ON TRUE
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

    async fn upsert_product_documents_in<C>(
        &self,
        conn: &C,
        tenant_id: Uuid,
        product_id: Option<Uuid>,
    ) -> Result<()>
    where
        C: ConnectionTrait,
    {
        let mut values = vec![tenant_id.into()];
        let mut where_clause = String::from("WHERE p.tenant_id = $1");

        if let Some(product_id) = product_id {
            where_clause.push_str(" AND p.id = $2");
            values.push(product_id.into());
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
                CONCAT('product:', p.id::text, ':', pt.locale) AS document_key,
                p.tenant_id,
                p.id AS document_id,
                'commerce'::text AS source_module,
                'product'::text AS entity_type,
                pt.locale,
                p.status::text AS status,
                (LOWER(p.status::text) = 'active') AS is_public,
                COALESCE(pt.title, '') AS title,
                p.vendor AS subtitle,
                NULL::text AS slug,
                pt.handle,
                COALESCE(pt.description, '') AS body,
                CONCAT_WS(
                    ' ',
                    COALESCE(p.vendor, ''),
                    COALESCE(pt.meta_title, ''),
                    COALESCE(pt.meta_description, '')
                ) AS keywords_text,
                jsonb_build_object(
                    'in_stock', COALESCE(agg.in_stock, false),
                    'has_price', (agg.price_min IS NOT NULL OR agg.price_max IS NOT NULL)
                ) AS facets,
                jsonb_build_object(
                    'handle', pt.handle,
                    'description', pt.description,
                    'vendor', p.vendor,
                    'price_min', agg.price_min,
                    'price_max', agg.price_max,
                    'in_stock', COALESCE(agg.in_stock, false),
                    'variant_count', COALESCE(agg.variant_count, 0),
                    'published_at', p.published_at
                ) AS payload,
                p.published_at,
                p.created_at,
                p.updated_at,
                NOW()
            FROM products p
            JOIN product_translations pt
                ON pt.product_id = p.id
            LEFT JOIN LATERAL (
                SELECT
                    COUNT(pv.id)::bigint AS variant_count,
                    COALESCE(SUM(inv.available_quantity), 0) > 0 AS in_stock,
                    MIN(price_agg.min_amount)::bigint AS price_min,
                    MAX(price_agg.max_amount)::bigint AS price_max
                FROM product_variants pv
                LEFT JOIN LATERAL (
                    SELECT COALESCE(SUM(il.stocked_quantity - il.reserved_quantity), 0) AS available_quantity
                    FROM inventory_items ii
                    LEFT JOIN inventory_levels il ON il.inventory_item_id = ii.id
                    WHERE ii.variant_id = pv.id
                ) inv ON TRUE
                LEFT JOIN LATERAL (
                    SELECT
                        MIN(pr.amount) AS min_amount,
                        MAX(pr.amount) AS max_amount
                    FROM prices pr
                    WHERE pr.variant_id = pv.id
                ) price_agg ON TRUE
                WHERE pv.product_id = p.id
                  AND pv.tenant_id = p.tenant_id
            ) agg ON TRUE
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
    entity: &str,
    tenant_id: Uuid,
    result: &Result<()>,
    duration: Duration,
) {
    let status = if result.is_ok() { "success" } else { "error" };
    metrics::record_search_indexing_operation(operation, entity, status, duration.as_secs_f64());

    if let Err(error) = result {
        metrics::record_module_error("search", classify_error(error), "error");
        tracing::error!(
            operation,
            entity,
            tenant_id = %tenant_id,
            error = %error,
            duration_ms = duration.as_millis() as u64,
            "Search projector operation failed"
        );
    } else {
        tracing::info!(
            operation,
            entity,
            tenant_id = %tenant_id,
            duration_ms = duration.as_millis() as u64,
            "Search projector operation completed"
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
