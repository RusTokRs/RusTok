use std::collections::{BTreeMap, HashMap};

use async_trait::async_trait;
use flex::{
    FlexSchemaTranslationError, FlexSchemaTranslationExactProgress, FlexSchemaTranslationLeaf,
    FlexSchemaTranslationProgressOwnerPort, FlexSchemaTranslationResult, UpdateFlexSchemaCommand,
    flex_schema_translation_leaf_required, parse_standalone_fields_config,
    schema_definition_translation_exact_values, validate_flex_schema_translation_locale_pair,
    validate_update_schema_command,
};
use sea_orm::{
    AccessMode, ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseConnection, EntityTrait,
    IsolationLevel, QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};
use uuid::Uuid;

use crate::models::{flex_schema_translations, flex_schemas};

const PROGRESS_PAGE_SIZE: u64 = 200;

#[derive(Clone)]
pub struct ServerFlexSchemaTranslationProgressOwner {
    db: DatabaseConnection,
}

impl ServerFlexSchemaTranslationProgressOwner {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl FlexSchemaTranslationProgressOwnerPort for ServerFlexSchemaTranslationProgressOwner {
    async fn read_exact_progress(
        &self,
        tenant_id: Uuid,
        source_locale: &str,
        target_locale: &str,
    ) -> FlexSchemaTranslationResult<FlexSchemaTranslationExactProgress> {
        validate_uuid(tenant_id, "tenant_id")?;
        validate_flex_schema_translation_locale_pair(source_locale, target_locale)?;

        // Progress spans multiple bounded UUID pages because fields_config is dynamic JSON.
        // PostgreSQL therefore uses one repeatable-read/read-only transaction so every page
        // observes the same owner snapshot. SQLite's transaction already pins a read
        // snapshot on first read and does not accept PostgreSQL isolation syntax.
        let txn = match self.db.get_database_backend() {
            DatabaseBackend::Postgres => {
                self.db
                    .begin_with_config(
                        Some(IsolationLevel::RepeatableRead),
                        Some(AccessMode::ReadOnly),
                    )
                    .await
            }
            _ => self.db.begin().await,
        }
        .map_err(database_error)?;

        let mut progress = FlexSchemaTranslationExactProgress::default();
        let mut after = None;
        loop {
            let source_schema_ids = sea_orm::sea_query::Query::select()
                .column(flex_schema_translations::Column::SchemaId)
                .from(flex_schema_translations::Entity)
                .and_where(
                    sea_orm::sea_query::Expr::col(flex_schema_translations::Column::Locale)
                        .eq(source_locale.to_string()),
                )
                .to_owned();
            let mut query = flex_schemas::Entity::find()
                .filter(flex_schemas::Column::TenantId.eq(tenant_id))
                .filter(flex_schemas::Column::Id.in_subquery(source_schema_ids))
                .order_by_asc(flex_schemas::Column::Id);
            if let Some(after) = after {
                query = query.filter(flex_schemas::Column::Id.gt(after));
            }

            let mut schemas = query
                .limit(PROGRESS_PAGE_SIZE + 1)
                .all(&txn)
                .await
                .map_err(database_error)?;
            let has_more = schemas.len() > PROGRESS_PAGE_SIZE as usize;
            if has_more {
                schemas.truncate(PROGRESS_PAGE_SIZE as usize);
            }
            if schemas.is_empty() {
                break;
            }

            let schema_ids = schemas.iter().map(|schema| schema.id).collect::<Vec<_>>();
            let translations = load_progress_translations(
                &txn,
                &schema_ids,
                source_locale,
                target_locale,
            )
            .await?;
            for schema in &schemas {
                let rows = translations.get(&schema.id).map(Vec::as_slice).unwrap_or(&[]);
                observe_schema(
                    &mut progress,
                    schema,
                    rows,
                    source_locale,
                    target_locale,
                )?;
            }

            if !has_more {
                break;
            }
            after = schemas.last().map(|schema| schema.id);
        }

        progress.validate()?;
        txn.commit().await.map_err(database_error)?;
        Ok(progress)
    }
}

async fn load_progress_translations<C>(
    db: &C,
    schema_ids: &[Uuid],
    source_locale: &str,
    target_locale: &str,
) -> FlexSchemaTranslationResult<HashMap<Uuid, Vec<flex_schema_translations::Model>>>
where
    C: ConnectionTrait,
{
    if schema_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let rows = flex_schema_translations::Entity::find()
        .filter(flex_schema_translations::Column::SchemaId.is_in(schema_ids.to_vec()))
        .filter(
            flex_schema_translations::Column::Locale
                .is_in(vec![source_locale.to_string(), target_locale.to_string()]),
        )
        .order_by_asc(flex_schema_translations::Column::SchemaId)
        .order_by_asc(flex_schema_translations::Column::Locale)
        .all(db)
        .await
        .map_err(database_error)?;
    let mut grouped = HashMap::<Uuid, Vec<flex_schema_translations::Model>>::new();
    for row in rows {
        grouped.entry(row.schema_id).or_default().push(row);
    }
    Ok(grouped)
}

fn observe_schema(
    progress: &mut FlexSchemaTranslationExactProgress,
    schema: &flex_schemas::Model,
    translations: &[flex_schema_translations::Model],
    source_locale: &str,
    target_locale: &str,
) -> FlexSchemaTranslationResult<()> {
    let source_row = translations
        .iter()
        .find(|translation| translation.locale == source_locale)
        .ok_or_else(|| {
            FlexSchemaTranslationError::OwnerInvariant(format!(
                "source-eligible Flex schema {} is missing exact locale `{source_locale}` inside the progress snapshot",
                schema.id
            ))
        })?;
    validate_translation_row(source_row)?;
    let target_row = translations
        .iter()
        .find(|translation| translation.locale == target_locale);
    if let Some(target_row) = target_row {
        validate_translation_row(target_row)?;
    }

    let definitions = parse_standalone_fields_config(schema.fields_config.clone()).map_err(|error| {
        FlexSchemaTranslationError::OwnerInvariant(format!(
            "persisted Flex schema fields_config violates the owner contract: {error}"
        ))
    })?;
    let mut source_values = schema_definition_translation_exact_values(&definitions, source_locale)?;
    insert_schema_row_values(&mut source_values, source_row);
    let mut target_values = schema_definition_translation_exact_values(&definitions, target_locale)?;
    if let Some(target_row) = target_row {
        insert_schema_row_values(&mut target_values, target_row);
    }

    checked_increment(&mut progress.resources, "resources")?;
    let mut complete = true;
    for leaf in source_values.keys() {
        let exact = target_values.contains_key(leaf);
        if flex_schema_translation_leaf_required(leaf) {
            checked_increment(&mut progress.required_units, "required_units")?;
            if exact {
                checked_increment(
                    &mut progress.exact_required_units,
                    "exact_required_units",
                )?;
            } else {
                complete = false;
            }
        } else {
            checked_increment(&mut progress.optional_units, "optional_units")?;
            if exact {
                checked_increment(
                    &mut progress.exact_optional_units,
                    "exact_optional_units",
                )?;
            }
        }
    }
    if complete {
        checked_increment(&mut progress.complete_resources, "complete_resources")?;
    }
    Ok(())
}

fn insert_schema_row_values(
    values: &mut BTreeMap<FlexSchemaTranslationLeaf, String>,
    row: &flex_schema_translations::Model,
) {
    values.insert(FlexSchemaTranslationLeaf::SchemaName, row.name.clone());
    if let Some(description) = &row.description {
        values.insert(
            FlexSchemaTranslationLeaf::SchemaDescription,
            description.clone(),
        );
    }
}

fn validate_translation_row(
    row: &flex_schema_translations::Model,
) -> FlexSchemaTranslationResult<()> {
    validate_update_schema_command(&UpdateFlexSchemaCommand {
        name: Some(row.name.clone()),
        description: row.description.clone(),
        ..Default::default()
    })
    .map_err(|error| {
        FlexSchemaTranslationError::OwnerInvariant(format!(
            "persisted Flex schema translation row violates the owner contract: {error}"
        ))
    })
}

fn checked_increment(value: &mut u64, label: &str) -> FlexSchemaTranslationResult<()> {
    *value = value.checked_add(1).ok_or_else(|| {
        FlexSchemaTranslationError::OwnerInvariant(format!(
            "Flex schema translation progress `{label}` overflowed u64"
        ))
    })?;
    Ok(())
}

fn validate_uuid(value: Uuid, label: &str) -> FlexSchemaTranslationResult<()> {
    if value.is_nil() {
        return Err(FlexSchemaTranslationError::Invalid(format!(
            "Flex schema translation {label} must not be the nil UUID"
        )));
    }
    Ok(())
}

fn database_error(error: sea_orm::DbErr) -> FlexSchemaTranslationError {
    FlexSchemaTranslationError::Database(error.to_string())
}
