//! Selected Inventory native-owner source and real generic Index consumers.
//! All stock conversion stays here; source authority/state remain with Auth/Inventory.

mod consumer;
mod schema;
mod source;
#[cfg(test)]
mod tests;

use rustok_core::{
    ModuleEventListenerContext, ModuleEventListenerRegistry, ModuleRuntimeExtensions,
};
use rustok_index::{
    PostgresIndexSourceFactory, register_index_mutation_event, register_index_schema_source,
    register_index_source, register_postgres_index_source_factory,
};
use sea_orm::DatabaseConnection;

pub const INVENTORY_STOCK_INDEX_SOURCE: &str = "inventory-stock-primary";
use rustok_inventory::source_history::INVENTORY_SOURCE_CHANGED_EVENT_TYPE;

pub(crate) fn register(extensions: &mut ModuleRuntimeExtensions) -> rustok_core::Result<()> {
    if !extensions.contains::<rustok_inventory::InventoryRuntimeSelected>() {
        return Ok(());
    }
    register_index_schema_source(
        extensions,
        "inventory",
        schema::stock_schema().map_err(invalid)?,
    )
    .map_err(invalid)?;
    register_postgres_index_source_factory(
        extensions,
        "inventory",
        INVENTORY_STOCK_INDEX_SOURCE,
        InventoryStockSourceFactory,
    )
    .map_err(invalid)?;
    register_index_mutation_event(
        extensions,
        "inventory",
        INVENTORY_SOURCE_CHANGED_EVENT_TYPE,
        INVENTORY_STOCK_INDEX_SOURCE,
        schema::schema_ref().map_err(invalid)?,
    )
    .map_err(invalid)
}

struct InventoryStockSourceFactory;

impl PostgresIndexSourceFactory for InventoryStockSourceFactory {
    fn register_source(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
        db: DatabaseConnection,
    ) -> Result<(), String> {
        let source = source::InventoryStockSource::new(
            db,
            extensions
                .get::<rustok_auth::WorkerServiceContextRuntime>()
                .cloned(),
        );
        register_index_source(
            extensions,
            "inventory",
            INVENTORY_STOCK_INDEX_SOURCE,
            [schema::schema_ref().map_err(|error| error.to_string())?],
            source,
        )
        .map_err(|error| error.to_string())
    }

    fn register_event_listeners(
        &self,
        registry: &mut ModuleEventListenerRegistry,
        context: &ModuleEventListenerContext<'_>,
    ) {
        registry.register(consumer::InventoryStockEventHandler::new(context));
    }
}

fn invalid(error: impl std::fmt::Display) -> rustok_core::Error {
    rustok_core::Error::Validation(format!(
        "Selected Inventory Index composition failed: {error}"
    ))
}
