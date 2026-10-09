//! Site symbol catalog maintenance for page documents.
//!
//! The canonical definition catalog for one `(tenant, locale)` lives in
//! `site_symbols`. Every body save replaces the stored catalog with the
//! document's `flySymbols` block, and every read/publish merges the stored
//! catalog back into the document before use, so definitions stay shared
//! across pages and "change everywhere" resolves at each page's next publish.

use std::collections::{BTreeMap, HashMap};

use fly::{FLY_SYMBOLS_FIELD, GrapesJsCodec, SymbolDescriptor, resolve_symbol_instances};
use rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, Set,
    Statement,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::helpers::body_revision_timestamp;
use crate::entities::{page, page_body, page_body_draft, site_symbol};
use crate::error::{PagesError, PagesResult};

const FLY_SYMBOLS_REVISION_FIELD: &str = "flySymbolsRevision";

/// Extracts the document's `flySymbols` block in document order.
pub(super) fn symbol_values_in_content(content: &str) -> PagesResult<Vec<Value>> {
    let value = parse_project_value(content)?;
    Ok(match value.get(FLY_SYMBOLS_FIELD) {
        Some(Value::Array(entries)) => entries.clone(),
        Some(_) => {
            return Err(PagesError::validation(format!(
                "Page document `{FLY_SYMBOLS_FIELD}` block must be an array"
            )));
        }
        None => Vec::new(),
    })
}

/// Whether any node in the raw project value declares a `symbolId` reference.
///
/// The scan runs on the raw JSON so instance-free documents keep their exact
/// stored bytes through the resolve step.
pub(super) fn content_has_symbol_instances(content: &str) -> PagesResult<bool> {
    let value = parse_project_value(content)?;
    Ok(value.get("pages").is_some_and(value_has_symbol_instance))
}

fn value_has_symbol_instance(value: &Value) -> bool {
    match value {
        Value::Object(map) => map
            .iter()
            .any(|(key, child)| key == "symbolId" || value_has_symbol_instance(child)),
        Value::Array(items) => items.iter().any(value_has_symbol_instance),
        _ => false,
    }
}

/// Refreshes the embedded catalog and its optimistic revision token on an
/// editor read. The returned token is required by subsequent body saves when a
/// catalog exists; stale pages cannot erase definitions created elsewhere.
pub(super) fn replace_symbol_block(content: &str, symbols: Vec<Value>) -> PagesResult<String> {
    let mut value = parse_project_value(content)?;
    let object = value
        .as_object_mut()
        .expect("parse_project_value returns an object");
    let replacement = (!symbols.is_empty()).then_some(Value::Array(symbols.clone()));
    let revision = Value::String(catalog_revision(&symbols)?);
    if object.get(FLY_SYMBOLS_FIELD) == replacement.as_ref()
        && object.get(FLY_SYMBOLS_REVISION_FIELD) == Some(&revision)
    {
        return Ok(content.to_string());
    }
    match replacement {
        Some(block) => {
            object.insert(FLY_SYMBOLS_FIELD.to_string(), block);
        }
        None => {
            object.remove(FLY_SYMBOLS_FIELD);
        }
    }
    object.insert(FLY_SYMBOLS_REVISION_FIELD.to_string(), revision);
    serde_json::to_string(&value)
        .map_err(|error| PagesError::validation(format!("unable to encode page document: {error}")))
}

fn catalog_revision(symbols: &[Value]) -> PagesResult<String> {
    let entries: BTreeMap<String, Value> = symbols
        .iter()
        .map(|value| {
            let id = SymbolDescriptor::from_value(value)
                .ok_or_else(|| PagesError::validation("invalid stored site symbol"))?
                .id;
            Ok((id, value.clone()))
        })
        .collect::<PagesResult<_>>()?;
    let bytes = serde_json::to_vec(&entries).map_err(|error| {
        PagesError::validation(format!("unable to encode site symbol catalog: {error}"))
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Expands every symbol instance into its definition content.
///
/// Instance-free documents return their exact input bytes; documents with
/// instances are decoded, resolved fail-closed (unknown references and cycles
/// are errors), and re-encoded.
pub(super) fn resolve_content_symbols(content: &str) -> PagesResult<String> {
    if !content_has_symbol_instances(content)? {
        return Ok(content.to_string());
    }
    let document = GrapesJsCodec::decode_str(content).map_err(|error| {
        PagesError::validation(format!(
            "Page Builder project cannot be decoded for symbol resolution: {error}"
        ))
    })?;
    let resolved = resolve_symbol_instances(&document).map_err(|error| {
        PagesError::validation(format!("Page Builder symbol resolution failed: {error}"))
    })?;
    let value = GrapesJsCodec::encode_value(&resolved).map_err(|error| {
        PagesError::validation(format!(
            "Page Builder project cannot be encoded after symbol resolution: {error}"
        ))
    })?;
    serde_json::to_string(&value)
        .map_err(|error| PagesError::validation(format!("unable to encode page document: {error}")))
}

/// Merges the canonical catalog into one project value and resolves instances.
pub(super) fn apply_site_symbols_to_content(
    content: &str,
    symbols: &[Value],
) -> PagesResult<String> {
    // Unlinked legacy bodies retain their exact bytes (including the absence
    // of new project extensions) across this feature cutover.
    if symbols.is_empty()
        && !content_has_symbol_instances(content)?
        && parse_project_value(content)?
            .get(FLY_SYMBOLS_FIELD)
            .is_none()
        && parse_project_value(content)?
            .get(FLY_SYMBOLS_REVISION_FIELD)
            .is_none()
    {
        return Ok(content.to_string());
    }
    let expanded = resolve_content_symbols(&replace_symbol_block(content, symbols.to_vec())?)?;
    // Definitions and the editor CAS token are authoring state, not published
    // source. Only their expanded occurrences belong in the immutable artifact.
    let mut value = parse_project_value(&expanded)?;
    if let Some(project) = value.as_object_mut() {
        project.remove(FLY_SYMBOLS_FIELD);
        project.remove(FLY_SYMBOLS_REVISION_FIELD);
    }
    serde_json::to_string(&value)
        .map_err(|error| PagesError::validation(format!("unable to encode resolved page: {error}")))
}

/// Loads the canonical symbol catalog for a tenant, keyed by locale.
pub(super) async fn load_site_symbols_by_locale<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
) -> PagesResult<HashMap<String, Vec<Value>>> {
    let rows = site_symbol::Entity::find()
        .filter(site_symbol::Column::TenantId.eq(tenant_id))
        .order_by_asc(site_symbol::Column::SymbolId)
        .all(conn)
        .await?;
    let mut by_locale: HashMap<String, Vec<Value>> = HashMap::new();
    for row in rows {
        by_locale.entry(row.locale).or_default().push(row.content);
    }
    Ok(by_locale)
}

/// Synchronizes the submitted catalog against the revision embedded in the
/// editor document. Fails closed on stale versions instead of deleting newer
/// definitions from another page. The page-body transaction owns this write.
pub(super) async fn sync_site_symbols_in_tx<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    content: &str,
) -> PagesResult<()> {
    let submitted = symbol_values_in_content(content)?;
    let project = parse_project_value(content)?;
    let token = project
        .get(FLY_SYMBOLS_REVISION_FIELD)
        .and_then(Value::as_str);
    lock_site_symbol_catalog_in_tx(conn, tenant_id, locale).await?;
    let existing = site_symbol::Entity::find()
        .filter(site_symbol::Column::TenantId.eq(tenant_id))
        .filter(site_symbol::Column::Locale.eq(locale))
        .order_by_asc(site_symbol::Column::SymbolId)
        .all(conn)
        .await?;
    let current: Vec<Value> = existing.iter().map(|row| row.content.clone()).collect();
    let revision = catalog_revision(&current)?;
    if token != Some(revision.as_str()) && !(token.is_none() && current.is_empty()) {
        return Err(PagesError::validation(
            "Site symbols changed since this page was loaded; reload and retry",
        ));
    }

    let mut desired: BTreeMap<String, (Option<String>, Value)> = BTreeMap::new();
    for value in submitted {
        let descriptor = SymbolDescriptor::from_value(&value).ok_or_else(|| {
            PagesError::validation(format!(
                "saved document `{FLY_SYMBOLS_FIELD}` entry is not a valid symbol definition"
            ))
        })?;
        if fly::validate_identifier(&descriptor.id).is_err() {
            return Err(PagesError::validation(format!(
                "saved document symbol id `{}` is not a valid identifier",
                descriptor.id
            )));
        }
        if desired
            .insert(descriptor.id.clone(), (descriptor.name, value))
            .is_some()
        {
            return Err(PagesError::validation("duplicate site symbol id"));
        }
    }
    if !desired.is_empty() || content_has_symbol_instances(content)? {
        let source = GrapesJsCodec::decode_str(content)
            .map_err(|error| PagesError::validation(format!("invalid symbol document: {error}")))?;
        resolve_symbol_instances(&source).map_err(|error| {
            PagesError::validation(format!("invalid symbol references: {error}"))
        })?;
    }
    if desired.values().map(|(_, value)| value).eq(current.iter()) {
        return Ok(());
    }

    // A definition may be removed only after all instances in other pages
    // have been removed. The current page is excluded because its submitted
    // body (already written inside this transaction) is checked directly.
    for removed in existing
        .iter()
        .filter(|row| !desired.contains_key(&row.symbol_id))
    {
        if value_has_symbol_instance_id(
            project.get("pages").unwrap_or(&Value::Null),
            &removed.symbol_id,
        ) || symbol_used_by_other_page(conn, tenant_id, page_id, locale, &removed.symbol_id)
            .await?
            || symbol_used_by_published_current_page(
                conn,
                tenant_id,
                page_id,
                locale,
                &removed.symbol_id,
            )
            .await?
        {
            return Err(PagesError::validation(format!(
                "cannot remove site symbol `{}` while a page still references it",
                removed.symbol_id
            )));
        }
    }

    site_symbol::Entity::delete_many()
        .filter(site_symbol::Column::TenantId.eq(tenant_id))
        .filter(site_symbol::Column::Locale.eq(locale))
        .exec(conn)
        .await?;
    if desired.is_empty() {
        return Ok(());
    }
    let now = body_revision_timestamp(chrono::Utc::now());
    let rows: Vec<site_symbol::ActiveModel> = desired
        .into_iter()
        .map(|(symbol_id, (name, content))| site_symbol::ActiveModel {
            tenant_id: Set(tenant_id),
            locale: Set(locale.to_string()),
            symbol_id: Set(symbol_id),
            name: Set(name),
            content: Set(content),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .collect();
    site_symbol::Entity::insert_many(rows).exec(conn).await?;
    Ok(())
}

/// Postgres serializes catalog writers for one tenant/locale before reading
/// the version. SQLite's write transaction already serializes writers.
async fn lock_site_symbol_catalog_in_tx<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    locale: &str,
) -> PagesResult<()> {
    if conn.get_database_backend() == DatabaseBackend::Postgres {
        let identity = format!("rustok-pages-site-symbols/{tenant_id}/{locale}");
        let digest = Sha256::digest(identity.as_bytes());
        let key = i64::from_be_bytes(digest[..8].try_into().expect("SHA-256 output has 32 bytes"));
        let sql = format!("SELECT pg_advisory_xact_lock({key})");
        conn.query_one(Statement::from_string(DatabaseBackend::Postgres, sql))
            .await?;
    }
    Ok(())
}

fn value_has_symbol_instance_id(value: &Value, symbol_id: &str) -> bool {
    match value {
        Value::Object(map) => {
            map.get("symbolId").and_then(Value::as_str) == Some(symbol_id)
                || map
                    .values()
                    .any(|child| value_has_symbol_instance_id(child, symbol_id))
        }
        Value::Array(items) => items
            .iter()
            .any(|child| value_has_symbol_instance_id(child, symbol_id)),
        _ => false,
    }
}

async fn symbol_used_by_other_page<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    symbol_id: &str,
) -> PagesResult<bool> {
    let bodies = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::Locale.eq(locale))
        .filter(page_body::Column::PageId.ne(page_id))
        .all(conn)
        .await?;
    for body in bodies {
        if body.format == PAGE_BUILDER_DOCUMENT_FORMAT
            && content_references_symbol(&body.content, symbol_id)?
        {
            return Ok(true);
        }
    }
    let drafts = page_body_draft::Entity::find()
        .filter(page_body_draft::Column::TenantId.eq(tenant_id))
        .filter(page_body_draft::Column::Locale.eq(locale))
        .filter(page_body_draft::Column::PageId.ne(page_id))
        .all(conn)
        .await?;
    for draft in drafts {
        if draft.format == PAGE_BUILDER_DOCUMENT_FORMAT
            && content_references_symbol(&draft.content, symbol_id)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn symbol_used_by_published_current_page<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    page_id: Uuid,
    locale: &str,
    symbol_id: &str,
) -> PagesResult<bool> {
    let published = page::Entity::find()
        .filter(page::Column::TenantId.eq(tenant_id))
        .filter(page::Column::Id.eq(page_id))
        .filter(page::Column::Status.eq("published"))
        .one(conn)
        .await?
        .is_some();
    if !published {
        return Ok(false);
    }
    let body = page_body::Entity::find()
        .filter(page_body::Column::TenantId.eq(tenant_id))
        .filter(page_body::Column::PageId.eq(page_id))
        .filter(page_body::Column::Locale.eq(locale))
        .one(conn)
        .await?;
    match body {
        Some(body) if body.format == PAGE_BUILDER_DOCUMENT_FORMAT => {
            content_references_symbol(&body.content, symbol_id)
        }
        _ => Ok(false),
    }
}

fn content_references_any_symbol(
    content: &str,
    ids: &std::collections::BTreeSet<String>,
) -> PagesResult<bool> {
    let value = parse_project_value(content)?;
    Ok(value
        .get("pages")
        .is_some_and(|pages| ids.iter().any(|id| value_has_symbol_instance_id(pages, id))))
}

fn content_references_symbol(content: &str, symbol_id: &str) -> PagesResult<bool> {
    Ok(parse_project_value(content)?
        .get("pages")
        .is_some_and(|pages| value_has_symbol_instance_id(pages, symbol_id)))
}

fn parse_project_value(content: &str) -> PagesResult<Value> {
    let value: Value = serde_json::from_str(content).map_err(|error| {
        PagesError::validation(format!("Page Builder project is not valid JSON: {error}"))
    })?;
    if !value.is_object() {
        return Err(PagesError::validation(
            "Page Builder project must be a JSON object".to_string(),
        ));
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project_with(symbols: Option<Value>, components: Value) -> String {
        let mut root = json!({
            "pages": [{ "id": "page-1", "component": { "id": "root", "type": "wrapper", "components": components } }]
        });
        if let Some(symbols) = symbols {
            root[FLY_SYMBOLS_FIELD] = symbols;
        }
        root.to_string()
    }

    #[test]
    fn replace_symbol_block_is_byte_stable_when_unchanged() {
        let symbols = vec![json!({ "id": "cta", "components": [] })];
        let content = project_with(Some(Value::Array(symbols.clone())), json!([]));
        let merged = replace_symbol_block(&content, symbols.clone()).expect("merge");
        assert_eq!(
            replace_symbol_block(&merged, symbols).expect("merge again"),
            merged
        );
        assert!(merged.contains(FLY_SYMBOLS_REVISION_FIELD));
    }

    #[test]
    fn replace_symbol_block_replaces_the_full_catalog() {
        let content = project_with(Some(json!([{ "id": "old", "components": [] }])), json!([]));
        let merged = replace_symbol_block(&content, vec![json!({ "id": "new", "components": [] })])
            .expect("merge");
        let symbols = symbol_values_in_content(&merged).expect("extract");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0]["id"], "new");
    }

    #[test]
    fn instance_free_content_resolves_byte_identically() {
        let content = project_with(Some(json!([{ "id": "cta", "components": [] }])), json!([]));
        let resolved = resolve_content_symbols(&content).expect("resolve");
        assert_eq!(resolved, content);
    }

    #[test]
    fn site_catalog_revision_is_independent_of_store_row_order() {
        let a = json!({ "id": "alpha", "components": [] });
        let b = json!({ "id": "beta", "components": [] });
        assert_eq!(
            catalog_revision(&[a.clone(), b.clone()]).expect("hash"),
            catalog_revision(&[b, a]).expect("hash")
        );
    }

    #[test]
    fn published_source_contains_expanded_occurrences_but_no_catalog() {
        let definition = json!({
            "id": "cta", "components": [{ "id": "body", "type": "text", "content": "Buy" }]
        });
        let source = project_with(
            None,
            json!([{ "id": "slot", "type": "symbol", "symbolId": "cta" }]),
        );
        let compiled = apply_site_symbols_to_content(&source, &[definition]).expect("expand");
        let document = parse_project_value(&compiled).expect("source");
        assert!(compiled.contains("Buy"));
        assert!(document.get("flySymbols").is_none());
        assert!(document.get("flySymbolsRevision").is_none());
        assert!(
            document["pages"][0]["component"]["components"][0]
                .get("symbolId")
                .is_none()
        );
        let legacy = project_with(None, json!([]));
        assert_eq!(
            apply_site_symbols_to_content(&legacy, &[]).expect("legacy"),
            legacy
        );
    }

    #[test]
    fn instances_resolve_fail_closed_and_deterministically() {
        let content = project_with(
            Some(json!([{
                "id": "cta",
                "components": [{ "id": "cta-1", "type": "text", "content": "Buy" }]
            }])),
            json!([{ "id": "slot", "type": "symbol", "symbolId": "cta" }]),
        );
        let resolved = resolve_content_symbols(&content).expect("resolve");
        assert!(resolved.contains("Buy"));
        assert!(!resolved.contains("\"symbolId\""));
        assert_eq!(resolved, resolve_content_symbols(&content).expect("again"));

        let missing = project_with(
            None,
            json!([{ "id": "slot", "type": "symbol", "symbolId": "ghost" }]),
        );
        assert!(resolve_content_symbols(&missing).is_err());
    }
}

impl super::PageService {
    /// Page ids whose current/draft bodies depend on `symbol_id`, directly or
    /// through a nested symbol definition. Results are tenant/locale scoped
    /// and require an editor-capable Pages reader (not a public page request).
    pub async fn site_symbol_usage(
        &self,
        tenant_id: Uuid,
        security: rustok_core::SecurityContext,
        locale: &str,
        symbol_id: &str,
    ) -> PagesResult<Vec<Uuid>> {
        use rustok_api::{Action, Resource};
        crate::services::rbac::enforce_scope(&security, Resource::Pages, Action::Read)?;
        if !crate::services::rbac::can_read_non_public_pages(&security) {
            return Err(PagesError::forbidden("Permission denied"));
        }
        let locale = super::helpers::normalize_locale(locale)?;
        fly::validate_identifier(symbol_id)
            .map_err(|reason| PagesError::validation(format!("invalid symbol id: {reason}")))?;
        let symbols = load_site_symbols_by_locale(&self.db, tenant_id)
            .await?
            .remove(&locale)
            .unwrap_or_default();
        let definitions: BTreeMap<String, SymbolDescriptor> = symbols
            .iter()
            .filter_map(|value| {
                SymbolDescriptor::from_value(value).map(|item| (item.id.clone(), item))
            })
            .collect();
        if !definitions.contains_key(symbol_id) {
            return Err(PagesError::validation(format!(
                "site symbol `{symbol_id}` not found"
            )));
        }
        let mut dependencies = std::collections::BTreeSet::from([symbol_id.to_string()]);
        loop {
            let prior = dependencies.len();
            for (id, definition) in &definitions {
                if fly::symbol_references(&definition.components)
                    .iter()
                    .any(|reference| dependencies.contains(reference))
                {
                    dependencies.insert(id.clone());
                }
            }
            if dependencies.len() == prior {
                break;
            }
        }
        let mut pages = std::collections::BTreeSet::new();
        for body in page_body::Entity::find()
            .filter(page_body::Column::TenantId.eq(tenant_id))
            .filter(page_body::Column::Locale.eq(&locale))
            .all(&self.db)
            .await?
        {
            if body.format == PAGE_BUILDER_DOCUMENT_FORMAT
                && content_references_any_symbol(&body.content, &dependencies)?
            {
                pages.insert(body.page_id);
            }
        }
        for draft in page_body_draft::Entity::find()
            .filter(page_body_draft::Column::TenantId.eq(tenant_id))
            .filter(page_body_draft::Column::Locale.eq(&locale))
            .all(&self.db)
            .await?
        {
            if draft.format == PAGE_BUILDER_DOCUMENT_FORMAT
                && content_references_any_symbol(&draft.content, &dependencies)?
            {
                pages.insert(draft.page_id);
            }
        }
        Ok(pages.into_iter().collect())
    }
}
