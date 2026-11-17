use async_trait::async_trait;
use flex_ports::{
    FieldDefinitionView, FlexVersionedAttachedSchema, FlexVersionedAttachedSchemaReadPort,
    PRODUCT_ENTITY_TYPE,
};
use rustok_api::{PortContext, PortError};
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    IsolationLevel, QueryFilter, QuerySelect, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::attached_field_read::{attached_read_tenant, verified_schema};

// This is the legacy Flex definition store used by the platform's Product field editor.
mod definitions {
    rustok_core::define_field_definitions_entity!("product_field_definitions");
}

pub struct FlexProductSchemaReadService {
    db: DatabaseConnection,
}

impl FlexProductSchemaReadService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn verify_storage(&self) -> Result<(), PortError> {
        definitions::Entity::find()
            .limit(0)
            .all(&self.db)
            .await
            .map_err(|_| schema_unavailable())?;
        self.db
            .execute_unprepared(
                "SELECT tenant_id, revision FROM flex_product_attached_schema_revisions LIMIT 0",
            )
            .await
            .map_err(|_| schema_unavailable())?;
        Ok(())
    }
}

#[async_trait]
impl FlexVersionedAttachedSchemaReadPort for FlexProductSchemaReadService {
    async fn read_versioned_attached_schema(
        &self,
        context: PortContext,
        entity_type: &str,
    ) -> Result<FlexVersionedAttachedSchema, PortError> {
        read_product_schema(&self.db, &context, entity_type).await
    }
}

pub(crate) async fn read_product_schema(
    db: &DatabaseConnection,
    context: &PortContext,
    entity_type: &str,
) -> Result<FlexVersionedAttachedSchema, PortError> {
    let tenant_id = attached_read_tenant(context)?;
    if entity_type != PRODUCT_ENTITY_TYPE {
        return Err(PortError::unavailable(
            "flex.versioned_attached_schema_unsupported",
            "Versioned Flex schema is unavailable for this donor",
        ));
    }
    let txn = match db.get_database_backend() {
        DatabaseBackend::Postgres => {
            db.begin_with_config(
                Some(IsolationLevel::RepeatableRead),
                Some(AccessMode::ReadOnly),
            )
            .await
        }
        _ => db.begin().await,
    }
    .map_err(|_| schema_unavailable())?;
    // TRUNCATE is not MVCC-safe. Protect the definition table before taking the snapshot.
    if txn.get_database_backend() == DatabaseBackend::Postgres {
        txn.execute_unprepared("LOCK TABLE product_field_definitions IN ACCESS SHARE MODE")
            .await
            .map_err(|_| schema_unavailable())?;
    }
    let definitions = definitions::Entity::find()
        .filter(definitions::Column::TenantId.eq(tenant_id))
        .filter(definitions::Column::IsActive.eq(true))
        .limit((flex_ports::MAX_FLEX_ATTACHED_FIELDS + 1) as u64)
        .all(&txn)
        .await
        .map_err(|_| schema_unavailable())?;
    let revision = product_schema_revision(&txn, tenant_id).await?;
    txn.commit().await.map_err(|_| schema_unavailable())?;
    let schema = verified_schema(
        tenant_id,
        entity_type,
        definitions.into_iter().map(definition_view).collect(),
    )?;
    let snapshot = FlexVersionedAttachedSchema { revision, schema };
    snapshot.verify_scope(tenant_id, entity_type)?;
    Ok(snapshot)
}

async fn product_schema_revision(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
) -> Result<i64, PortError> {
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "SELECT revision FROM flex_product_attached_schema_revisions WHERE tenant_id = $1",
            [tenant_id.into()],
        ))
        .await
        .map_err(|_| schema_unavailable())?;
    row.map(|row| {
        row.try_get("", "revision")
            .map_err(|_| schema_unavailable())
    })
    .transpose()
    .map(|revision| revision.unwrap_or_default())
}

fn definition_view(row: definitions::Model) -> FieldDefinitionView {
    FieldDefinitionView {
        id: row.id,
        field_key: row.field_key,
        field_type: row.field_type,
        label: row.label,
        description: row.description,
        is_localized: row.is_localized,
        is_required: row.is_required,
        default_value: row.default_value,
        validation: row.validation,
        position: row.position,
        is_active: row.is_active,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }
}

fn schema_unavailable() -> PortError {
    PortError::unavailable(
        "flex.attached_schema_unavailable",
        "Flex attached schema is temporarily unavailable",
    )
}
