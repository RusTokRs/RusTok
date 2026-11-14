//! Starter templates catalog (presets) for quick page creation.
use chrono::Utc;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};

use crate::entities::{page, page_body, page_body_draft, page_starter, site_symbol};
use crate::services::rbac::can_read_page_source;
use super::page::{content_has_symbol_instances, normalize_locale, replace_symbol_block, resolve_content_symbols};
use crate::error::{PagesError, PagesResult};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PageStarterRecord {
    pub id: Uuid,
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub default_template_key: String,
    pub content: serde_json::Value,
    /// None for built-ins; tenant-owned rows carry a compare-and-swap token.
    pub revision: Option<Uuid>,
}

impl TryFrom<page_starter::Model> for PageStarterRecord {
    type Error = PagesError;

    fn try_from(row: page_starter::Model) -> PagesResult<Self> {
        Ok(Self {
            id: row.id,
            key: row.starter_key,
            category: row.category,
            name: row.name,
            description: row.description,
            thumbnail_url: row.thumbnail_url,
            default_template_key: row.default_template_key,
            content: row.content,
            revision: Some(row.revision),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavePageStarterInput {
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub thumbnail_url: Option<String>,
    pub default_template_key: Option<String>,
    pub content: serde_json::Value,
    /// None creates a tenant row; updates require its last observed token.
    pub expected_revision: Option<Uuid>,
}

/// Update metadata without replacing the starter document or its thumbnail.
#[derive(Clone, Debug)]
pub struct UpdatePageStarterMetadataInput {
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
    pub default_template_key: String,
    pub expected_revision: Uuid,
}

/// Create-only snapshot of one persisted page working copy, never the browser's unsaved draft.
#[derive(Clone, Debug)]
pub struct SavePageStarterFromPageInput {
    pub page_id: Uuid,
    pub locale: String,
    pub key: String,
    pub category: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SavedPageStarterSnapshot {
    pub starter: PageStarterRecord,
    pub source_locale: String,
    pub source_state: String,
    pub source_revision: String,
}

#[derive(Clone)]
pub struct PageStarterService {
    db: DatabaseConnection,
}

impl PageStarterService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Lists built-ins and tenant starters, with tenant records overriding built-ins by key.
    /// Category filtering is applied after overrides, matching `get` for every key.
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        category: Option<&str>,
    ) -> PagesResult<Vec<PageStarterRecord>> {
        require_scope(security, Action::Read)?;
        let custom_rows: Vec<PageStarterRecord> = page_starter::Entity::find()
            .filter(page_starter::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PagesResult<Vec<_>>>()?;

        let mut catalog = builtin_starters()?
            .into_iter()
            .map(|starter| (starter.key.clone(), starter))
            .collect::<std::collections::BTreeMap<_, _>>();
        for starter in custom_rows {
            catalog.insert(starter.key.clone(), starter);
        }
        let category = category.map(str::trim).filter(|value| !value.is_empty());
        Ok(catalog
            .into_values()
            .filter(|starter| {
                category.is_none_or(|value| starter.category.eq_ignore_ascii_case(value))
            })
            .collect())
    }

    /// Gets a single starter by key.
    pub async fn get(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        key: &str,
    ) -> PagesResult<Option<PageStarterRecord>> {
        require_scope(security, Action::Read)?;
        let key = key.trim();
        let custom = page_starter::Entity::find()
            .filter(page_starter::Column::TenantId.eq(tenant_id))
            .filter(page_starter::Column::StarterKey.eq(key))
            .one(&self.db)
            .await?
            .map(TryInto::try_into)
            .transpose()?;

        if custom.is_some() {
            return Ok(custom);
        }

        Ok(builtin_starters()?.into_iter().find(|s| s.key == key))
    }

    /// Capture the exact-locale persisted working copy. No unsaved browser state is read.
    /// Source reads and the starter insert are not a single revision-locked transaction;
    /// the returned token identifies the body that was actually captured.
    pub async fn save_from_page(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        input: SavePageStarterFromPageInput,
    ) -> PagesResult<SavedPageStarterSnapshot> {
        require_scope(security, Action::Manage)?;
        let locale = normalize_locale(&input.locale)?;
        let key = input.key.trim();
        if builtin_starters()?.iter().any(|starter| starter.key == key) {
            return Err(PagesError::validation("Choose a key that does not belong to a built-in starter"));
        }
        let page = page::Entity::find_by_id(input.page_id)
            .filter(page::Column::TenantId.eq(tenant_id))
            .one(&self.db).await?
            .ok_or_else(|| PagesError::validation("Source page not found"))?;
        if !can_read_page_source(security, page.author_id) {
            return Err(PagesError::forbidden("Permission denied"));
        }
        let draft = page_body_draft::Entity::find()
            .filter(page_body_draft::Column::TenantId.eq(tenant_id))
            .filter(page_body_draft::Column::PageId.eq(page.id))
            .filter(page_body_draft::Column::Locale.eq(&locale))
            .one(&self.db).await?;
        let (content, format, revision, state) = if let Some(body) = draft {
            (body.content, body.format, body.updated_at.to_string(), "draft")
        } else {
            let body = page_body::Entity::find()
                .filter(page_body::Column::TenantId.eq(tenant_id))
                .filter(page_body::Column::PageId.eq(page.id))
                .filter(page_body::Column::Locale.eq(&locale))
                .one(&self.db).await?
                .ok_or_else(|| PagesError::validation("No saved page document for this locale"))?;
            (body.content, body.format, body.updated_at.to_string(), "current")
        };
        if format != rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT {
            return Err(PagesError::validation("Source is not a Page Builder document"));
        }
        let mut content: serde_json::Value = serde_json::from_str(&content)
            .map_err(|error| PagesError::validation(format!("Invalid source document: {error}")))?;
        let object = content.as_object_mut()
            .ok_or_else(|| PagesError::validation("Invalid source document"))?;
        // Symbol definitions are shared authoring data, not part of the starter's authority.
        // Keep references only; instantiation resolves against the target locale.
        object.remove("flySymbols");
        object.remove("flySymbolsRevision");
        let starter = self.save(tenant_id, security, SavePageStarterInput {
            key: input.key, category: input.category, name: input.name,
            description: input.description, thumbnail_url: None,
            default_template_key: Some(page.template), content, expected_revision: None,
        }).await?;
        Ok(SavedPageStarterSnapshot {
            starter, source_locale: locale, source_state: state.to_string(), source_revision: revision,
        })
    }

    /// Saves a starter preset into the tenant's library (creates or updates).
    pub async fn save(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        input: SavePageStarterInput,
    ) -> PagesResult<PageStarterRecord> {
        require_scope(security, Action::Manage)?;
        let key = input.key.trim();
        fly::validate_identifier(key)
            .map_err(|err| PagesError::validation(format!("Invalid starter key: {err}")))?;

        let category = input.category.trim();
        let name = input.name.trim();
        let default_template_key = input.default_template_key.as_deref().unwrap_or("default").trim();
        validate_metadata(category, name, default_template_key)?;

        // A decodable document can still have ambiguous identities and cannot be cloned.
        // Reject it before replacing a usable starter in the tenant catalog.
        let decoded =
            fly::GrapesJsCodec::decode_value(input.content.clone()).map_err(|error| {
                PagesError::validation(format!("Invalid starter content document: {error}"))
            })?;
        decoded
            .clone_as_page(&mut fly::UniqueIdGenerator::new("page"))
            .map_err(|error| PagesError::validation(format!("Cannot instantiate starter: {error}")))?;

        let now = Utc::now();
        let revision = Uuid::new_v4();
        let id = if let Some(expected) = input.expected_revision {
            let existing = page_starter::Entity::find()
                .filter(page_starter::Column::TenantId.eq(tenant_id))
                .filter(page_starter::Column::StarterKey.eq(key))
                .filter(page_starter::Column::Revision.eq(expected))
                .one(&self.db)
                .await?
                .ok_or_else(starter_conflict)?;
            let result = page_starter::Entity::update_many()
                .col_expr(page_starter::Column::Category, Expr::value(category.to_string()))
                .col_expr(page_starter::Column::Name, Expr::value(name.to_string()))
                .col_expr(page_starter::Column::Description, Expr::value(input.description.clone()))
                .col_expr(page_starter::Column::ThumbnailUrl, Expr::value(input.thumbnail_url.clone()))
                .col_expr(page_starter::Column::DefaultTemplateKey, Expr::value(default_template_key.to_string()))
                .col_expr(page_starter::Column::Content, Expr::value(input.content.clone()))
                .col_expr(page_starter::Column::Revision, Expr::value(revision))
                .col_expr(page_starter::Column::UpdatedAt, Expr::value(now))
                .filter(page_starter::Column::TenantId.eq(tenant_id))
                .filter(page_starter::Column::StarterKey.eq(key))
                .filter(page_starter::Column::Revision.eq(expected))
                .exec(&self.db)
                .await?;
            if result.rows_affected != 1 {
                return Err(starter_conflict());
            }
            existing.id
        } else {
            let id = Uuid::new_v4();
            let inserted = page_starter::Entity::insert(page_starter::ActiveModel {
                id: Set(id),
                tenant_id: Set(tenant_id),
                starter_key: Set(key.to_string()),
                category: Set(category.to_string()),
                name: Set(name.to_string()),
                description: Set(input.description.clone()),
                thumbnail_url: Set(input.thumbnail_url.clone()),
                default_template_key: Set(default_template_key.to_string()),
                content: Set(input.content.clone()),
                revision: Set(revision),
                created_at: Set(now.into()),
                updated_at: Set(now.into()),
            })
            .on_conflict(
                OnConflict::columns([page_starter::Column::TenantId, page_starter::Column::StarterKey])
                    .do_nothing()
                    .to_owned(),
            )
            .exec_without_returning(&self.db)
            .await?;
            if inserted != 1 {
                return Err(starter_conflict());
            }
            id
        };

        Ok(PageStarterRecord {
            id,
            key: key.to_string(),
            category: category.to_string(),
            name: name.to_string(),
            description: input.description,
            thumbnail_url: input.thumbnail_url,
            default_template_key: default_template_key.to_string(),
            content: input.content,
            revision: Some(revision),
        })
    }

    /// Atomically update only metadata of an existing tenant row at its observed revision.
    /// Built-ins have no tenant row and cannot be implicitly overridden by this operation.
    pub async fn update_metadata(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        input: UpdatePageStarterMetadataInput,
    ) -> PagesResult<Uuid> {
        require_scope(security, Action::Manage)?;
        let key = input.key.trim();
        fly::validate_identifier(key)
            .map_err(|err| PagesError::validation(format!("Invalid starter key: {err}")))?;
        let category = input.category.trim();
        let name = input.name.trim();
        let layout = input.default_template_key.trim();
        validate_metadata(category, name, layout)?;
        let revision = Uuid::new_v4();
        let result = page_starter::Entity::update_many()
            .col_expr(page_starter::Column::Category, Expr::value(category.to_string()))
            .col_expr(page_starter::Column::Name, Expr::value(name.to_string()))
            .col_expr(page_starter::Column::Description, Expr::value(input.description))
            .col_expr(page_starter::Column::DefaultTemplateKey, Expr::value(layout.to_string()))
            .col_expr(page_starter::Column::Revision, Expr::value(revision))
            .col_expr(page_starter::Column::UpdatedAt, Expr::value(Utc::now()))
            .filter(page_starter::Column::TenantId.eq(tenant_id))
            .filter(page_starter::Column::StarterKey.eq(key))
            .filter(page_starter::Column::Revision.eq(input.expected_revision))
            .exec(&self.db).await?;
        if result.rows_affected != 1 {
            return Err(starter_conflict());
        }
        Ok(revision)
    }

    /// Delete a tenant-owned row only at the observed revision. Deleting an
    /// override reveals the built-in with that key; built-ins have no row.
    pub async fn delete(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        key: &str,
        expected_revision: Uuid,
    ) -> PagesResult<()> {
        require_scope(security, Action::Manage)?;
        let key = key.trim();
        fly::validate_identifier(key)
            .map_err(|err| PagesError::validation(format!("Invalid starter key: {err}")))?;
        let result = page_starter::Entity::delete_many()
            .filter(page_starter::Column::TenantId.eq(tenant_id))
            .filter(page_starter::Column::StarterKey.eq(key))
            .filter(page_starter::Column::Revision.eq(expected_revision))
            .exec(&self.db)
            .await?;
        if result.rows_affected != 1 {
            return Err(starter_conflict());
        }
        Ok(())
    }

    /// Instantiates the exact catalog revision the caller selected. A null token
    /// means the caller selected a built-in, not an arbitrary tenant override.
    pub async fn instantiate_selected(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        key: &str,
        expected_revision: Option<Uuid>,
        locale: &str,
    ) -> PagesResult<String> {
        let locale = normalize_locale(locale)?;
        let starter = self.get(tenant_id, security, key).await?
            .ok_or_else(|| PagesError::validation("Starter preset not found"))?;
        if starter.revision != expected_revision {
            return Err(starter_conflict());
        }
        let document = Self::instantiate_starter(&starter)?;
        if !content_has_symbol_instances(&document)? {
            // A catalog embedded in a starter is not authority to modify site symbols.
            let mut value: serde_json::Value = serde_json::from_str(&document)
                .map_err(|error| PagesError::validation(format!("Invalid instantiated document: {error}")))?;
            let object = value.as_object_mut().ok_or_else(|| PagesError::validation("Invalid instantiated document"))?;
            object.remove("flySymbols");
            object.remove("flySymbolsRevision");
            return serde_json::to_string(&value)
                .map_err(|error| PagesError::validation(format!("Cannot serialize instantiated page: {error}")));
        }
        // The response contains the full site catalog, whose authoring API requires manage.
        // Do not disclose it through the starter read endpoint to read-only principals.
        require_scope(security, Action::Manage)?;
        let symbols = site_symbol::Entity::find()
            .filter(site_symbol::Column::TenantId.eq(tenant_id))
            .filter(site_symbol::Column::Locale.eq(locale))
            .order_by_asc(site_symbol::Column::SymbolId)
            .all(&self.db)
            .await?
            .into_iter().map(|row| row.content).collect();
        let document = replace_symbol_block(&document, symbols)?;
        // Validate references without replacing linked instances in the authoring document.
        resolve_content_symbols(&document)?;
        Ok(document)
    }

    /// Raw identity-remapping helper. Use `instantiate_selected` before creating a page:
    /// this helper does not reconcile embedded site symbols with the target locale.
    pub fn instantiate_starter(starter: &PageStarterRecord) -> PagesResult<String> {
        let document =
            fly::GrapesJsCodec::decode_value(starter.content.clone()).map_err(|error| {
                PagesError::validation(format!("Invalid starter document: {error}"))
            })?;
        let mut generator = fly::UniqueIdGenerator::new("page");
        let page_doc = document.clone_as_page(&mut generator).map_err(|error| {
            PagesError::validation(format!("Cannot instantiate starter: {error}"))
        })?;
        let value = fly::GrapesJsCodec::encode_value(&page_doc).map_err(|error| {
            PagesError::validation(format!("Cannot encode instantiated page: {error}"))
        })?;
        serde_json::to_string(&value).map_err(|error| {
            PagesError::validation(format!("Cannot serialize instantiated page: {error}"))
        })
    }
}

fn validate_metadata(category: &str, name: &str, layout: &str) -> PagesResult<()> {
    if category.is_empty() || category.len() > 64 {
        return Err(PagesError::validation("Category must be between 1 and 64 characters"));
    }
    if name.is_empty() || name.len() > 255 {
        return Err(PagesError::validation("Name must be between 1 and 255 characters"));
    }
    fly::validate_identifier(layout)
        .map_err(|err| PagesError::validation(format!("Invalid default template key: {err}")))?;
    Ok(())
}

fn starter_conflict() -> PagesError {
    PagesError::StarterRevisionConflict
}

fn require_scope(security: &SecurityContext, action: Action) -> PagesResult<()> {
    if security.is_public_read() || security.get_scope(Resource::Pages, action) != PermissionScope::All {
        return Err(PagesError::forbidden(
            "Starter catalog requires tenant-wide Pages permission",
        ));
    }
    Ok(())
}

fn builtin_starters() -> PagesResult<Vec<PageStarterRecord>> {
    let mut starters = vec![
        PageStarterRecord {
            id: Uuid::nil(),
            revision: None,
            key: "landing-product".to_string(),
            category: "landing".to_string(),
            name: "SaaS & Product Launch".to_string(),
            description: Some(
                "High-converting landing page with Hero, Feature Grid, Testimonials and CTA."
                    .to_string(),
            ),
            thumbnail_url: Some("/static/presets/landing-product.svg".to_string()),
            default_template_key: "default".to_string(),
            content: serde_json::json!({
                "flyDocumentKind": "starter_preset",
                "pages": [{
                    "id": "home",
                    "flyPageMeta": {
                        "title": "Welcome to Our Platform",
                        "description": "Next-generation digital experience.",
                        "slug": "welcome"
                    },
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [
                            {
                                "id": "hero-section",
                                "type": "section",
                                "flyLandingSection": "hero",
                                "style": { "padding": "80px 24px", "text-align": "center", "background": "#0f172a", "color": "#ffffff" },
                                "components": [
                                    { "id": "hero-h1", "type": "heading", "tagName": "h1", "flyLandingProperty": "headline", "content": "Build Faster with RusToK", "style": { "font-size": "48px", "margin": "0 0 16px" } },
                                    { "id": "hero-p", "type": "text", "flyLandingProperty": "body", "content": "Unified enterprise e-commerce and modular visual site builder.", "style": { "font-size": "20px", "margin": "0 0 32px", "color": "#94a3b8" } },
                                    { "id": "hero-cta", "type": "button", "tagName": "a", "flyLandingProperty": "primary_action", "content": "Get Started Free", "style": { "background": "#2563eb", "color": "#ffffff", "padding": "14px 28px", "border-radius": "8px", "font-weight": "700", "text-decoration": "none" } }
                                ]
                            },
                            {
                                "id": "features-section",
                                "type": "section",
                                "flyLandingSection": "feature_grid",
                                "style": { "padding": "72px 24px", "background": "#f8fafc" },
                                "components": [
                                    { "id": "feat-h2", "type": "heading", "tagName": "h2", "flyLandingProperty": "headline", "content": "Engineered for Performance", "style": { "font-size": "36px", "text-align": "center", "margin": "0 0 48px" } },
                                    {
                                        "id": "feat-grid",
                                        "type": "grid",
                                        "style": { "display": "grid", "grid-template-columns": "repeat(3, 1fr)", "gap": "24px", "max-width": "1120px", "margin": "0 auto" },
                                        "components": [
                                            { "id": "card-1", "type": "column", "style": { "padding": "24px", "background": "#ffffff", "border-radius": "12px", "border": "1px solid #e2e8f0" }, "components": [{ "id": "c1-h3", "type": "heading", "tagName": "h3", "content": "High-Speed SSR" }, { "id": "c1-p", "type": "text", "content": "Zero JavaScript overhead with pre-compiled landing artifacts." }] },
                                            { "id": "card-2", "type": "column", "style": { "padding": "24px", "background": "#ffffff", "border-radius": "12px", "border": "1px solid #e2e8f0" }, "components": [{ "id": "c2-h3", "type": "heading", "tagName": "h3", "content": "Transactional Outbox" }, { "id": "c2-p", "type": "text", "content": "Guaranteed delivery and immediate Next.js ISR cache invalidation." }] },
                                            { "id": "card-3", "type": "column", "style": { "padding": "24px", "background": "#ffffff", "border-radius": "12px", "border": "1px solid #e2e8f0" }, "components": [{ "id": "c3-h3", "type": "heading", "tagName": "h3", "content": "Sovereign Multi-Tenancy" }, { "id": "c3-p", "type": "text", "content": "Database-enforced isolation per tenant and exact locale." }] }
                                        ]
                                    }
                                ]
                            },
                            {
                                "id": "cta-section",
                                "type": "section",
                                "flyLandingSection": "call_to_action",
                                "style": { "padding": "72px 24px", "text-align": "center", "background": "#1e293b", "color": "#ffffff" },
                                "components": [
                                    { "id": "cta-h2", "type": "heading", "tagName": "h2", "flyLandingProperty": "headline", "content": "Ready to Launch Your Experience?", "style": { "font-size": "36px", "margin": "0 0 16px" } },
                                    { "id": "cta-p", "type": "text", "flyLandingProperty": "body", "content": "Deploy your localized pages with verified cryptographic provenance.", "style": { "font-size": "18px", "margin": "0 0 32px", "color": "#cbd5e1" } }
                                ]
                            }
                        ]
                    }
                }]
            }),
        },
        PageStarterRecord {
            id: Uuid::nil(),
            revision: None,
            key: "about-company".to_string(),
            category: "about".to_string(),
            name: "About Company & Team".to_string(),
            description: Some(
                "Tell your story, introduce your leadership team and share company values."
                    .to_string(),
            ),
            thumbnail_url: Some("/static/presets/about-company.svg".to_string()),
            default_template_key: "default".to_string(),
            content: serde_json::json!({
                "flyDocumentKind": "starter_preset",
                "pages": [{
                    "id": "about",
                    "flyPageMeta": {
                        "title": "About Our Company",
                        "description": "Our history, team and mission.",
                        "slug": "about-us"
                    },
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [
                            {
                                "id": "about-hero",
                                "type": "section",
                                "style": { "padding": "64px 24px", "max-width": "800px", "margin": "0 auto" },
                                "components": [
                                    { "id": "about-h1", "type": "heading", "tagName": "h1", "content": "Our Mission & Values", "style": { "font-size": "44px", "margin": "0 0 20px" } },
                                    { "id": "about-p", "type": "text", "content": "We build open-source enterprise infrastructure for high-scale commerce and content publishing.", "style": { "font-size": "20px", "line-height": "1.6", "color": "#475569" } }
                                ]
                            }
                        ]
                    }
                }]
            }),
        },
        PageStarterRecord {
            id: Uuid::nil(),
            revision: None,
            key: "contact-simple".to_string(),
            category: "contact".to_string(),
            name: "Contact & Support".to_string(),
            description: Some(
                "Clean contact form with embedded messaging and contact information.".to_string(),
            ),
            thumbnail_url: Some("/static/presets/contact-simple.svg".to_string()),
            default_template_key: "default".to_string(),
            content: serde_json::json!({
                "flyDocumentKind": "starter_preset",
                "pages": [{
                    "id": "contact",
                    "flyPageMeta": {
                        "title": "Contact Us",
                        "description": "Get in touch with our team.",
                        "slug": "contact"
                    },
                    "component": {
                        "id": "root",
                        "type": "wrapper",
                        "components": [
                            {
                                "id": "contact-section",
                                "type": "section",
                                "flyLandingSection": "contact_form",
                                "style": { "padding": "64px 24px", "max-width": "720px", "margin": "0 auto" },
                                "components": [
                                    { "id": "contact-h1", "type": "heading", "tagName": "h1", "flyLandingProperty": "headline", "content": "Get in Touch", "style": { "font-size": "40px", "margin": "0 0 16px" } },
                                    { "id": "contact-p", "type": "text", "flyLandingProperty": "body", "content": "Have a question or looking for enterprise support? Send us a message.", "style": { "font-size": "18px", "color": "#64748b", "margin": "0 0 32px" } }
                                ]
                            }
                        ]
                    }
                }]
            }),
        },
    ];
    starters.extend(super::starter_presets::catalog()?);
    Ok(starters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_starters_are_valid_and_non_empty() {
        let starters = builtin_starters().expect("built-ins must decode");
        assert!(!starters.is_empty());
        for starter in &starters {
            assert!(!starter.key.is_empty());
            assert!(!starter.name.is_empty());
            assert!(!starter.category.is_empty());
            let instantiated = PageStarterService::instantiate_starter(starter)
                .expect("built-in starter must instantiate");
            assert!(
                instantiated.contains("\"type\":\"page\"")
                    || instantiated.contains("\"flyDocumentKind\":\"page\"")
            );
        }
    }
}
