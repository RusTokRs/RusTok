-- Read-only Taxonomy route-registry drift audit for one tenant.
-- Usage:
--   psql "$DATABASE_URL" --set=ON_ERROR_STOP=1 \
--     --set=tenant_id='00000000-0000-0000-0000-000000000000' \
--     --file=crates/rustok-taxonomy/docs/sql/route-registry-drift.sql
--
-- The complete route identity is:
-- tenant_id + kind + scope_type + scope_value + locale + route_key.

WITH raw_localized_routes AS (
    SELECT
        t.tenant_id,
        t.id AS term_id,
        t.kind,
        t.scope_type,
        t.scope_value,
        tr.locale,
        tr.slug AS route_key,
        'translation'::text AS source
    FROM taxonomy_terms AS t
    JOIN taxonomy_term_translations AS tr
      ON tr.term_id = t.id
     AND tr.tenant_id = t.tenant_id
    WHERE t.tenant_id = :'tenant_id'::uuid

    UNION ALL

    SELECT
        t.tenant_id,
        t.id AS term_id,
        t.kind,
        t.scope_type,
        t.scope_value,
        a.locale,
        a.slug AS route_key,
        'alias'::text AS source
    FROM taxonomy_terms AS t
    JOIN taxonomy_term_aliases AS a
      ON a.term_id = t.id
     AND a.tenant_id = t.tenant_id
    WHERE t.tenant_id = :'tenant_id'::uuid
),
localized_routes AS (
    SELECT
        tenant_id,
        term_id,
        kind,
        scope_type,
        scope_value,
        locale,
        route_key,
        string_agg(DISTINCT source, ',' ORDER BY source) AS source
    FROM raw_localized_routes
    GROUP BY tenant_id, term_id, kind, scope_type, scope_value, locale, route_key
),
localized_state AS (
    SELECT
        lr.tenant_id,
        lr.term_id,
        lr.kind,
        lr.scope_type,
        lr.scope_value,
        lr.locale,
        lr.route_key,
        lr.source,
        rk.term_id AS registry_owner_term_id,
        CASE
            WHEN rk.term_id IS NULL THEN 'missing_reservation'
            WHEN rk.term_id <> lr.term_id THEN 'cross_term_collision'
            ELSE 'consistent'
        END AS registry_state
    FROM localized_routes AS lr
    LEFT JOIN taxonomy_term_route_keys AS rk
      ON rk.tenant_id = lr.tenant_id
     AND rk.kind = lr.kind
     AND rk.scope_type = lr.scope_type
     AND rk.scope_value = lr.scope_value
     AND rk.locale = lr.locale
     AND rk.route_key = lr.route_key
),
stale_registry AS (
    SELECT
        rk.tenant_id,
        rk.term_id,
        rk.kind,
        rk.scope_type,
        rk.scope_value,
        rk.locale,
        rk.route_key,
        'registry_only'::text AS source,
        rk.term_id AS registry_owner_term_id,
        'stale_reservation'::text AS registry_state
    FROM taxonomy_term_route_keys AS rk
    WHERE rk.tenant_id = :'tenant_id'::uuid
      AND NOT EXISTS (
          SELECT 1
          FROM localized_routes AS lr
          WHERE lr.tenant_id = rk.tenant_id
            AND lr.term_id = rk.term_id
            AND lr.kind = rk.kind
            AND lr.scope_type = rk.scope_type
            AND lr.scope_value = rk.scope_value
            AND lr.locale = rk.locale
            AND lr.route_key = rk.route_key
      )
)
SELECT
    tenant_id,
    term_id,
    kind,
    scope_type,
    scope_value,
    locale,
    route_key,
    source,
    registry_owner_term_id,
    registry_state
FROM localized_state

UNION ALL

SELECT
    tenant_id,
    term_id,
    kind,
    scope_type,
    scope_value,
    locale,
    route_key,
    source,
    registry_owner_term_id,
    registry_state
FROM stale_registry

ORDER BY kind, scope_type, scope_value, locale, route_key, term_id, registry_state;
