use super::*;

use sea_orm::{DatabaseTransaction, DbBackend, QueryResult};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

pub const MAX_PRODUCT_OPTION_TRANSLATION_CHANGE_PAGE: u16 = 200;
const DELETED_REVISION_PREFIX: &str = "option-deleted-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductOptionTranslationChangeLifecycle {
    Active,
    Archived,
    Deleted,
}

impl ProductOptionTranslationChangeLifecycle {
    fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Deleted => "deleted",
        }
    }

    fn parse(value: &str) -> CommerceResult<Self> {
        match value {
            "active" => Ok(Self::Active),
            "archived" => Ok(Self::Archived),
            "deleted" => Ok(Self::Deleted),
            _ => Err(CommerceError::Validation(
                "Product Option translation change journal returned an invalid lifecycle"
                    .to_owned(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductOptionTranslationChangeRecord {
    pub change_seq: u64,
    pub product_id: Uuid,
    pub option_id: Uuid,
    pub resource_revision: String,
    pub lifecycle: ProductOptionTranslationChangeLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviousOptionTranslationChange {
    resource_revision: String,
    lifecycle: ProductOptionTranslationChangeLifecycle,
}

impl CatalogService {
    pub async fn product_option_translation_change_highwater(
        &self,
        tenant_id: Uuid,
    ) -> CommerceResult<Option<u64>> {
        validate_tenant(tenant_id)?;
        ensure_postgres(self.database())?;
        let row = self
            .database()
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT MAX(change_seq) AS highwater FROM product_option_translation_change_journal WHERE tenant_id = $1",
                vec![tenant_id.into()],
            ))
            .await?
            .ok_or_else(|| {
                CommerceError::Validation(
                    "Product Option translation change high-water query returned no row"
                        .to_owned(),
                )
            })?;
        optional_positive_sequence(row.try_get("", "highwater")?, "high-water")
    }

    pub async fn read_product_option_translation_changes(
        &self,
        tenant_id: Uuid,
        after_seq: u64,
        through_seq: u64,
        limit: u16,
    ) -> CommerceResult<Vec<ProductOptionTranslationChangeRecord>> {
        validate_tenant(tenant_id)?;
        if through_seq == 0 || after_seq > through_seq {
            return Err(CommerceError::Validation(
                "Product Option translation change cursor bounds are invalid".to_owned(),
            ));
        }
        if limit == 0 || limit > MAX_PRODUCT_OPTION_TRANSLATION_CHANGE_PAGE {
            return Err(CommerceError::Validation(format!(
                "Product Option translation change page size must be between 1 and {MAX_PRODUCT_OPTION_TRANSLATION_CHANGE_PAGE}"
            )));
        }
        ensure_postgres(self.database())?;

        let rows = self
            .database()
            .query_all_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
SELECT change_seq, product_id, option_id, resource_revision, lifecycle
FROM product_option_translation_change_journal
WHERE tenant_id = $1
  AND change_seq > $2
  AND change_seq <= $3
ORDER BY change_seq ASC
LIMIT $4
"#,
                vec![
                    tenant_id.into(),
                    i64::try_from(after_seq)
                        .map_err(|_| invalid_sequence("after"))?
                        .into(),
                    i64::try_from(through_seq)
                        .map_err(|_| invalid_sequence("through"))?
                        .into(),
                    i64::from(limit).into(),
                ],
            ))
            .await?;

        rows.into_iter().map(change_record_from_row).collect()
    }
}

/// Captures durable Product-owned Option Translation change evidence.
///
/// Live Product lifecycle events are reconstructed from the exact post-command
/// aggregate and semantic-deduped by the same canonical revision returned by
/// exact read/apply. Product deletion is different: Option rows have already
/// been removed when the root event is published, so `deleted_option_ids` must
/// contain the Product-owned identities captured immediately before deletion.
/// The supplied identities are committed under the exact same outbox envelope
/// UUID and database transaction; no Translation-owned shadow persistence is
/// introduced.
pub(crate) async fn record_product_option_translation_changes_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    deleted_option_ids: Option<&[Uuid]>,
) -> CommerceResult<()> {
    if txn.get_database_backend() != DbBackend::Postgres {
        return Ok(());
    }
    if tenant_id.is_nil() || product_id.is_nil() || root_event_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product Option translation change journal identity must not be nil".to_owned(),
        ));
    }

    let previous = load_previous_changes(txn, tenant_id, product_id).await?;
    let product = entities::product::Entity::find_by_id(product_id)
        .filter(entities::product::Column::TenantId.eq(tenant_id))
        .one(txn)
        .await?;

    match (product, deleted_option_ids) {
        (Some(product), None) => {
            record_live_product_options(txn, &product, root_event_id, &previous).await
        }
        (None, Some(option_ids)) => {
            record_deleted_product_options(
                txn,
                tenant_id,
                product_id,
                root_event_id,
                option_ids,
                &previous,
            )
            .await
        }
        (Some(_), Some(_)) => Err(CommerceError::Validation(
            "Product Option translation delete targets were supplied while Product is still live"
                .to_owned(),
        )),
        (None, None) => Err(CommerceError::Validation(
            "Product Option translation delete evidence requires pre-delete Option identities"
                .to_owned(),
        )),
    }
}

async fn record_live_product_options(
    txn: &DatabaseTransaction,
    product: &entities::product::Model,
    root_event_id: Uuid,
    previous: &HashMap<Uuid, PreviousOptionTranslationChange>,
) -> CommerceResult<()> {
    let options = entities::product_option::Entity::find()
        .filter(entities::product_option::Column::ProductId.eq(product.id))
        .order_by_asc(entities::product_option::Column::Id)
        .all(txn)
        .await?;
    if options.is_empty() {
        return Ok(());
    }

    let option_ids = options.iter().map(|option| option.id).collect::<Vec<_>>();
    let values = entities::product_option_value::Entity::find()
        .filter(entities::product_option_value::Column::OptionId.is_in(option_ids.clone()))
        .order_by_asc(entities::product_option_value::Column::OptionId)
        .order_by_asc(entities::product_option_value::Column::Position)
        .order_by_asc(entities::product_option_value::Column::Id)
        .all(txn)
        .await?;
    let option_translations = entities::product_option_translation::Entity::find()
        .filter(entities::product_option_translation::Column::OptionId.is_in(option_ids))
        .order_by_asc(entities::product_option_translation::Column::OptionId)
        .order_by_asc(entities::product_option_translation::Column::Locale)
        .all(txn)
        .await?;

    let value_ids = values.iter().map(|value| value.id).collect::<Vec<_>>();
    let value_translations = if value_ids.is_empty() {
        Vec::new()
    } else {
        entities::product_option_value_translation::Entity::find()
            .filter(
                entities::product_option_value_translation::Column::ValueId.is_in(value_ids),
            )
            .order_by_asc(entities::product_option_value_translation::Column::ValueId)
            .order_by_asc(entities::product_option_value_translation::Column::Locale)
            .all(txn)
            .await?
    };

    let mut values_by_option = values.into_iter().fold(
        HashMap::<Uuid, Vec<entities::product_option_value::Model>>::new(),
        |mut grouped, value| {
            grouped.entry(value.option_id).or_default().push(value);
            grouped
        },
    );
    let mut option_translations_by_option = option_translations.into_iter().fold(
        HashMap::<Uuid, Vec<entities::product_option_translation::Model>>::new(),
        |mut grouped, translation| {
            grouped
                .entry(translation.option_id)
                .or_default()
                .push(translation);
            grouped
        },
    );
    let value_to_option = values_by_option
        .iter()
        .flat_map(|(option_id, values)| values.iter().map(|value| (value.id, *option_id)))
        .collect::<HashMap<_, _>>();
    let mut value_translations_by_option = value_translations.into_iter().try_fold(
        HashMap::<Uuid, Vec<entities::product_option_value_translation::Model>>::new(),
        |mut grouped, translation| {
            let option_id = value_to_option.get(&translation.value_id).copied().ok_or_else(|| {
                CommerceError::Validation(
                    "Product Option translation journal value escaped its owner aggregate"
                        .to_owned(),
                )
            })?;
            grouped.entry(option_id).or_default().push(translation);
            Ok::<_, CommerceError>(grouped)
        },
    )?;
    let lifecycle = product_lifecycle(&product.status);

    for option in options {
        let values = values_by_option.remove(&option.id).unwrap_or_default();
        let option_translations = option_translations_by_option
            .remove(&option.id)
            .unwrap_or_default();
        let value_translations = value_translations_by_option
            .remove(&option.id)
            .unwrap_or_default();
        let resource_revision = option_translation_resource_revision(
            product,
            &option,
            &values,
            &option_translations,
            &value_translations,
        );
        if previous.get(&option.id).is_some_and(|previous| {
            previous.resource_revision == resource_revision && previous.lifecycle == lifecycle
        }) {
            continue;
        }
        insert_change(
            txn,
            root_event_id,
            product.tenant_id,
            product.id,
            option.id,
            &resource_revision,
            lifecycle,
        )
        .await?;
    }

    Ok(())
}

async fn record_deleted_product_options(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
    root_event_id: Uuid,
    option_ids: &[Uuid],
    previous: &HashMap<Uuid, PreviousOptionTranslationChange>,
) -> CommerceResult<()> {
    let mut seen = HashSet::with_capacity(option_ids.len());
    let mut ordered = Vec::with_capacity(option_ids.len());
    for option_id in option_ids {
        if option_id.is_nil() || !seen.insert(*option_id) {
            return Err(CommerceError::Validation(
                "Product Option translation delete targets must be unique non-nil UUIDs"
                    .to_owned(),
            ));
        }
        ordered.push(*option_id);
    }
    ordered.sort_unstable();

    for option_id in ordered {
        if previous
            .get(&option_id)
            .is_some_and(|previous| previous.lifecycle == ProductOptionTranslationChangeLifecycle::Deleted)
        {
            continue;
        }
        let resource_revision = deleted_revision(root_event_id, option_id);
        insert_change(
            txn,
            root_event_id,
            tenant_id,
            product_id,
            option_id,
            &resource_revision,
            ProductOptionTranslationChangeLifecycle::Deleted,
        )
        .await?;
    }
    Ok(())
}

async fn load_previous_changes(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> CommerceResult<HashMap<Uuid, PreviousOptionTranslationChange>> {
    let rows = txn
        .query_all_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
SELECT DISTINCT ON (option_id)
    option_id,
    resource_revision,
    lifecycle
FROM product_option_translation_change_journal
WHERE tenant_id = $1
  AND product_id = $2
ORDER BY option_id ASC, change_seq DESC
"#,
            vec![tenant_id.into(), product_id.into()],
        ))
        .await?;

    let mut previous = HashMap::with_capacity(rows.len());
    for row in rows {
        let option_id: Uuid = row.try_get("", "option_id")?;
        let resource_revision: String = row.try_get("", "resource_revision")?;
        let lifecycle: String = row.try_get("", "lifecycle")?;
        if option_id.is_nil() || resource_revision.trim().is_empty() {
            return Err(CommerceError::Validation(
                "Product Option translation change journal returned an invalid previous row"
                    .to_owned(),
            ));
        }
        previous.insert(
            option_id,
            PreviousOptionTranslationChange {
                resource_revision,
                lifecycle: ProductOptionTranslationChangeLifecycle::parse(&lifecycle)?,
            },
        );
    }
    Ok(previous)
}

async fn insert_change(
    txn: &DatabaseTransaction,
    root_event_id: Uuid,
    tenant_id: Uuid,
    product_id: Uuid,
    option_id: Uuid,
    resource_revision: &str,
    lifecycle: ProductOptionTranslationChangeLifecycle,
) -> CommerceResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO product_option_translation_change_journal (
    root_event_id,
    tenant_id,
    product_id,
    option_id,
    resource_revision,
    lifecycle,
    created_at
) VALUES ($1, $2, $3, $4, $5, $6, CURRENT_TIMESTAMP)
ON CONFLICT (root_event_id, option_id) DO NOTHING
"#,
        vec![
            root_event_id.into(),
            tenant_id.into(),
            product_id.into(),
            option_id.into(),
            resource_revision.to_owned().into(),
            lifecycle.as_str().into(),
        ],
    ))
    .await?;
    Ok(())
}

fn change_record_from_row(row: QueryResult) -> CommerceResult<ProductOptionTranslationChangeRecord> {
    let change_seq = positive_sequence(row.try_get("", "change_seq")?, "change")?;
    let product_id: Uuid = row.try_get("", "product_id")?;
    let option_id: Uuid = row.try_get("", "option_id")?;
    let resource_revision: String = row.try_get("", "resource_revision")?;
    let lifecycle: String = row.try_get("", "lifecycle")?;
    if product_id.is_nil() || option_id.is_nil() || resource_revision.trim().is_empty() {
        return Err(CommerceError::Validation(
            "Product Option translation change journal returned an invalid row".to_owned(),
        ));
    }
    Ok(ProductOptionTranslationChangeRecord {
        change_seq,
        product_id,
        option_id,
        resource_revision,
        lifecycle: ProductOptionTranslationChangeLifecycle::parse(&lifecycle)?,
    })
}

fn product_lifecycle(
    status: &entities::product::ProductStatus,
) -> ProductOptionTranslationChangeLifecycle {
    match status {
        entities::product::ProductStatus::Draft | entities::product::ProductStatus::Active => {
            ProductOptionTranslationChangeLifecycle::Active
        }
        entities::product::ProductStatus::Archived => {
            ProductOptionTranslationChangeLifecycle::Archived
        }
    }
}

/// Keep this algorithm byte-for-byte semantically aligned with
/// `option_translation::product_option_translation_resource_revision`.
fn option_translation_resource_revision(
    product: &entities::product::Model,
    option: &entities::product_option::Model,
    values: &[entities::product_option_value::Model],
    option_translations: &[entities::product_option_translation::Model],
    value_translations: &[entities::product_option_value_translation::Model],
) -> String {
    let mut hasher = Sha256::new();
    digest_text(
        &mut hasher,
        "rustok-product/option-translation-resource/v1",
    );
    digest_text(&mut hasher, &product.id.to_string());
    digest_text(&mut hasher, &product.tenant_id.to_string());
    digest_text(&mut hasher, &product.status.to_string());
    digest_text(&mut hasher, &option.id.to_string());
    digest_text(&mut hasher, &option.product_id.to_string());
    digest_i32(&mut hasher, option.position);

    let mut ordered_values = values.iter().collect::<Vec<_>>();
    ordered_values.sort_by_key(|value| (value.position, value.id));
    for value in ordered_values {
        digest_text(&mut hasher, &value.id.to_string());
        digest_text(&mut hasher, &value.option_id.to_string());
        digest_i32(&mut hasher, value.position);
    }

    let mut titles = option_translations.iter().collect::<Vec<_>>();
    titles.sort_by(|left, right| left.locale.cmp(&right.locale));
    for translation in titles {
        digest_text(&mut hasher, &translation.option_id.to_string());
        digest_text(&mut hasher, &translation.locale);
        digest_text(&mut hasher, &translation.title);
    }

    let mut translated_values = value_translations.iter().collect::<Vec<_>>();
    translated_values.sort_by(|left, right| {
        left.value_id
            .cmp(&right.value_id)
            .then_with(|| left.locale.cmp(&right.locale))
    });
    for translation in translated_values {
        digest_text(&mut hasher, &translation.value_id.to_string());
        digest_text(&mut hasher, &translation.locale);
        digest_text(&mut hasher, &translation.value);
    }

    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn digest_text(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn digest_i32(hasher: &mut Sha256, value: i32) {
    hasher.update(value.to_be_bytes());
}

fn deleted_revision(root_event_id: Uuid, option_id: Uuid) -> String {
    format!("{DELETED_REVISION_PREFIX}:{root_event_id}:{option_id}")
}

fn validate_tenant(tenant_id: Uuid) -> CommerceResult<()> {
    if tenant_id.is_nil() {
        return Err(CommerceError::Validation(
            "Product Option translation change tenant must not be nil".to_owned(),
        ));
    }
    Ok(())
}

fn ensure_postgres(db: &DatabaseConnection) -> CommerceResult<()> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(CommerceError::Validation(
            "Product Option translation change source requires PostgreSQL".to_owned(),
        ));
    }
    Ok(())
}

fn optional_positive_sequence(value: Option<i64>, field: &str) -> CommerceResult<Option<u64>> {
    value.map(|value| positive_sequence(value, field)).transpose()
}

fn positive_sequence(value: i64, field: &str) -> CommerceResult<u64> {
    let value = u64::try_from(value).map_err(|_| invalid_sequence(field))?;
    if value == 0 {
        return Err(invalid_sequence(field));
    }
    Ok(value)
}

fn invalid_sequence(field: &str) -> CommerceError {
    CommerceError::Validation(format!(
        "Product Option translation change {field} sequence must be positive"
    ))
}
