-- Run after the Product Index graph trigger migration, one tenant and page at a time.
PREPARE repair_product_index_graph (uuid, uuid) AS
WITH page AS MATERIALIZED (
    SELECT tenant_id, id, index_revision
    FROM products
    WHERE tenant_id = $1 AND id > $2
    ORDER BY id
    LIMIT 250
),
reconciled AS MATERIALIZED (
    SELECT rustok_product_reconcile_index_graph_projection(page.tenant_id, page.id)
    FROM page
    LEFT JOIN LATERAL (
        SELECT product_source_version
        FROM product_index_graph_projection_snapshots
        WHERE tenant_id = page.tenant_id AND product_id = page.id
        ORDER BY projection_epoch DESC
        LIMIT 1
    ) latest ON TRUE
    WHERE (latest.product_source_version IS NULL OR latest.product_source_version < page.index_revision)
      AND EXISTS (
          SELECT 1
          FROM product_sales_channel_index_relation_snapshots
          WHERE tenant_id = page.tenant_id AND product_id = page.id
      )
    ORDER BY page.id
)
SELECT
    (SELECT id FROM page ORDER BY id DESC LIMIT 1) AS next_cursor,
    (SELECT count(*) FROM reconciled) AS candidates;
