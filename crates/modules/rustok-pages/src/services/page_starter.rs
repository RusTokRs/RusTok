//! Starter templates catalog (presets) for quick page creation.
use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};

use crate::entities::page_starter;
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
}

#[derive(Clone)]
pub struct PageStarterService {
    db: DatabaseConnection,
}

impl PageStarterService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Lists starters for a tenant, optionally filtered by category.
    /// If no tenant-specific starters exist in the database, returns the standard built-in starters.
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: &SecurityContext,
        category: Option<&str>,
    ) -> PagesResult<Vec<PageStarterRecord>> {
        require_scope(security, Action::Read)?;
        let mut query =
            page_starter::Entity::find().filter(page_starter::Column::TenantId.eq(tenant_id));

        if let Some(cat) = category {
            let cat = cat.trim();
            if !cat.is_empty() {
                query = query.filter(page_starter::Column::Category.eq(cat));
            }
        }

        let custom_rows: Vec<PageStarterRecord> = query
            .order_by_asc(page_starter::Column::StarterKey)
            .all(&self.db)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<PagesResult<Vec<_>>>()?;

        if !custom_rows.is_empty() {
            return Ok(custom_rows);
        }

        // Return platform built-ins if tenant has not yet customized the library
        let built_ins = builtin_starters();
        if let Some(cat) = category {
            let cat = cat.trim();
            if !cat.is_empty() {
                return Ok(built_ins
                    .into_iter()
                    .filter(|s| s.category.eq_ignore_ascii_case(cat))
                    .collect());
            }
        }
        Ok(built_ins)
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

        Ok(builtin_starters().into_iter().find(|s| s.key == key))
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
        if category.is_empty() || category.len() > 64 {
            return Err(PagesError::validation(
                "Category must be between 1 and 64 characters",
            ));
        }

        let name = input.name.trim();
        if name.is_empty() || name.len() > 255 {
            return Err(PagesError::validation(
                "Name must be between 1 and 255 characters",
            ));
        }

        let default_template_key = input
            .default_template_key
            .as_deref()
            .unwrap_or("default")
            .trim();

        // Validate that content is a decodable ProjectDocument
        let _decoded =
            fly::GrapesJsCodec::decode_value(input.content.clone()).map_err(|error| {
                PagesError::validation(format!("Invalid starter content document: {error}"))
            })?;

        let now = Utc::now();
        let existing = page_starter::Entity::find()
            .filter(page_starter::Column::TenantId.eq(tenant_id))
            .filter(page_starter::Column::StarterKey.eq(key))
            .one(&self.db)
            .await?;

        let id = match existing {
            Some(row) => {
                let id = row.id;
                let mut active: page_starter::ActiveModel = row.into();
                active.category = Set(category.to_string());
                active.name = Set(name.to_string());
                active.description = Set(input.description.clone());
                active.thumbnail_url = Set(input.thumbnail_url.clone());
                active.default_template_key = Set(default_template_key.to_string());
                active.content = Set(input.content.clone());
                active.updated_at = Set(now.into());
                active.update(&self.db).await?;
                id
            }
            None => {
                let id = Uuid::new_v4();
                page_starter::ActiveModel {
                    id: Set(id),
                    tenant_id: Set(tenant_id),
                    starter_key: Set(key.to_string()),
                    category: Set(category.to_string()),
                    name: Set(name.to_string()),
                    description: Set(input.description.clone()),
                    thumbnail_url: Set(input.thumbnail_url.clone()),
                    default_template_key: Set(default_template_key.to_string()),
                    content: Set(input.content.clone()),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await?;
                id
            }
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
        })
    }

    /// Instantiates a starter into an independent, remapped page document string.
    pub fn instantiate_starter(starter: &PageStarterRecord) -> PagesResult<String> {
        let document =
            fly::GrapesJsCodec::decode_value(starter.content.clone()).map_err(|error| {
                PagesError::validation(format!("Invalid starter document: {error}"))
            })?;
        let mut generator = fly::SequentialIdGenerator::default();
        let page_doc = document.clone_as_page(&mut generator);
        let value = fly::GrapesJsCodec::encode_value(&page_doc).map_err(|error| {
            PagesError::validation(format!("Cannot encode instantiated page: {error}"))
        })?;
        serde_json::to_string(&value).map_err(|error| {
            PagesError::validation(format!("Cannot serialize instantiated page: {error}"))
        })
    }
}

fn require_scope(security: &SecurityContext, action: Action) -> PagesResult<()> {
    if security.get_scope(Resource::Pages, action) == PermissionScope::None {
        return Err(PagesError::forbidden(
            "Access to starter catalog requires Pages permission",
        ));
    }
    Ok(())
}

fn builtin_starters() -> Vec<PageStarterRecord> {
    vec![
        PageStarterRecord {
            id: Uuid::nil(),
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
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_starters_are_valid_and_non_empty() {
        let starters = builtin_starters();
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
