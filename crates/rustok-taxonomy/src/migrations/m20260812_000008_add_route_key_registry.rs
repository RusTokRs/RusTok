use std::collections::BTreeMap;

use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RouteIdentity {
    tenant_id: Uuid,
    kind: String,
    scope_type: String,
    scope_value: String,
    locale: String,
    route_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteOwnership {
    identity: RouteIdentity,
    term_id: Uuid,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        let backend = connection.get_database_backend();

        if let Some(row) = connection
            .query_one(Statement::from_string(
                backend,
                tenant_mismatch_query().to_string(),
            ))
            .await?
        {
            let source_kind = row.try_get::<String>("", "source_kind")?;
            let stored_tenant_id = row.try_get::<Uuid>("", "stored_tenant_id")?;
            let term_tenant_id = row.try_get::<Uuid>("", "term_tenant_id")?;
            let term_id = row.try_get::<Uuid>("", "term_id")?;
            let locale = row.try_get::<String>("", "locale")?;
            let route_key = row.try_get::<String>("", "route_key")?;
            return Err(DbErr::Migration(format!(
                "taxonomy route-key registry backfill blocked by tenant mismatch: source={source_kind} term_id={term_id} stored_tenant_id={stored_tenant_id} term_tenant_id={term_tenant_id} locale={locale} route_key={route_key}",
            )));
        }

        let rows = connection
            .query_all(Statement::from_string(
                backend,
                route_source_query().to_string(),
            ))
            .await?;
        let ownerships = rows
            .iter()
            .map(route_ownership_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        validate_route_ownerships(&ownerships)?;

        manager
            .create_table(
                Table::create()
                    .table(TaxonomyTermRouteKeys::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::Kind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::ScopeType)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::ScopeValue)
                            .string_len(64)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::RouteKey)
                            .string_len(120)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TaxonomyTermRouteKeys::TermId)
                            .uuid()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_taxonomy_term_route_keys")
                            .col(TaxonomyTermRouteKeys::TenantId)
                            .col(TaxonomyTermRouteKeys::Kind)
                            .col(TaxonomyTermRouteKeys::ScopeType)
                            .col(TaxonomyTermRouteKeys::ScopeValue)
                            .col(TaxonomyTermRouteKeys::Locale)
                            .col(TaxonomyTermRouteKeys::RouteKey),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_taxonomy_term_route_keys_tenant_term")
                            .from_tbl(TaxonomyTermRouteKeys::Table)
                            .from_col(TaxonomyTermRouteKeys::TenantId)
                            .from_col(TaxonomyTermRouteKeys::TermId)
                            .to_tbl(TaxonomyTerms::Table)
                            .to_col(TaxonomyTerms::TenantId)
                            .to_col(TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_taxonomy_term_route_keys_term_locale")
                    .table(TaxonomyTermRouteKeys::Table)
                    .col(TaxonomyTermRouteKeys::TenantId)
                    .col(TaxonomyTermRouteKeys::TermId)
                    .col(TaxonomyTermRouteKeys::Locale)
                    .to_owned(),
            )
            .await?;

        connection
            .execute_unprepared(
                r#"
INSERT INTO taxonomy_term_route_keys
    (tenant_id, kind, scope_type, scope_value, locale, route_key, term_id)
SELECT tenant_id, kind, scope_type, scope_value, locale, route_key, term_id
FROM (
    SELECT DISTINCT
        t.tenant_id AS tenant_id,
        t.kind AS kind,
        t.scope_type AS scope_type,
        t.scope_value AS scope_value,
        tr.locale AS locale,
        tr.slug AS route_key,
        tr.term_id AS term_id
    FROM taxonomy_term_translations tr
    INNER JOIN taxonomy_terms t
        ON t.id = tr.term_id
       AND t.tenant_id = tr.tenant_id

    UNION

    SELECT DISTINCT
        t.tenant_id AS tenant_id,
        t.kind AS kind,
        t.scope_type AS scope_type,
        t.scope_value AS scope_value,
        a.locale AS locale,
        a.slug AS route_key,
        a.term_id AS term_id
    FROM taxonomy_term_aliases a
    INNER JOIN taxonomy_terms t
        ON t.id = a.term_id
       AND t.tenant_id = a.tenant_id
) route_rows;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TaxonomyTermRouteKeys::Table).to_owned())
            .await
    }
}

fn tenant_mismatch_query() -> &'static str {
    r#"
SELECT source_kind, stored_tenant_id, term_tenant_id, term_id, locale, route_key
FROM (
    SELECT
        'translation' AS source_kind,
        tr.tenant_id AS stored_tenant_id,
        t.tenant_id AS term_tenant_id,
        tr.term_id AS term_id,
        tr.locale AS locale,
        tr.slug AS route_key
    FROM taxonomy_term_translations tr
    INNER JOIN taxonomy_terms t ON t.id = tr.term_id
    WHERE tr.tenant_id <> t.tenant_id

    UNION ALL

    SELECT
        'alias' AS source_kind,
        a.tenant_id AS stored_tenant_id,
        t.tenant_id AS term_tenant_id,
        a.term_id AS term_id,
        a.locale AS locale,
        a.slug AS route_key
    FROM taxonomy_term_aliases a
    INNER JOIN taxonomy_terms t ON t.id = a.term_id
    WHERE a.tenant_id <> t.tenant_id
) mismatches
ORDER BY source_kind, stored_tenant_id, term_id, locale, route_key
LIMIT 1
"#
}

fn route_source_query() -> &'static str {
    r#"
SELECT
    t.tenant_id AS tenant_id,
    t.kind AS kind,
    t.scope_type AS scope_type,
    t.scope_value AS scope_value,
    route_rows.locale AS locale,
    route_rows.route_key AS route_key,
    route_rows.term_id AS term_id
FROM (
    SELECT term_id, tenant_id, locale, slug AS route_key
    FROM taxonomy_term_translations
    UNION ALL
    SELECT term_id, tenant_id, locale, slug AS route_key
    FROM taxonomy_term_aliases
) route_rows
INNER JOIN taxonomy_terms t
    ON t.id = route_rows.term_id
   AND t.tenant_id = route_rows.tenant_id
ORDER BY
    t.tenant_id,
    t.kind,
    t.scope_type,
    t.scope_value,
    route_rows.locale,
    route_rows.route_key,
    route_rows.term_id
"#
}

fn route_ownership_from_row(row: &sea_orm::QueryResult) -> Result<RouteOwnership, DbErr> {
    Ok(RouteOwnership {
        identity: RouteIdentity {
            tenant_id: row.try_get::<Uuid>("", "tenant_id")?,
            kind: row.try_get::<String>("", "kind")?,
            scope_type: row.try_get::<String>("", "scope_type")?,
            scope_value: row.try_get::<String>("", "scope_value")?,
            locale: row.try_get::<String>("", "locale")?,
            route_key: row.try_get::<String>("", "route_key")?,
        },
        term_id: row.try_get::<Uuid>("", "term_id")?,
    })
}

fn validate_route_ownerships(ownerships: &[RouteOwnership]) -> Result<(), DbErr> {
    let mut owners = BTreeMap::<RouteIdentity, Uuid>::new();
    for ownership in ownerships {
        match owners.get(&ownership.identity) {
            Some(existing_term_id) if *existing_term_id != ownership.term_id => {
                let identity = &ownership.identity;
                return Err(DbErr::Migration(format!(
                    "taxonomy route-key registry backfill blocked by ambiguous route: tenant={} kind={} scope_type={} scope_value={} locale={} route_key={} term_ids={},{}",
                    identity.tenant_id,
                    identity.kind,
                    identity.scope_type,
                    identity.scope_value,
                    identity.locale,
                    identity.route_key,
                    existing_term_id,
                    ownership.term_id,
                )));
            }
            Some(_) => {}
            None => {
                owners.insert(ownership.identity.clone(), ownership.term_id);
            }
        }
    }
    Ok(())
}

#[derive(DeriveIden)]
enum TaxonomyTermRouteKeys {
    Table,
    TenantId,
    Kind,
    ScopeType,
    ScopeValue,
    Locale,
    RouteKey,
    TermId,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    TenantId,
    Id,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ownership(term_id: Uuid, route_key: &str) -> RouteOwnership {
        RouteOwnership {
            identity: RouteIdentity {
                tenant_id: Uuid::from_u128(1),
                kind: "tag".to_string(),
                scope_type: "module".to_string(),
                scope_value: "blog".to_string(),
                locale: "en".to_string(),
                route_key: route_key.to_string(),
            },
            term_id,
        }
    }

    #[test]
    fn backfill_allows_translation_and_alias_for_same_term() {
        let term_id = Uuid::from_u128(10);
        validate_route_ownerships(&[ownership(term_id, "systems"), ownership(term_id, "systems")])
            .expect("same-term route representations must deduplicate during backfill");
    }

    #[test]
    fn backfill_rejects_cross_term_route_collision() {
        let error = validate_route_ownerships(&[
            ownership(Uuid::from_u128(10), "systems"),
            ownership(Uuid::from_u128(11), "systems"),
        ])
        .expect_err("different terms cannot own one localized route key");

        let message = error.to_string();
        assert!(message.contains("route_key=systems"));
        assert!(message.contains(&Uuid::from_u128(10).to_string()));
        assert!(message.contains(&Uuid::from_u128(11).to_string()));
    }
}
