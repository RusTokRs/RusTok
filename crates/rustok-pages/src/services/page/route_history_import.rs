use std::collections::BTreeSet;

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DatabaseTransaction, DbBackend, EntityTrait, QueryFilter, QuerySelect, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::instrument;
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::{
    SecurityContext,
    error::{ErrorKind, RichError},
};

use crate::entities::{
    page, page_route_alias, page_route_history_import, page_route_publication, page_translation,
};
use crate::error::{PagesError, PagesResult};
use crate::services::rbac::enforce_scope;

use super::helpers::{normalize_locale, normalize_slug};

pub const MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS: usize = 100;
pub const PAGE_ROUTE_HISTORY_IMPORT_CONFLICT: &str = "PAGE_ROUTE_HISTORY_IMPORT_CONFLICT";

const ROUTE_DISPOSITION_REDIRECT: &str = "redirect";
const ROUTE_DISPOSITION_GONE: &str = "gone";
const HISTORICAL_ROUTE_IMPORT_REASON: &str = "Historical page route import";
const MAX_IMPORT_SOURCE_LEN: usize = 64;
const MAX_IMPORT_SOURCE_RECORD_ID_LEN: usize = 191;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageRouteHistoryImportItem {
    pub source_record_id: String,
    pub page_id: Uuid,
    pub locale: String,
    pub slug: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ImportPageRouteHistoryInput {
    pub source: String,
    pub items: Vec<PageRouteHistoryImportItem>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageRouteHistoryImportResult {
    pub processed_item_count: u32,
    pub inserted_receipt_count: u32,
    pub replayed_receipt_count: u32,
    pub inserted_snapshot_count: u32,
    pub inserted_gone_alias_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreparedImportItem {
    source_record_id: String,
    page_id: Uuid,
    locale: String,
    slug: String,
    request_hash: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RouteEnsureOutcome {
    snapshot_inserted: bool,
    gone_alias_inserted: bool,
}

/// Explicit, bounded repair for public Pages route history that cannot be inferred safely.
///
/// Old non-builder publication and physically deleted page rows do not provide enough durable
/// source data for an automatic scan. This owner therefore requires an external provenance key for
/// every recovered route, records an immutable replay receipt, and composes the existing snapshot
/// and tombstone ledgers in one transaction. Exact replay is accepted; source-key payload drift,
/// route ownership drift, or redirect-only history for a missing page fails closed.
pub struct PageRouteHistoryImportService {
    db: DatabaseConnection,
}

impl PageRouteHistoryImportService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    #[instrument(skip(self, security, input))]
    pub async fn import_public_routes(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: ImportPageRouteHistoryInput,
    ) -> PagesResult<PageRouteHistoryImportResult> {
        enforce_scope(&security, Resource::Pages, Action::Manage)?;
        let (source, items) = prepare_input(tenant_id, input)?;
        let txn = self.db.begin().await?;

        let mut inserted_receipt_count = 0_u32;
        let mut replayed_receipt_count = 0_u32;
        let mut inserted_snapshot_count = 0_u32;
        let mut inserted_gone_alias_count = 0_u32;
        let mut terminal_pages = BTreeSet::new();

        for item in &items {
            let receipts = page_route_history_import::Entity::find()
                .filter(page_route_history_import::Column::TenantId.eq(tenant_id))
                .filter(page_route_history_import::Column::Source.eq(&source))
                .filter(
                    page_route_history_import::Column::SourceRecordId.eq(&item.source_record_id),
                )
                .all(&txn)
                .await?;

            let current_page = load_page_for_import(&txn, item.page_id).await?;
            if current_page
                .as_ref()
                .is_some_and(|page| page.tenant_id != tenant_id)
            {
                return Err(route_history_import_conflict(
                    "Historical route import page identifier belongs to another tenant",
                ));
            }
            let page_exists = current_page.is_some();

            let (page_was_missing, replayed) = match receipts.as_slice() {
                [] => (!page_exists, false),
                [receipt] => {
                    verify_receipt(receipt, tenant_id, &source, item)?;
                    (receipt.page_was_missing, true)
                }
                _ => {
                    return Err(route_history_import_conflict(
                        "Historical route import provenance is ambiguous",
                    ));
                }
            };

            let terminal = page_was_missing || !page_exists;
            let outcome = ensure_route_in_tx(&txn, tenant_id, item, terminal).await?;
            if outcome.snapshot_inserted {
                inserted_snapshot_count = checked_increment(
                    inserted_snapshot_count,
                    "Historical route import snapshot count overflow",
                )?;
            }
            if outcome.gone_alias_inserted {
                inserted_gone_alias_count = checked_increment(
                    inserted_gone_alias_count,
                    "Historical route import gone alias count overflow",
                )?;
            }
            if terminal {
                terminal_pages.insert(item.page_id);
            }

            if replayed {
                replayed_receipt_count = checked_increment(
                    replayed_receipt_count,
                    "Historical route import replay count overflow",
                )?;
            } else {
                page_route_history_import::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    source: Set(source.clone()),
                    source_record_id: Set(item.source_record_id.clone()),
                    request_hash: Set(item.request_hash.clone()),
                    page_id: Set(item.page_id),
                    locale: Set(item.locale.clone()),
                    slug: Set(item.slug.clone()),
                    page_was_missing: Set(page_was_missing),
                    imported_by: Set(security.user_id),
                    imported_at: Set(Utc::now().into()),
                }
                .insert(&txn)
                .await?;
                inserted_receipt_count = checked_increment(
                    inserted_receipt_count,
                    "Historical route import receipt count overflow",
                )?;
            }
        }

        for page_id in terminal_pages {
            if !page_has_terminal_gone_alias(&txn, tenant_id, page_id).await? {
                return Err(route_history_import_conflict(
                    "A missing page import must include or reference at least one terminal gone route",
                ));
            }
        }

        let processed_item_count = u32::try_from(items.len())
            .map_err(|_| PagesError::validation("Historical route import item count overflow"))?;
        txn.commit().await?;

        Ok(PageRouteHistoryImportResult {
            processed_item_count,
            inserted_receipt_count,
            replayed_receipt_count,
            inserted_snapshot_count,
            inserted_gone_alias_count,
        })
    }
}

fn prepare_input(
    tenant_id: Uuid,
    input: ImportPageRouteHistoryInput,
) -> PagesResult<(String, Vec<PreparedImportItem>)> {
    if tenant_id.is_nil() {
        return Err(PagesError::validation(
            "Historical route import tenant must not be nil",
        ));
    }
    if input.items.is_empty() || input.items.len() > MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS {
        return Err(PagesError::validation(format!(
            "Historical route import must contain between 1 and {MAX_PAGE_ROUTE_HISTORY_IMPORT_ITEMS} items",
        )));
    }

    let source = normalize_source(&input.source)?;
    let mut seen_record_ids = BTreeSet::new();
    let mut prepared = Vec::with_capacity(input.items.len());
    for item in input.items {
        if item.page_id.is_nil() {
            return Err(PagesError::validation(
                "Historical route import page must not be nil",
            ));
        }
        let source_record_id = normalize_source_record_id(&item.source_record_id)?;
        if !seen_record_ids.insert(source_record_id.clone()) {
            return Err(PagesError::validation(format!(
                "Historical route import source record is duplicated in the batch: {source_record_id}",
            )));
        }
        let locale = normalize_locale(&item.locale)?;
        let slug = normalize_slug(&item.slug)?;
        let request_hash =
            import_request_hash(&source, &source_record_id, item.page_id, &locale, &slug);
        prepared.push(PreparedImportItem {
            source_record_id,
            page_id: item.page_id,
            locale,
            slug,
            request_hash,
        });
    }
    Ok((source, prepared))
}

async fn load_page_for_import(
    txn: &DatabaseTransaction,
    page_id: Uuid,
) -> PagesResult<Option<page::Model>> {
    let query = || page::Entity::find_by_id(page_id);
    Ok(match txn.get_database_backend() {
        DbBackend::Sqlite => query().one(txn).await?,
        DbBackend::Postgres | DbBackend::MySql => query().lock_exclusive().one(txn).await?,
        _ => unreachable!("unsupported SeaORM database backend"),
})
}

async fn ensure_route_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    item: &PreparedImportItem,
    terminal: bool,
) -> PagesResult<RouteEnsureOutcome> {
    let current = page_translation::Entity::find()
        .filter(page_translation::Column::TenantId.eq(tenant_id))
        .filter(page_translation::Column::Locale.eq(&item.locale))
        .filter(page_translation::Column::Slug.eq(&item.slug))
        .all(txn)
        .await?;
    match current.as_slice() {
        [] => {}
        [translation] if !terminal && translation.page_id == item.page_id => {}
        _ => {
            return Err(route_history_import_conflict(
                "Historical route import overlaps a current route claim",
            ));
        }
    }

    let snapshots = page_route_publication::Entity::find()
        .filter(page_route_publication::Column::TenantId.eq(tenant_id))
        .filter(page_route_publication::Column::Locale.eq(&item.locale))
        .filter(page_route_publication::Column::Slug.eq(&item.slug))
        .all(txn)
        .await?;
    let snapshot_inserted = match snapshots.as_slice() {
        [] => {
            page_route_publication::ActiveModel {
                id: Set(Uuid::new_v4()),
                tenant_id: Set(tenant_id),
                page_id: Set(item.page_id),
                locale: Set(item.locale.clone()),
                slug: Set(item.slug.clone()),
                recorded_at: Set(Utc::now().into()),
            }
            .insert(txn)
            .await?;
            true
        }
        [snapshot] if snapshot.page_id == item.page_id => false,
        _ => {
            return Err(route_history_import_conflict(
                "Historical route import overlaps another retained public owner",
            ));
        }
    };

    let aliases = page_route_alias::Entity::find()
        .filter(page_route_alias::Column::TenantId.eq(tenant_id))
        .filter(page_route_alias::Column::Locale.eq(&item.locale))
        .filter(page_route_alias::Column::Slug.eq(&item.slug))
        .all(txn)
        .await?;
    if !current.is_empty() && !aliases.is_empty() {
        return Err(route_history_import_conflict(
            "Historical route import encountered ambiguous current and alias ownership",
        ));
    }

    let gone_alias_inserted = if terminal {
        match aliases.as_slice() {
            [] => {
                page_route_alias::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    tenant_id: Set(tenant_id),
                    page_id: Set(item.page_id),
                    locale: Set(item.locale.clone()),
                    slug: Set(item.slug.clone()),
                    disposition: Set(ROUTE_DISPOSITION_GONE.to_string()),
                    target_page_id: Set(None),
                    target_locale: Set(None),
                    reason: Set(HISTORICAL_ROUTE_IMPORT_REASON.to_string()),
                    created_at: Set(Utc::now().into()),
                }
                .insert(txn)
                .await?;
                true
            }
            [alias]
                if alias.page_id == item.page_id
                    && alias.disposition == ROUTE_DISPOSITION_GONE
                    && alias.target_page_id.is_none()
                    && alias.target_locale.is_none() =>
            {
                false
            }
            [alias]
                if alias.page_id == item.page_id
                    && alias.disposition == ROUTE_DISPOSITION_REDIRECT
                    && alias.target_page_id == Some(item.page_id)
                    && alias.target_locale.is_some() =>
            {
                false
            }
            _ => {
                return Err(route_history_import_conflict(
                    "Historical route import overlaps an incompatible alias claim",
                ));
            }
        }
    } else {
        match aliases.as_slice() {
            [] => false,
            [alias]
                if alias.page_id == item.page_id
                    && alias.disposition == ROUTE_DISPOSITION_REDIRECT
                    && alias.target_page_id == Some(item.page_id)
                    && alias.target_locale.is_some() =>
            {
                false
            }
            _ => {
                return Err(route_history_import_conflict(
                    "Historical route import overlaps a terminal or incompatible alias claim",
                ));
            }
        }
    };

    Ok(RouteEnsureOutcome {
        snapshot_inserted,
        gone_alias_inserted,
    })
}

async fn page_has_terminal_gone_alias(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    page_id: Uuid,
) -> PagesResult<bool> {
    Ok(page_route_alias::Entity::find()
        .filter(page_route_alias::Column::TenantId.eq(tenant_id))
        .filter(page_route_alias::Column::PageId.eq(page_id))
        .filter(page_route_alias::Column::Disposition.eq(ROUTE_DISPOSITION_GONE))
        .filter(page_route_alias::Column::TargetPageId.is_null())
        .filter(page_route_alias::Column::TargetLocale.is_null())
        .one(txn)
        .await?
        .is_some())
}

fn verify_receipt(
    receipt: &page_route_history_import::Model,
    tenant_id: Uuid,
    source: &str,
    item: &PreparedImportItem,
) -> PagesResult<()> {
    if receipt.tenant_id != tenant_id
        || receipt.source != source
        || receipt.source_record_id != item.source_record_id
        || receipt.request_hash != item.request_hash
        || receipt.page_id != item.page_id
        || receipt.locale != item.locale
        || receipt.slug != item.slug
    {
        return Err(route_history_import_conflict(
            "Historical route import provenance key is already bound to another payload",
        ));
    }
    Ok(())
}

fn normalize_source(value: &str) -> PagesResult<String> {
    let source = value.trim().to_ascii_lowercase();
    if source.is_empty()
        || source.chars().count() > MAX_IMPORT_SOURCE_LEN
        || !source
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(PagesError::validation(
            "Historical route import source must use 1-64 ASCII letters, digits, dots, underscores or dashes",
        ));
    }
    Ok(source)
}

fn normalize_source_record_id(value: &str) -> PagesResult<String> {
    let source_record_id = value.trim();
    if source_record_id.is_empty()
        || source_record_id.chars().count() > MAX_IMPORT_SOURCE_RECORD_ID_LEN
        || source_record_id.chars().any(char::is_control)
    {
        return Err(PagesError::validation(
            "Historical route import source record id is invalid",
        ));
    }
    Ok(source_record_id.to_string())
}

fn import_request_hash(
    source: &str,
    source_record_id: &str,
    page_id: Uuid,
    locale: &str,
    slug: &str,
) -> String {
    let digest = Sha256::digest(
        format!(
            "page-route-history-import-v1\0{source}\0{source_record_id}\0{page_id}\0{locale}\0{slug}"
        )
        .as_bytes(),
    );
    encode_digest(&digest)
}

fn encode_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn checked_increment(value: u32, message: &'static str) -> PagesResult<u32> {
    value
        .checked_add(1)
        .ok_or_else(|| PagesError::validation(message))
}

fn route_history_import_conflict(message: impl Into<String>) -> PagesError {
    PagesError::Rich(Box::new(
        RichError::new(ErrorKind::Conflict, message.into())
            .with_user_message(
                "The historical page route import conflicts with retained route ownership or provenance.",
            )
            .with_error_code(PAGE_ROUTE_HISTORY_IMPORT_CONFLICT),
    ))
}
