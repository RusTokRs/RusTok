use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    EntityName, ModuleName, PostgresQueryRootAdmission, SchemaRef, SchemaVersion,
    register_postgres_index_query_admission,
};

const PRODUCT_QUERY_MATERIALIZED_FRESHNESS: &str = r#"
EXISTS (
    SELECT 1
    FROM products owner_product
    JOIN LATERAL (
        SELECT
            projection.projection_epoch,
            projection.product_source_version,
            projection.relation_epoch
        FROM product_index_graph_projection_snapshots projection
        WHERE projection.tenant_id = {{root}}.tenant_id
          AND projection.product_id = {{root}}.entity_id
        ORDER BY projection.projection_epoch DESC
        LIMIT 1
    ) current_projection ON TRUE
    JOIN LATERAL (
        SELECT
            witness.product_source_version,
            witness.channel_identity_generation
        FROM product_sales_channel_index_relation_freshness_snapshots witness
        WHERE witness.tenant_id = {{root}}.tenant_id
          AND witness.product_id = {{root}}.entity_id
          AND witness.relation_epoch = current_projection.relation_epoch
        ORDER BY witness.sequence_no DESC
        LIMIT 1
    ) current_freshness ON TRUE
    WHERE owner_product.tenant_id = {{root}}.tenant_id
      AND owner_product.id = {{root}}.entity_id
      AND {{root}}.source_version = current_projection.projection_epoch
      AND current_projection.product_source_version = owner_product.index_revision
      AND current_freshness.product_source_version <= owner_product.index_revision
      AND current_freshness.channel_identity_generation = COALESCE(
          (
              SELECT channel_generation.generation
              FROM channel_index_identity_generations channel_generation
              WHERE channel_generation.tenant_id = {{root}}.tenant_id
          ),
          0
      )
      AND NOT EXISTS (
          SELECT 1
          FROM product_sales_channel_index_relation_convergence_requests request
          WHERE request.tenant_id = {{root}}.tenant_id
            AND request.product_id = {{root}}.entity_id
            AND request.product_source_version > current_freshness.product_source_version
      )
      AND EXISTS (
          SELECT 1
          FROM product_translations translation
          WHERE translation.tenant_id = {{root}}.tenant_id
            AND translation.product_id = {{root}}.entity_id
            AND translation.locale = {{root}}.locale_key
      )
)
"#;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }
    let admission = PostgresQueryRootAdmission::new(PRODUCT_QUERY_MATERIALIZED_FRESHNESS).map_err(
        |error| {
            rustok_core::Error::Validation(format!(
                "selected Product Index query admission construction failed: {error}"
            ))
        },
    )?;
    register_postgres_index_query_admission(
        extensions,
        "product",
        product_schema_ref()?,
        admission,
    )
    .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index query admission registration failed: {error}"
        ))
    })
}

fn product_schema_ref() -> rustok_core::Result<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new("rustok-product").map_err(|error| {
            rustok_core::Error::Validation(format!("Product Index module name is invalid: {error}"))
        })?,
        entity: EntityName::new("product").map_err(|error| {
            rustok_core::Error::Validation(format!("Product Index entity name is invalid: {error}"))
        })?,
        version: SchemaVersion::new(3),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialized_freshness_predicate_fences_owner_and_locale_state() {
        for marker in [
            "product_index_graph_projection_snapshots",
            "product_sales_channel_index_relation_freshness_snapshots",
            "channel_index_identity_generations",
            "product_sales_channel_index_relation_convergence_requests",
            "{{root}}.source_version = current_projection.projection_epoch",
            "current_projection.product_source_version = owner_product.index_revision",
            "request.product_source_version > current_freshness.product_source_version",
            "translation.locale = {{root}}.locale_key",
            "LIMIT 1",
        ] {
            assert!(
                PRODUCT_QUERY_MATERIALIZED_FRESHNESS.contains(marker),
                "missing {marker}"
            );
        }
        for forbidden in ["index_links", "index_entities", "$1", "channel_visibility"] {
            assert!(
                !PRODUCT_QUERY_MATERIALIZED_FRESHNESS.contains(forbidden),
                "forbidden {forbidden}"
            );
        }
    }
}
