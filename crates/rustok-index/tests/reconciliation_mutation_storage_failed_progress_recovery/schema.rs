use rustok_index::{
    EntityName, FieldCardinality, FieldName, IndexField, IndexSchema, IndexValueType,
    LocaleMode, ModuleName, SchemaRef, SchemaVersion,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};
use uuid::Uuid;

use super::connection::TestResult;

pub fn schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("mutation-storage-failed-progress-recovery-harness").unwrap(),
        entity: EntityName::new("item").unwrap(),
        version: SchemaVersion::INITIAL,
    }
}

pub fn schema() -> IndexSchema {
    IndexSchema {
        reference: schema_ref(),
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").unwrap(),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

pub async fn persist_schema(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<()> {
    let schema = schema();
    let fingerprint = schema.fingerprint()?.to_string();
    let schema_json = serde_json::to_value(&schema)?;
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES ($1, $2, $3, $4, $5, $6, 'active')",
        vec![
            tenant_id.into(),
            schema.reference.module.as_str().to_owned().into(),
            schema.reference.entity.as_str().to_owned().into(),
            i64::from(schema.reference.version.get()).into(),
            fingerprint.into(),
            SqlValue::Json(Some(Box::new(schema_json))),
        ],
    ))
    .await?;
    Ok(())
}
