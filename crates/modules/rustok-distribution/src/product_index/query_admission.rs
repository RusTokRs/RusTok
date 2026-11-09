use rustok_core::ModuleRuntimeExtensions;
use rustok_index::{
    EntityName, ModuleName, PostgresQueryEntityAdmission, SchemaRef, SchemaVersion,
    register_postgres_index_query_admission,
    register_postgres_index_query_link_target_availability,
};

use rustok_channel::index_projection::SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS;
use rustok_product::index_projection::{
    PRODUCT_QUERY_MATERIALIZED_FRESHNESS, PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS,
};

use super::{PRODUCT_SCHEMA_ROUTING_KEY, with_channel_identity_source};

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_product::ProductRuntimeSelected>() {
        return Ok(());
    }

    let product_schema = product_schema_ref()?;
    register_rule(
        extensions,
        "product",
        product_schema.clone(),
        &with_channel_identity_source(PRODUCT_QUERY_MATERIALIZED_FRESHNESS),
        "Product",
    )?;
    register_postgres_index_query_link_target_availability(extensions, "product", product_schema)
        .map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected Product Index linked-target availability registration failed: {error}"
        ))
    })?;
    register_rule(
        extensions,
        "product",
        product_variant_schema_ref()?,
        PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS,
        "ProductVariant",
    )?;

    if extensions.contains::<rustok_channel::ChannelRuntimeSelected>() {
        register_rule(
            extensions,
            "channel",
            sales_channel_schema_ref()?,
            SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS,
            "SalesChannel",
        )?;
    }
    Ok(())
}

fn register_rule(
    extensions: &mut ModuleRuntimeExtensions,
    owner_module: &str,
    schema: SchemaRef,
    template: &str,
    label: &str,
) -> rustok_core::Result<()> {
    let admission = PostgresQueryEntityAdmission::new(template).map_err(|error| {
        rustok_core::Error::Validation(format!(
            "selected {label} Index query admission construction failed: {error}"
        ))
    })?;
    register_postgres_index_query_admission(extensions, owner_module, schema, admission).map_err(
        |error| {
            rustok_core::Error::Validation(format!(
                "selected {label} Index query admission registration failed: {error}"
            ))
        },
    )
}

fn product_schema_ref() -> rustok_core::Result<SchemaRef> {
    schema_ref(
        "rustok-product",
        "product",
        SchemaVersion::new(PRODUCT_SCHEMA_ROUTING_KEY),
        "Product",
    )
}

fn product_variant_schema_ref() -> rustok_core::Result<SchemaRef> {
    schema_ref(
        "rustok-product",
        "product_variant",
        SchemaVersion::new(2),
        "ProductVariant",
    )
}

fn sales_channel_schema_ref() -> rustok_core::Result<SchemaRef> {
    schema_ref(
        "rustok-channel",
        "sales_channel",
        SchemaVersion::INITIAL,
        "SalesChannel",
    )
}

fn schema_ref(
    module: &str,
    entity: &str,
    version: SchemaVersion,
    label: &str,
) -> rustok_core::Result<SchemaRef> {
    Ok(SchemaRef {
        module: ModuleName::new(module).map_err(|error| {
            rustok_core::Error::Validation(format!("{label} Index module name is invalid: {error}"))
        })?,
        entity: EntityName::new(entity).map_err(|error| {
            rustok_core::Error::Validation(format!("{label} Index entity name is invalid: {error}"))
        })?,
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_materialized_freshness_predicate_fences_owner_and_locale_state() {
        for marker in [
            "product_index_graph_projection_snapshots",
            "product_sales_channel_index_relation_freshness_snapshots",
            "channel_index_identity_generations",
            "product_sales_channel_index_relation_convergence_requests",
            "{{entity}}.source_version = current_projection.projection_epoch",
            "current_projection.product_source_version = owner_product.index_revision",
            "request.product_source_version > current_freshness.product_source_version",
            "translation.locale = {{entity}}.locale_key",
            "LIMIT 1",
        ] {
            assert!(
                with_channel_identity_source(PRODUCT_QUERY_MATERIALIZED_FRESHNESS).contains(marker),
                "missing {marker}"
            );
        }
    }

    #[test]
    fn linked_target_predicates_require_live_owner_revision_identity() {
        for (template, owner_table) in [
            (
                PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS,
                "product_variants owner_variant",
            ),
            (
                SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS,
                "channels owner_channel",
            ),
        ] {
            assert!(template.contains(owner_table));
            assert!(template.contains("owner_"));
            assert!(template.contains(".tenant_id = {{entity}}.tenant_id"));
            assert!(template.contains(".id = {{entity}}.entity_id"));
            assert!(template.contains(".index_revision = {{entity}}.source_version"));
        }
    }

    #[test]
    fn owner_admission_predicates_do_not_read_materialized_index_storage() {
        for template in [
            &with_channel_identity_source(PRODUCT_QUERY_MATERIALIZED_FRESHNESS),
            PRODUCT_VARIANT_QUERY_MATERIALIZED_FRESHNESS,
            SALES_CHANNEL_QUERY_MATERIALIZED_FRESHNESS,
        ] {
            for forbidden in [
                format!("index_{}", "links"),
                format!("index_{}", "entities"),
                format!("${}", 1),
                format!("Index{}", "Mutation"),
            ] {
                assert!(!template.contains(&forbidden), "forbidden {forbidden}");
            }
        }
    }
}
