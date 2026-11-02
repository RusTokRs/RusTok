/*
 * Copyright (c) 2026 RusTokRs.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 */

//! Definition presentation/lifecycle, not storage-shape conversion or value reassignment.
//! The public state token is a state fingerprint, not a Product/document revision.

mod impact;
#[cfg(test)]
mod tests;

use super::*;
use crate::services::write_transaction::record_product_operation_result;
use sea_orm::TransactionTrait;
use sha2::{Digest, Sha256};

pub const MAX_ATTRIBUTE_LIFECYCLE_ROWS: usize = 1000;
pub const MAX_ATTRIBUTE_REFERENCE_EXAMPLES: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinitionCopy {
    pub label: String,
    pub help_text: Option<String>,
    pub facet_label: Option<String>,
    pub seo_label: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinitionPatch {
    pub translation: Option<AttributeDefinitionCopy>,
    pub is_filterable: Option<bool>,
    pub is_searchable: Option<bool>,
    pub is_sortable: Option<bool>,
    pub is_comparable: Option<bool>,
    pub show_on_storefront: Option<bool>,
    pub show_in_admin_grid: Option<bool>,
    pub search_weight: Option<i32>,
    pub position: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeOptionDefinitionPatch {
    pub label: Option<String>,
    pub position: Option<i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeDefinitionAction {
    Update,
    Archive,
    Restore,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAttributeDefinitionInput {
    pub attribute_id: Uuid,
    pub option_id: Option<Uuid>,
    pub locale: String,
    pub expected_state: String,
    pub action: AttributeDefinitionAction,
    pub attribute_patch: Option<AttributeDefinitionPatch>,
    pub option_patch: Option<AttributeOptionDefinitionPatch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeLifecycleRequest {
    pub attribute_id: Option<Uuid>,
    pub option_id: Option<Uuid>,
    pub include_archived: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromQueryResult)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinition {
    pub id: Uuid,
    pub code: String,
    pub value_type: String,
    pub scope: String,
    pub is_localized: bool,
    pub is_filterable: bool,
    pub is_searchable: bool,
    pub is_sortable: bool,
    pub is_comparable: bool,
    pub show_on_storefront: bool,
    pub show_in_admin_grid: bool,
    pub search_weight: i32,
    pub position: i32,
    pub is_archived: bool,
    #[sea_orm(skip)]
    pub translation: Option<AttributeDefinitionCopy>,
    #[sea_orm(skip)]
    pub state_token: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromQueryResult)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeOptionDefinition {
    pub id: Uuid,
    pub attribute_id: Uuid,
    pub code: String,
    pub position: i32,
    pub is_archived: bool,
    pub attribute_archived: bool,
    /// Exact locale only. `None` is not an empty or fallback translation.
    pub label: Option<String>,
    #[sea_orm(skip)]
    pub state_token: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeReferenceKind {
    ProductValue,
    VariantValue,
    SchemaBinding,
    CategoryBinding,
    CloneSnapshot,
    VariantAxis,
    AllowedOption,
    AttributeGroup,
    ChannelSetting,
    OptionRule,
    VirtualCategory,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeReference {
    pub kind: AttributeReferenceKind,
    pub resource_id: Uuid,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeReferenceImpact {
    pub references: Vec<AttributeReference>,
    pub has_more: bool,
    /// False is a refusal to prove unused, never an empty successful inspection.
    pub inspection_complete: bool,
}

impl AttributeReferenceImpact {
    pub fn can_archive(&self) -> bool {
        self.inspection_complete && self.references.is_empty() && !self.has_more
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeLifecycleWorkspace {
    pub attributes: Vec<AttributeDefinition>,
    /// Only selected-attribute requests load this collection, including archived children.
    pub options: Option<Vec<AttributeOptionDefinition>>,
    pub impact: Option<AttributeReferenceImpact>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeDefinitionRefusal {
    Stale,
    InUse,
    InspectionLimit,
    NotFound,
    ParentArchived,
}

/// Both positive and negative decisions are durable owner receipt results. Technical/commit
/// failures remain errors and do not masquerade as an authoritative negative decision.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttributeDefinitionResult {
    pub attribute_id: Uuid,
    pub option_id: Option<Uuid>,
    pub applied: bool,
    pub state_token: Option<String>,
    pub refusal: Option<AttributeDefinitionRefusal>,
    pub impact: Option<AttributeReferenceImpact>,
}

impl UpdateAttributeDefinitionInput {
    fn validate(&self) -> CommerceResult<()> {
        validate_uuid("attribute_id", self.attribute_id)?;
        if let Some(id) = self.option_id {
            validate_uuid("option_id", id)?;
        }
        let locale = rustok_api::TenantLocale::new(self.locale.as_str())
            .map_err(|_| CommerceError::Validation("Invalid definition locale".into()))?;
        if locale.as_str() != self.locale || !valid_state_token(&self.expected_state) {
            return Err(CommerceError::Validation(
                "Invalid definition predecessor".into(),
            ));
        }
        match (
            self.action,
            self.option_id,
            &self.attribute_patch,
            &self.option_patch,
        ) {
            (AttributeDefinitionAction::Update, None, Some(patch), None) => {
                if patch == &AttributeDefinitionPatch::default() {
                    return Err(CommerceError::Validation(
                        "Definition patch is empty".into(),
                    ));
                }
                if let Some(copy) = &patch.translation {
                    validate_copy(copy)?;
                }
                validate_position(patch.position)?;
                if patch.search_weight.is_some_and(|weight| weight < 0) {
                    return Err(CommerceError::Validation(
                        "Search weight must be nonnegative".into(),
                    ));
                }
            }
            (AttributeDefinitionAction::Update, Some(_), None, Some(patch)) => {
                if patch == &AttributeOptionDefinitionPatch::default() {
                    return Err(CommerceError::Validation("Option patch is empty".into()));
                }
                if let Some(label) = &patch.label {
                    validate_label(label)?;
                }
                validate_position(patch.position)?;
            }
            (
                AttributeDefinitionAction::Archive | AttributeDefinitionAction::Restore,
                _,
                None,
                None,
            ) => {}
            _ => {
                return Err(CommerceError::Validation(
                    "Definition action and patch do not match".into(),
                ));
            }
        }
        Ok(())
    }
}

fn validate_label(label: &str) -> CommerceResult<()> {
    if label.trim().is_empty() || label.chars().count() > 255 || label.contains('\0') {
        return Err(CommerceError::Validation(
            "Definition label must contain 1..255 characters".into(),
        ));
    }
    Ok(())
}

fn validate_copy(copy: &AttributeDefinitionCopy) -> CommerceResult<()> {
    validate_label(&copy.label)?;
    for label in [&copy.facet_label, &copy.seo_label].into_iter().flatten() {
        validate_label(label)?;
    }
    if copy
        .help_text
        .as_ref()
        .is_some_and(|text| text.len() > 4096 || text.contains('\0'))
    {
        return Err(CommerceError::Validation(
            "Definition help text exceeds its bound".into(),
        ));
    }
    Ok(())
}

fn validate_position(position: Option<i32>) -> CommerceResult<()> {
    if position.is_some_and(|position| position < 0) {
        return Err(CommerceError::Validation(
            "Definition position must be nonnegative".into(),
        ));
    }
    Ok(())
}

fn valid_state_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn fingerprint<T: Serialize>(tenant_id: Uuid, locale: &str, state: &T) -> CommerceResult<String> {
    let input = serde_json::to_vec(&("product:attribute-definition", tenant_id, locale, state))
        .map_err(|_| {
            CommerceError::Core(rustok_core::error::Error::Internal(
                "Definition state cannot be encoded".into(),
            ))
        })?;
    Ok(hex::encode(Sha256::digest(input)))
}

/// Retained copy may predate current write validation. Read it exactly, including negative
/// positions/empty labels, but never allocate an unbounded historic text payload in the client.
fn definition_text_bytes(backend: DatabaseBackend, expression: &str) -> CommerceResult<String> {
    match backend {
        DatabaseBackend::Postgres => Ok(format!("COALESCE(octet_length({expression}), 0)")),
        DatabaseBackend::Sqlite => Ok(format!("COALESCE(length(CAST({expression} AS BLOB)), 0)")),
        _ => Err(CommerceError::Database(sea_orm::DbErr::Custom(
            "Definition read capability is unavailable".into(),
        ))),
    }
}
async fn check_copy_budget<C: ConnectionTrait>(
    db: &C,
    sql: String,
    values: Vec<sea_orm::Value>,
) -> CommerceResult<()> {
    #[derive(FromQueryResult)]
    struct CopyBudget {
        maximum: Option<i64>,
        total: Option<i64>,
    }
    let row = CopyBudget::find_by_statement(Statement::from_sql_and_values(db.get_database_backend(),
        format!("SELECT CAST(MAX(copy_bytes) AS BIGINT) AS maximum, CAST(SUM(copy_bytes) AS BIGINT) AS total FROM ({sql}) AS retained_copy"), values,
    )).one(db).await?.ok_or_else(|| CommerceError::Database(sea_orm::DbErr::Custom("Definition copy inspection is unavailable".into())))?;
    if row
        .maximum
        .is_some_and(|bytes| !(0..=65536).contains(&bytes))
        || row
            .total
            .is_some_and(|bytes| !(0..=8_388_608).contains(&bytes))
    {
        return Err(CommerceError::Database(sea_orm::DbErr::Custom(
            "Definition copy inspection exceeds its bounded capability".into(),
        )));
    }
    Ok(())
}

async fn load_definitions<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    locale: &str,
    attribute_id: Option<Uuid>,
    include_archived: bool,
) -> CommerceResult<Vec<AttributeDefinition>> {
    let filter = if attribute_id.is_some() {
        " AND id = $2"
    } else {
        ""
    };
    let archive_filter = if include_archived {
        ""
    } else {
        " AND archived_at IS NULL"
    };
    let mut values = vec![tenant_id.into()];
    if let Some(id) = attribute_id {
        values.push(id.into());
    }
    let bytes = ["code", "value_type", "scope"]
        .into_iter()
        .map(|column| definition_text_bytes(db.get_database_backend(), column))
        .collect::<CommerceResult<Vec<_>>>()?
        .join(" + ");
    check_copy_budget(db, format!("SELECT {bytes} AS copy_bytes FROM product_attributes WHERE tenant_id = $1{filter}{archive_filter} ORDER BY position, code, id LIMIT {}", MAX_ATTRIBUTE_LIFECYCLE_ROWS + 1), values.clone()).await?;
    // Translation uses the same fence then a parent FOR UPDATE lock. Holding these row
    // shares makes the size probe and exact-copy fetch one stable projection, not a TOCTOU.
    let row_lock = if db.get_database_backend() == DatabaseBackend::Postgres {
        " FOR SHARE"
    } else {
        ""
    };
    let mut rows = AttributeDefinition::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        format!("SELECT id, code, value_type, scope, is_localized, is_filterable, is_searchable, is_sortable, is_comparable, show_on_storefront, show_in_admin_grid, search_weight, position, (archived_at IS NOT NULL) AS is_archived FROM product_attributes WHERE tenant_id = $1{filter}{archive_filter} ORDER BY position, code, id LIMIT {}{row_lock}", MAX_ATTRIBUTE_LIFECYCLE_ROWS + 1),
        values,
    )).all(db).await?;
    ensure_schema_list_within_limit(rows.len(), "attribute definition")?;
    // One bounded translation query, not one request/query per directory row.
    if !rows.is_empty() {
        #[derive(FromQueryResult)]
        struct DirectoryCopyRow {
            attribute_id: Uuid,
            label: String,
            help_text: Option<String>,
            facet_label: Option<String>,
            seo_label: Option<String>,
        }
        let ids = rows.iter().map(|row| row.id).collect::<Vec<_>>();
        let (placeholders, mut values) = uuid_filter_values(tenant_id, &ids);
        values.push(locale.to_owned().into());
        let locale_placeholder = format!("${}", values.len());
        let bytes = ["t.label", "t.help_text", "t.facet_label", "t.seo_label"]
            .into_iter()
            .map(|column| definition_text_bytes(db.get_database_backend(), column))
            .collect::<CommerceResult<Vec<_>>>()?
            .join(" + ");
        check_copy_budget(db, format!("SELECT {bytes} AS copy_bytes FROM product_attribute_translations t INNER JOIN product_attributes a ON a.id = t.attribute_id WHERE a.tenant_id = $1 AND a.id IN ({placeholders}) AND t.locale = {locale_placeholder}"), values.clone()).await?;
        let mut copies = DirectoryCopyRow::find_by_statement(Statement::from_sql_and_values(
            db.get_database_backend(),
            format!("SELECT t.attribute_id, t.label, t.help_text, t.facet_label, t.seo_label FROM product_attribute_translations t INNER JOIN product_attributes a ON a.id = t.attribute_id WHERE a.tenant_id = $1 AND a.id IN ({placeholders}) AND t.locale = {locale_placeholder}"), values,
        )).all(db).await?.into_iter().map(|copy| (copy.attribute_id, AttributeDefinitionCopy { label: copy.label, help_text: copy.help_text, facet_label: copy.facet_label, seo_label: copy.seo_label })).collect::<HashMap<_, _>>();
        for row in &mut rows {
            row.translation = copies.remove(&row.id);
            row.state_token = fingerprint(tenant_id, locale, row)?;
        }
    }
    Ok(rows)
}

async fn load_options<C: ConnectionTrait>(
    db: &C,
    tenant_id: Uuid,
    attribute_id: Uuid,
    locale: &str,
    option_id: Option<Uuid>,
) -> CommerceResult<Vec<AttributeOptionDefinition>> {
    let mut values = vec![
        tenant_id.into(),
        attribute_id.into(),
        locale.to_owned().into(),
    ];
    let filter = if let Some(id) = option_id {
        values.push(id.into());
        " AND o.id = $4"
    } else {
        ""
    };
    let bytes = format!(
        "{} + {}",
        definition_text_bytes(db.get_database_backend(), "o.code")?,
        definition_text_bytes(db.get_database_backend(), "t.label")?
    );
    check_copy_budget(db, format!("SELECT {bytes} AS copy_bytes FROM product_attribute_options o INNER JOIN product_attributes a ON a.tenant_id = o.tenant_id AND a.id = o.attribute_id LEFT JOIN product_attribute_option_translations t ON t.option_id = o.id AND t.locale = $3 WHERE o.tenant_id = $1 AND o.attribute_id = $2{filter} ORDER BY o.position, o.code, o.id LIMIT {}", MAX_ATTRIBUTE_LIFECYCLE_ROWS + 1), values.clone()).await?;
    let mut rows = AttributeOptionDefinition::find_by_statement(Statement::from_sql_and_values(
        db.get_database_backend(),
        format!("SELECT o.id, o.attribute_id, o.code, o.position, (o.archived_at IS NOT NULL) AS is_archived, (a.archived_at IS NOT NULL) AS attribute_archived, t.label FROM product_attribute_options o INNER JOIN product_attributes a ON a.tenant_id = o.tenant_id AND a.id = o.attribute_id LEFT JOIN product_attribute_option_translations t ON t.option_id = o.id AND t.locale = $3 WHERE o.tenant_id = $1 AND o.attribute_id = $2{filter} ORDER BY o.position, o.code, o.id LIMIT {}", MAX_ATTRIBUTE_LIFECYCLE_ROWS + 1), values,
    )).all(db).await?;
    ensure_schema_list_within_limit(rows.len(), "attribute option definition")?;
    for row in &mut rows {
        row.state_token = fingerprint(tenant_id, locale, row)?;
    }
    Ok(rows)
}

async fn lock_definition(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    attribute_id: Uuid,
) -> CommerceResult<()> {
    let suffix = if txn.get_database_backend() == DatabaseBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    txn.query_one_raw(Statement::from_sql_and_values(
        txn.get_database_backend(),
        format!("SELECT id FROM product_attributes WHERE tenant_id = $1 AND id = $2{suffix}"),
        vec![tenant_id.into(), attribute_id.into()],
    ))
    .await?;
    Ok(())
}

fn ensure_lifecycle_backend(backend: DatabaseBackend) -> CommerceResult<()> {
    if matches!(backend, DatabaseBackend::Postgres | DatabaseBackend::Sqlite) {
        Ok(())
    } else {
        Err(CommerceError::Database(sea_orm::DbErr::Custom(
            "Attribute lifecycle requires an available PostgreSQL or SQLite schema capability"
                .into(),
        )))
    }
}

impl ProductCatalogSchemaService {
    pub async fn read_attribute_lifecycle(
        &self,
        tenant_id: Uuid,
        locale: &str,
        request: AttributeLifecycleRequest,
    ) -> CommerceResult<AttributeLifecycleWorkspace> {
        ensure_lifecycle_backend(self.db.get_database_backend())?;
        validate_uuid("tenant_id", tenant_id)?;
        let canonical = rustok_api::TenantLocale::new(locale)
            .map_err(|_| CommerceError::Validation("Invalid definition locale".into()))?;
        if canonical.as_str() != locale {
            return Err(CommerceError::Validation(
                "Definition locale must be canonical".into(),
            ));
        }
        if let Some(id) = request.attribute_id {
            validate_uuid("attribute_id", id)?;
        }
        if let Some(id) = request.option_id {
            validate_uuid("option_id", id)?;
        }
        if request.option_id.is_some() && request.attribute_id.is_none() {
            return Err(CommerceError::Validation(
                "An option requires its parent attribute".into(),
            ));
        }
        let txn = self.db.begin().await?;
        policy::lock_schema_for_product_write(&txn, tenant_id).await?;
        let attributes = load_definitions(
            &txn,
            tenant_id,
            locale,
            request.attribute_id,
            request.include_archived,
        )
        .await?;
        let (options, impact) = if let Some(attribute) = attributes
            .first()
            .filter(|_| request.attribute_id.is_some())
        {
            let options = load_options(&txn, tenant_id, attribute.id, locale, None).await?;
            let option = request
                .option_id
                .and_then(|id| options.iter().find(|option| option.id == id));
            // A missing selected option remains missing, not the parent attribute. Return the
            // actual parent/directory so an explicit fresh intent can choose another target;
            // no impact or editable target is fabricated for the absent UUID.
            let impact = if request.option_id.is_some() && option.is_none() {
                None
            } else {
                Some(impact::inspect(&txn, tenant_id, attribute, option).await?)
            };
            (Some(options), impact)
        } else {
            (None, None)
        };
        txn.commit().await?;
        Ok(AttributeLifecycleWorkspace {
            attributes,
            options,
            impact,
        })
    }

    pub async fn update_attribute_definition(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: UpdateAttributeDefinitionInput,
    ) -> CommerceResult<AttributeDefinitionResult> {
        input.validate()?;
        ensure_lifecycle_backend(self.db.get_database_backend())?;
        validate_uuid("tenant_id", tenant_id)?;
        validate_uuid("actor_id", actor_id)?;
        let txn = ProductWriteTransaction::begin(&self.db, self.event_bus.clone()).await?;
        policy::lock_schema_for_policy_write(&txn, tenant_id).await?;
        lock_definition(&txn, tenant_id, input.attribute_id).await?;
        let attribute = load_definitions(
            &txn,
            tenant_id,
            &input.locale,
            Some(input.attribute_id),
            true,
        )
        .await?
        .pop();
        let mut result = AttributeDefinitionResult {
            attribute_id: input.attribute_id,
            option_id: input.option_id,
            applied: false,
            state_token: None,
            refusal: None,
            impact: None,
        };
        let Some(mut attribute) = attribute else {
            result.refusal = Some(AttributeDefinitionRefusal::NotFound);
            return finish(txn, result).await;
        };
        let option = if let Some(option_id) = input.option_id {
            load_options(
                &txn,
                tenant_id,
                input.attribute_id,
                &input.locale,
                Some(option_id),
            )
            .await?
            .pop()
        } else {
            None
        };
        if input.option_id.is_some() && option.is_none() {
            result.refusal = Some(AttributeDefinitionRefusal::NotFound);
            return finish(txn, result).await;
        }
        let current_token = option
            .as_ref()
            .map(|option| &option.state_token)
            .unwrap_or(&attribute.state_token);
        result.state_token = Some(current_token.clone());
        if input.expected_state != *current_token {
            result.refusal = Some(AttributeDefinitionRefusal::Stale);
            return finish(txn, result).await;
        }
        let archived = option
            .as_ref()
            .map(|option| option.is_archived)
            .unwrap_or(attribute.is_archived);
        if input.action == AttributeDefinitionAction::Restore
            && option.is_some()
            && attribute.is_archived
        {
            result.refusal = Some(AttributeDefinitionRefusal::ParentArchived);
            return finish(txn, result).await;
        }
        if input.action == AttributeDefinitionAction::Archive && !archived {
            let impact = impact::inspect(&txn, tenant_id, &attribute, option.as_ref()).await?;
            if !impact.can_archive() {
                result.refusal = Some(if impact.inspection_complete {
                    AttributeDefinitionRefusal::InUse
                } else {
                    AttributeDefinitionRefusal::InspectionLimit
                });
                result.impact = Some(impact);
                return finish(txn, result).await;
            }
        }
        let changed = match option {
            Some(mut option) => {
                let before = option.clone();
                match input.action {
                    AttributeDefinitionAction::Archive => option.is_archived = true,
                    AttributeDefinitionAction::Restore => option.is_archived = false,
                    AttributeDefinitionAction::Update => {
                        if let Some(patch) = &input.option_patch {
                            if let Some(label) = &patch.label {
                                option.label = Some(label.clone());
                            }
                            if let Some(position) = patch.position {
                                option.position = position;
                            }
                        }
                    }
                }
                if before == option {
                    false
                } else {
                    write_option(
                        &txn,
                        tenant_id,
                        &input.locale,
                        &option,
                        before.label != option.label,
                    )
                    .await?;
                    result.state_token = load_options(
                        &txn,
                        tenant_id,
                        input.attribute_id,
                        &input.locale,
                        Some(option.id),
                    )
                    .await?
                    .pop()
                    .map(|row| row.state_token);
                    txn.publish(
                        tenant_id,
                        Some(actor_id),
                        DomainEvent::ProductAttributeOptionUpdated {
                            option_id: option.id,
                            attribute_id: input.attribute_id,
                        },
                    )
                    .await?;
                    true
                }
            }
            None => {
                let before = attribute.clone();
                match input.action {
                    AttributeDefinitionAction::Archive => attribute.is_archived = true,
                    AttributeDefinitionAction::Restore => attribute.is_archived = false,
                    AttributeDefinitionAction::Update => {
                        if let Some(patch) = &input.attribute_patch {
                            apply_attribute_patch(&mut attribute, patch);
                        }
                    }
                }
                if before == attribute {
                    false
                } else {
                    write_attribute(
                        &txn,
                        tenant_id,
                        &input.locale,
                        &attribute,
                        before.translation != attribute.translation,
                    )
                    .await?;
                    result.state_token = load_definitions(
                        &txn,
                        tenant_id,
                        &input.locale,
                        Some(input.attribute_id),
                        true,
                    )
                    .await?
                    .pop()
                    .map(|row| row.state_token);
                    txn.publish(
                        tenant_id,
                        Some(actor_id),
                        DomainEvent::ProductAttributeUpdated {
                            attribute_id: input.attribute_id,
                        },
                    )
                    .await?;
                    true
                }
            }
        };
        if changed && result.state_token.is_none() {
            return Err(CommerceError::Core(rustok_core::error::Error::Internal(
                "Definition result disappeared before commit".into(),
            )));
        }
        result.applied = true;
        finish(txn, result).await
    }
}

fn apply_attribute_patch(attribute: &mut AttributeDefinition, patch: &AttributeDefinitionPatch) {
    if let Some(copy) = &patch.translation {
        attribute.translation = Some(copy.clone());
    }
    if let Some(value) = patch.is_filterable {
        attribute.is_filterable = value;
    }
    if let Some(value) = patch.is_searchable {
        attribute.is_searchable = value;
    }
    if let Some(value) = patch.is_sortable {
        attribute.is_sortable = value;
    }
    if let Some(value) = patch.is_comparable {
        attribute.is_comparable = value;
    }
    if let Some(value) = patch.show_on_storefront {
        attribute.show_on_storefront = value;
    }
    if let Some(value) = patch.show_in_admin_grid {
        attribute.show_in_admin_grid = value;
    }
    if let Some(value) = patch.search_weight {
        attribute.search_weight = value;
    }
    if let Some(value) = patch.position {
        attribute.position = value;
    }
}

async fn write_attribute(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    row: &AttributeDefinition,
    write_copy: bool,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), "UPDATE product_attributes SET is_filterable = $3, is_searchable = $4, is_sortable = $5, is_comparable = $6, show_on_storefront = $7, show_in_admin_grid = $8, search_weight = $9, position = $10, archived_at = CASE WHEN $11 THEN COALESCE(archived_at, CURRENT_TIMESTAMP) ELSE NULL END, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND id = $2", vec![tenant_id.into(), row.id.into(), row.is_filterable.into(), row.is_searchable.into(), row.is_sortable.into(), row.is_comparable.into(), row.show_on_storefront.into(), row.show_in_admin_grid.into(), row.search_weight.into(), row.position.into(), row.is_archived.into()])).await?;
    if write_copy && let Some(copy) = &row.translation {
        txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), "INSERT INTO product_attribute_translations (id, attribute_id, locale, label, help_text, facet_label, seo_label) VALUES ($1, $2, $3, $4, $5, $6, $7) ON CONFLICT (attribute_id, locale) DO UPDATE SET label = EXCLUDED.label, help_text = EXCLUDED.help_text, facet_label = EXCLUDED.facet_label, seo_label = EXCLUDED.seo_label", vec![generate_id().into(), row.id.into(), locale.to_owned().into(), copy.label.clone().into(), copy.help_text.clone().into(), copy.facet_label.clone().into(), copy.seo_label.clone().into()])).await?;
    }
    Ok(())
}

async fn write_option(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    locale: &str,
    row: &AttributeOptionDefinition,
    write_copy: bool,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), "UPDATE product_attribute_options SET position = $4, archived_at = CASE WHEN $5 THEN COALESCE(archived_at, CURRENT_TIMESTAMP) ELSE NULL END, updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND attribute_id = $2 AND id = $3", vec![tenant_id.into(), row.attribute_id.into(), row.id.into(), row.position.into(), row.is_archived.into()])).await?;
    if write_copy && let Some(label) = &row.label {
        txn.execute_raw(Statement::from_sql_and_values(txn.get_database_backend(), "INSERT INTO product_attribute_option_translations (id, option_id, locale, label) VALUES ($1, $2, $3, $4) ON CONFLICT (option_id, locale) DO UPDATE SET label = EXCLUDED.label", vec![generate_id().into(), row.id.into(), locale.to_owned().into(), label.clone().into()])).await?;
    }
    Ok(())
}

async fn finish(
    txn: ProductWriteTransaction,
    result: AttributeDefinitionResult,
) -> CommerceResult<AttributeDefinitionResult> {
    record_product_operation_result(&result);
    txn.commit().await?;
    Ok(result)
}
