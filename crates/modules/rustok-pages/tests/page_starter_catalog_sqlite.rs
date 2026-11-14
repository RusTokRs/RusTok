//! Page starter presets catalog and instantiation tests.
use std::error::Error;
use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext};
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use rustok_pages::dto::{CreatePageInput, PageBodyInput, PageTranslationInput};
use rustok_pages::{PageService, PageStarterService, PagesModule, SavePageStarterInput};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use serde_json::json;
use uuid::Uuid;

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

async fn setup_db(tenant_id: Uuid) -> TestResult<DatabaseConnection> {
    setup_db_with_connections(tenant_id, 1).await
}

async fn setup_db_with_connections(tenant_id: Uuid, connections: u32) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(format!(
        "sqlite:file:page_starters_{}?mode=memory&cache=shared",
        Uuid::new_v4()
    ));
    options
        .max_connections(connections)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await?;

    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
        .await?;
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_id}')"))
        .await?;

    db.execute_unprepared(
        "CREATE TABLE tenant_modules (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            module_slug TEXT NOT NULL,
            enabled INTEGER NOT NULL,
            settings TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
    )
    .await?;

    db.execute_unprepared(
        "CREATE TABLE pages (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            author_id TEXT,
            status TEXT NOT NULL,
            template TEXT NOT NULL,
            metadata TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            published_at TEXT,
            archived_at TEXT,
            version INTEGER NOT NULL
        )",
    )
    .await?;

    let manager = SchemaManager::new(&db);
    let _ = rustok_outbox::SysEventsMigration.up(&manager).await;
    let migrations = PagesModule.migrations();
    // Run the additive migrations for bodies, translations, symbols, templates, and starters
    for migration in migrations {
        if migration.name().ends_with("create_page_starters")
            || migration.name().ends_with("add_page_starter_revision")
        {
            migration.up(&manager).await?;
        } else {
            let _ = migration.up(&manager).await;
        }
    }
    Ok(db)
}

#[tokio::test]
async fn starter_catalog_returns_builtins_when_tenant_has_no_overrides() -> TestResult<()> {
    let tenant_id = Uuid::new_v4();
    let db = setup_db(tenant_id).await?;
    let starter_service = PageStarterService::new(db);
    let actor = SecurityContext::system();

    let all = starter_service.list(tenant_id, &actor, None).await?;
    for key in ["blank", "saas_landing", "ecommerce_showcase", "lead_capture", "link_in_bio", "company_contact", "landing-product", "about-company", "contact-simple"] {
        let starter = all.iter().find(|starter| starter.key == key)
            .unwrap_or_else(|| panic!("missing built-in starter {key}"));
        assert!(starter.revision.is_none());
        assert!(starter_service.instantiate_selected(tenant_id, &actor, key, None, "en").await.is_ok());
    }

    let landing_only = starter_service
        .list(tenant_id, &actor, Some("landing"))
        .await?;
    assert!(landing_only.iter().any(|starter| starter.key == "landing-product"));
    assert!(landing_only.iter().any(|starter| starter.key == "saas_landing"));
    assert!(landing_only.iter().all(|starter| starter.category == "landing"));

    let about = starter_service
        .get(tenant_id, &actor, "about-company")
        .await?;
    assert!(about.is_some());
    assert_eq!(about.unwrap().category, "about");

    Ok(())
}

#[tokio::test]
async fn custom_starters_are_tenant_isolated_and_instantiate_into_clean_pages() -> TestResult<()> {
    let tenant_a = Uuid::new_v4();
    let tenant_b = Uuid::new_v4();
    let db = setup_db(tenant_a).await?;
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{tenant_b}')"))
        .await?;

    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let page_service = PageService::new(db.clone(), event_bus);
    let starter_service = PageStarterService::new(db);
    let actor = SecurityContext::system();

    // 1. Tenant A saves a custom starter
    let custom = starter_service
        .save(
            tenant_a,
            &actor,
            SavePageStarterInput {
                key: "custom-promo".to_string(),
                category: "marketing".to_string(),
                name: "Custom Summer Promo".to_string(),
                description: Some("Tenant-specific promotional blueprint.".to_string()),
                thumbnail_url: None,
                default_template_key: Some("default".to_string()),
                expected_revision: None,
                content: json!({
                    "flyDocumentKind": "starter_preset",
                    "pages": [{
                        "id": "p1",
                        "component": {
                            "id": "root",
                            "type": "wrapper",
                            "components": [
                                { "id": "promo-h1", "type": "heading", "tagName": "h1", "content": "Summer Deal" },
                                { "id": "promo-text", "type": "text", "content": "Limited time offer." }
                            ]
                        }
                    }]
                }),
            },
        )
        .await?;

    assert_eq!(custom.key, "custom-promo");

    // Tenant A sees its custom starter alongside the built-ins.
    let starters_a = starter_service.list(tenant_a, &actor, None).await?;
    assert!(starters_a.iter().any(|starter| starter.key == "custom-promo"));
    assert!(starters_a.iter().any(|starter| starter.key == "landing-product"));

    // Tenant B sees default built-ins (tenant isolation)
    let starters_b = starter_service.list(tenant_b, &actor, None).await?;
    assert!(
        starters_b.iter().all(|s| s.key != "custom-promo"),
        "Tenant B must not see Tenant A's custom starter"
    );

    // 2. Instantiate custom starter into a clean, remapped page document
    let instantiated_json = PageStarterService::instantiate_starter(&custom)?;
    let parsed: serde_json::Value = serde_json::from_str(&instantiated_json)?;

    // Remapped IDs must differ from original
    let comp_ids = parsed["pages"][0]["component"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert!(!comp_ids.contains(&"promo-h1".to_string()));
    assert!(!comp_ids.contains(&"promo-text".to_string()));
    let second: serde_json::Value =
        serde_json::from_str(&PageStarterService::instantiate_starter(&custom)?)?;
    let second_ids = second["pages"][0]["component"]["components"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(second_ids.iter().all(|id| !comp_ids.iter().any(|first| first.as_str() == *id)));

    // 3. Create a real page using the instantiated starter
    let page = page_service
        .create(
            tenant_a,
            actor.clone(),
            CreatePageInput {
                translations: vec![PageTranslationInput {
                    locale: "en".to_string(),
                    title: "Summer Promotion Page".to_string(),
                    slug: Some("summer-promo".to_string()),
                    meta_title: None,
                    meta_description: None,
                }],
                template: Some("default".to_string()),
                body: Some(PageBodyInput {
                    locale: "en".to_string(),
                    document: parsed,
                }),
                channel_slugs: None,
                publish: false,
            },
        )
        .await?;

    assert_eq!(page.template, "default");
    assert!(page.body.is_some());
    let page_body = page.body.unwrap();
    assert!(page_body.content.contains("Summer Deal"));
    assert!(page_body.content.contains("Limited time offer."));

    Ok(())
}

#[tokio::test]
async fn overrides_are_filtered_after_merging_and_invalid_updates_are_rejected() -> TestResult<()> {
    let tenant = Uuid::new_v4();
    let service = PageStarterService::new(setup_db(tenant).await?);
    let actor = SecurityContext::system();
    let input = SavePageStarterInput {
        key: "landing-product".into(),
        category: "custom".into(),
        name: "Custom landing".into(),
        description: None,
        thumbnail_url: None,
        default_template_key: Some("default".into()),
        expected_revision: None,
        content: json!({
            "flyDocumentKind": "starter_preset",
            "pages": [{"id": "p", "component": {
                "id": "root", "type": "wrapper", "components": [
                    {"id": "heading", "type": "heading", "content": "Original"}
                ]
            }}]
        }),
    };
    let saved = service.save(tenant, &actor, input.clone()).await?;
    let landing = service.list(tenant, &actor, Some("landing")).await?;
    assert!(!landing.iter().any(|row| row.key == "landing-product"));
    let custom = service.list(tenant, &actor, Some("CUSTOM")).await?;
    assert_eq!(custom.len(), 1);
    assert_eq!(custom[0].name, "Custom landing");
    assert_eq!(service.get(tenant, &actor, "landing-product").await?.unwrap(), custom[0]);
    assert!(service.list(tenant, &actor, None).await?.iter().any(|row| row.key == "about-company"));

    let mut invalid_layout = input.clone();
    invalid_layout.expected_revision = saved.revision;
    invalid_layout.default_template_key = Some("bad layout key".into());
    assert!(service.save(tenant, &actor, invalid_layout).await.is_err());

    let mut invalid = input;
    invalid.expected_revision = saved.revision;
    invalid.name = "Broken replacement".into();
    invalid.content["pages"][0]["component"]["components"] = json!([
        {"id": "duplicate", "type": "text", "content": "one"},
        {"id": "duplicate", "type": "text", "content": "two"}
    ]);
    assert!(service.save(tenant, &actor, invalid).await.is_err());
    assert_eq!(service.get(tenant, &actor, "landing-product").await?.unwrap().name, "Custom landing");
    Ok(())
}

#[tokio::test]
async fn starter_writes_and_deletes_are_revision_guarded_and_tenant_scoped() -> TestResult<()> {
    let tenant = Uuid::new_v4();
    let other = Uuid::new_v4();
    let service = PageStarterService::new(setup_db(tenant).await?);
    let actor = SecurityContext::system();
    assert!(service.list(tenant, &SecurityContext::public_read(), None).await.is_err());
    assert!(service.get(tenant, &SecurityContext::public_read(), "landing-product").await.is_err());
    let input = SavePageStarterInput {
        key: "landing-product".into(),
        category: "custom".into(),
        name: "Override".into(),
        description: None,
        thumbnail_url: None,
        default_template_key: None,
        content: json!({"flyDocumentKind": "starter_preset", "pages": [{
            "id": "p", "component": {"id": "root", "type": "wrapper", "components": [
                {"id": "title", "type": "heading", "content": "Hello"}
            ]}
        }]}),
        expected_revision: None,
    };
    let created = service.save(tenant, &actor, input.clone()).await?;
    let first = created.revision.unwrap();
    assert!(service.instantiate_selected(tenant, &actor, &input.key, None, "en").await.is_err());
    assert!(service.instantiate_selected(tenant, &actor, &input.key, Some(first), "en").await.is_ok());
    assert!(service.save(tenant, &actor, input.clone()).await.is_err());
    assert!(service.delete(other, &actor, &input.key, first).await.is_err());
    assert!(service.delete(tenant, &actor, &input.key, Uuid::new_v4()).await.is_err());

    let mut update = input.clone();
    update.expected_revision = Some(first);
    update.name = "Updated".into();
    let updated = service.save(tenant, &actor, update.clone()).await?;
    let second = updated.revision.unwrap();
    assert_ne!(first, second);
    assert!(service.instantiate_selected(tenant, &actor, &input.key, Some(first), "en").await.is_err());
    assert!(service.save(tenant, &actor, update.clone()).await.is_err());
    assert!(service.delete(tenant, &actor, &input.key, first).await.is_err());
    assert_eq!(service.get(tenant, &actor, &input.key).await?.unwrap().name, "Updated");
    service.delete(tenant, &actor, &input.key, second).await?;
    let builtin = service.get(tenant, &actor, &input.key).await?.unwrap();
    assert!(builtin.revision.is_none());
    assert_ne!(builtin.name, "Updated");
    assert!(service.instantiate_selected(tenant, &actor, &input.key, None, "en").await.is_ok());

    let recreated = service.save(tenant, &actor, input.clone()).await?;
    assert_ne!(recreated.revision, Some(second));
    assert!(service.delete(tenant, &actor, &input.key, second).await.is_err());
    assert!(service.save(tenant, &actor, update).await.is_err());
    assert_eq!(service.get(tenant, &actor, &input.key).await?.unwrap().revision, recreated.revision);

    let mut left = input.clone();
    left.expected_revision = recreated.revision;
    left.name = "Left".into();
    let mut right = left.clone();
    right.name = "Right".into();
    let (left_result, right_result) = tokio::join!(
        service.save(tenant, &actor, left),
        service.save(tenant, &actor, right),
    );
    assert_ne!(left_result.is_ok(), right_result.is_ok());
    Ok(())
}

#[tokio::test]
async fn starter_revision_migration_backfills_existing_rows() -> TestResult<()> {
    let db = Database::connect("sqlite::memory:").await?;
    let manager = SchemaManager::new(&db);
    let migrations = PagesModule.migrations();
    let create = migrations.iter().find(|migration| migration.name().ends_with("create_page_starters")).unwrap();
    let revision = migrations.iter().find(|migration| migration.name().ends_with("add_page_starter_revision")).unwrap();
    create.up(&manager).await?;
    let id = Uuid::new_v4();
    let tenant = Uuid::new_v4();
    db.execute_unprepared(&format!(
        "INSERT INTO page_starters (id, tenant_id, starter_key, category, name, content) VALUES ('{id}', '{tenant}', 'existing', 'other', 'Existing', '{{\"pages\":[]}}')"
    )).await?;
    revision.up(&manager).await?;
    let service = PageStarterService::new(db);
    let row = service.get(tenant, &SecurityContext::system(), "existing").await?.unwrap();
    assert_eq!(row.revision, Some(id));
    Ok(())
}

#[tokio::test]
async fn starter_symbols_use_current_exact_locale_without_disclosing_catalog_to_readers() -> TestResult<()> {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use rustok_pages::{SaveSiteSymbolInput, SiteSymbolService};

    let tenant = Uuid::new_v4();
    let db = setup_db(tenant).await?;
    let actor = SecurityContext::system();
    let service = PageStarterService::new(db.clone());
    let symbols = SiteSymbolService::new(db.clone());
    let current = json!({"id": "cta", "components": [{"id": "copy", "type": "text", "content": "Current"}]});
    symbols.save(tenant, &actor, SaveSiteSymbolInput {
        symbol_id: "cta".into(), locale: "en".into(), name: None, content: current.clone(),
    }).await?;
    let starter = service.save(tenant, &actor, SavePageStarterInput {
        key: "linked".into(), category: "custom".into(), name: "Linked".into(),
        description: None, thumbnail_url: None, default_template_key: None,
        content: json!({
            "pages": [{"id": "page-1", "component": {"id": "root", "type": "wrapper", "components": [
                {"id": "slot", "type": "symbol", "symbolId": "cta"}
            ]}}],
            "flySymbols": [{"id": "cta", "components": [{"id": "copy", "type": "text", "content": "Stale"}]}],
            "flySymbolsRevision": "stale"
        }), expected_revision: None,
    }).await?;
    let read_only = SecurityContext::from_permissions(UserRole::Manager, Some(Uuid::new_v4()), [Permission::PAGES_READ]);
    assert!(service.instantiate_selected(tenant, &read_only, "linked", starter.revision, "en").await.is_err());
    assert!(service.instantiate_selected(tenant, &actor, "linked", starter.revision, "ru").await.is_err());
    let document: serde_json::Value = serde_json::from_str(&service.instantiate_selected(tenant, &actor, "linked", starter.revision, "en").await?)?;
    assert_eq!(document["flySymbols"][0], current);
    assert_ne!(document["flySymbolsRevision"], "stale");
    assert_eq!(document["pages"][0]["component"]["components"][0]["symbolId"], "cta");

    let changed = json!({"id": "cta", "components": [{"id": "copy", "type": "text", "content": "Changed"}]});
    symbols.save(tenant, &actor, SaveSiteSymbolInput {
        symbol_id: "cta".into(), locale: "en".into(), name: None, content: changed.clone(),
    }).await?;
    let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let pages = PageService::new(db, event_bus);
    let create_input = |document| CreatePageInput {
        translations: vec![PageTranslationInput { locale: "en".into(), title: "Linked".into(), slug: Some("linked".into()), meta_title: None, meta_description: None }],
        template: None, body: Some(PageBodyInput { locale: "en".into(), document }), channel_slugs: None, publish: false,
    };
    assert!(pages.create(tenant, actor.clone(), create_input(document)).await.is_err());
    assert_eq!(symbols.list(tenant, &actor, "en").await?[0].content, changed);
    let fresh: serde_json::Value = serde_json::from_str(&service.instantiate_selected(tenant, &actor, "linked", starter.revision, "en").await?)?;
    pages.create(tenant, actor.clone(), create_input(fresh)).await?;
    assert_eq!(symbols.list(tenant, &actor, "en").await?[0].content, changed);
    Ok(())
}

#[tokio::test]
async fn unlinked_starter_does_not_install_its_embedded_symbol_catalog() -> TestResult<()> {
    let tenant = Uuid::new_v4();
    let service = PageStarterService::new(setup_db(tenant).await?);
    let actor = SecurityContext::system();
    let starter = service.save(tenant, &actor, SavePageStarterInput {
        key: "unlinked".into(), category: "custom".into(), name: "Unlinked".into(),
        description: None, thumbnail_url: None, default_template_key: None,
        content: json!({"pages": [{"id": "p", "component": {"id": "root", "type": "wrapper", "components": []}}],
            "flySymbols": [{"id": "old", "components": []}], "flySymbolsRevision": "stale"}),
        expected_revision: None,
    }).await?;
    let document: serde_json::Value = serde_json::from_str(&service.instantiate_selected(tenant, &actor, "unlinked", starter.revision, "en").await?)?;
    assert!(document.get("flySymbols").is_none());
    assert!(document.get("flySymbolsRevision").is_none());
    Ok(())
}

#[tokio::test]
async fn saved_page_snapshot_is_exact_locale_create_only_and_strips_symbol_authority() -> TestResult<()> {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use rustok_pages::services::page_starter::SavePageStarterFromPageInput;
    let tenant = Uuid::new_v4();
    let other = Uuid::new_v4();
    let db = setup_db(tenant).await?;
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{other}')")).await?;
    let actor = SecurityContext::system();
    let bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let pages = PageService::new(db.clone(), bus);
    let page = pages.create(tenant, actor.clone(), CreatePageInput {
        translations: vec![PageTranslationInput { locale: "en".into(), title: "Source".into(), slug: Some("source".into()), meta_title: None, meta_description: None }],
        template: Some("default".into()), channel_slugs: None, publish: false,
        body: Some(PageBodyInput { locale: "en".into(), document: json!({
            "pages": [{"id": "p", "component": {"id": "root", "type": "wrapper", "components": [{"id": "text", "type": "text", "content": "Persisted"}]}}],
            "flySymbols": [{"id": "shared", "components": []}]
        }) }),
    }).await?;
    let starters = PageStarterService::new(db.clone());
    let input = || SavePageStarterFromPageInput { page_id: page.id, locale: "en".into(), key: "saved-source".into(), category: "custom".into(), name: "Saved".into(), description: None };
    let reader = SecurityContext::from_permissions(UserRole::Manager, Some(Uuid::new_v4()), [Permission::PAGES_READ]);
    assert!(starters.save_from_page(tenant, &reader, input()).await.is_err());
    assert!(starters.save_from_page(other, &actor, input()).await.is_err());
    let mut wrong_locale = input(); wrong_locale.locale = "ru".into();
    assert!(starters.save_from_page(tenant, &actor, wrong_locale).await.is_err());
    let mut builtin = input(); builtin.key = "blank".into();
    assert!(starters.save_from_page(tenant, &actor, builtin).await.is_err());
    let result = starters.save_from_page(tenant, &actor, input()).await?;
    assert_eq!(result.source_locale, "en");
    assert_eq!(result.source_state, "current");
    assert!(!result.source_revision.is_empty());
    assert!(result.starter.content.to_string().contains("Persisted"));
    assert!(result.starter.content.get("flySymbols").is_none());
    assert!(result.starter.content.get("flySymbolsRevision").is_none());
    assert!(starters.save_from_page(tenant, &actor, input()).await.is_err());

    // A saved draft overlays the current body, with no locale fallback.
    use sea_orm::{ActiveModelTrait, Set};
    let now = chrono::Utc::now().into();
    rustok_pages::entities::page_body_draft::ActiveModel {
        id: Set(Uuid::new_v4()), tenant_id: Set(tenant), page_id: Set(page.id), locale: Set("en".into()),
        content: Set(json!({"pages": [{"id": "p", "component": {"id": "root", "type": "wrapper", "components": [{"id": "text", "type": "text", "content": "Draft"}]}}]}).to_string()),
        format: Set(rustok_page_builder::PAGE_BUILDER_DOCUMENT_FORMAT.into()),
        created_at: Set(now), updated_at: Set(now), created_by: Set(None),
    }.insert(&db).await?;
    let mut draft_input = input(); draft_input.key = "saved-draft".into();
    let draft = starters.save_from_page(tenant, &actor, draft_input).await?;
    assert_eq!(draft.source_state, "draft");
    assert!(draft.starter.content.to_string().contains("Draft"));
    Ok(())
}

#[tokio::test]
async fn metadata_updates_are_cas_guarded_and_preserve_document_and_thumbnail() -> TestResult<()> {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use rustok_pages::services::page_starter::UpdatePageStarterMetadataInput;
    let tenant = Uuid::new_v4();
    let other = Uuid::new_v4();
    let service = PageStarterService::new(setup_db(tenant).await?);
    let actor = SecurityContext::system();
    let created = service.save(tenant, &actor, SavePageStarterInput {
        key: "landing-product".into(), category: "custom".into(), name: "Original".into(),
        description: Some("Old".into()), thumbnail_url: Some("https://example.com/image.png".into()),
        default_template_key: Some("default".into()), expected_revision: None,
        content: json!({"pages": [{"id": "p", "component": {"id": "root", "type": "wrapper", "components": []}}]}),
    }).await?;
    let revision = created.revision.unwrap();
    let input = UpdatePageStarterMetadataInput {
        key: created.key.clone(), category: "news".into(), name: "Updated".into(),
        description: Some("New description".into()), default_template_key: "new-layout".into(), expected_revision: revision,
    };
    let reader = SecurityContext::from_permissions(UserRole::Manager, Some(Uuid::new_v4()), [Permission::PAGES_READ]);
    assert!(service.update_metadata(tenant, &reader, input.clone()).await.is_err());
    assert!(service.update_metadata(other, &actor, input.clone()).await.is_err());
    let mut invalid = input.clone(); invalid.default_template_key = "invalid layout".into();
    assert!(service.update_metadata(tenant, &actor, invalid).await.is_err());
    let mut builtin = input.clone(); builtin.key = "blank".into();
    assert!(service.update_metadata(tenant, &actor, builtin).await.is_err());
    let new_revision = service.update_metadata(tenant, &actor, input.clone()).await?;
    assert_ne!(new_revision, revision);
    assert!(service.update_metadata(tenant, &actor, input.clone()).await.is_err());
    assert!(service.delete(tenant, &actor, &input.key, revision).await.is_err());
    let updated = service.get(tenant, &actor, &input.key).await?.unwrap();
    assert_eq!(updated.name, "Updated");
    assert_eq!(updated.category, "news");
    assert_eq!(updated.default_template_key, "new-layout");
    assert_eq!(updated.description.as_deref(), Some("New description"));
    assert_eq!(updated.thumbnail_url, created.thumbnail_url);
    assert_eq!(updated.content, created.content);
    assert_eq!(updated.revision, Some(new_revision));
    let mut left = input.clone(); left.expected_revision = new_revision; left.name = "Left".into();
    let mut right = left.clone(); right.name = "Right".into();
    let (a, b) = tokio::join!(service.update_metadata(tenant, &actor, left), service.update_metadata(tenant, &actor, right));
    assert_ne!(a.is_ok(), b.is_ok());
    Ok(())
}

#[tokio::test]
async fn checked_layout_creation_requires_exact_catalog_entry_and_preserves_legacy_labels() -> TestResult<()> {
    use rustok_api::Permission;
    use rustok_core::UserRole;
    use rustok_pages::PageTemplateService;

    let tenant = Uuid::new_v4();
    let other = Uuid::new_v4();
    let db = setup_db(tenant).await?;
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{other}')")).await?;
    let pages = PageService::new(db.clone(), TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))));
    let catalog = PageTemplateService::new(db.clone());
    let actor = SecurityContext::system();
    let creator = SecurityContext::from_permissions(UserRole::Manager, Some(Uuid::new_v4()), [Permission::PAGES_CREATE]);
    let input = |key: &str, locale: &str, slug: &str| CreatePageInput {
        translations: vec![PageTranslationInput { locale: locale.into(), title: slug.into(), slug: Some(slug.into()), meta_title: None, meta_description: None }],
        template: Some(key.into()), body: None, channel_slugs: None, publish: false,
    };
    catalog.save(tenant, &actor, "site", "en", vec![], vec![], None).await?;
    assert!(pages.create_with_catalog_layout(other, actor.clone(), input("site", "en", "other")).await.is_err());
    assert!(pages.create_with_catalog_layout(tenant, actor.clone(), input("site", "ru", "wrong-locale")).await.is_err());
    assert!(pages.create_with_catalog_layout(tenant, creator, input("site", "en", "forbidden")).await.is_err());
    assert!(pages.create_with_catalog_layout(tenant, actor.clone(), input("missing", "en", "missing")).await.is_err());
    let mut multiple = input("site", "en", "multiple");
    multiple.translations.push(PageTranslationInput { locale: "ru".into(), title: "Other".into(), slug: Some("other".into()), meta_title: None, meta_description: None });
    assert!(pages.create_with_catalog_layout(tenant, actor.clone(), multiple).await.is_err());
    pages.create_with_catalog_layout(tenant, actor.clone(), input("site", "en", "selected")).await?;
    assert!(catalog.delete(tenant, &actor, "site", "en", 1).await.is_err());
    pages.create_with_catalog_layout(tenant, actor.clone(), input("default", "en", "default-page")).await?;
    pages.create(tenant, actor.clone(), input("legacy-label", "en", "legacy")).await?;
    catalog.save(tenant, &actor, "deleted", "en", vec![], vec![], None).await?;
    catalog.delete(tenant, &actor, "deleted", "en", 1).await?;
    assert!(pages.create_with_catalog_layout(tenant, actor, input("deleted", "en", "after-delete")).await.is_err());
    Ok(())
}

#[tokio::test]
async fn checked_layout_create_and_same_locale_delete_do_not_both_commit() -> TestResult<()> {
    use rustok_pages::entities::{page, page_template};
    use rustok_pages::PageTemplateService;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};

    let tenant = Uuid::new_v4();
    let db = setup_db_with_connections(tenant, 2).await?;
    let actor = SecurityContext::system();
    let pages = PageService::new(db.clone(), TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone()))));
    let catalog = PageTemplateService::new(db.clone());
    for index in 0..10 {
        let key = format!("race-{index}");
        let slug = format!("page-{index}");
        catalog.save(tenant, &actor, &key, "en", vec![], vec![], None).await?;
        let input = CreatePageInput {
            translations: vec![PageTranslationInput { locale: "en".into(), title: slug.clone(), slug: Some(slug), meta_title: None, meta_description: None }],
            template: Some(key.clone()), body: None, channel_slugs: None, publish: false,
        };
        // Either writer may fail on SQLite contention. Check the committed state,
        // not just the return value (a post-commit response read could also fail).
        let (created, deleted) = tokio::join!(
            pages.create_with_catalog_layout(tenant, actor.clone(), input),
            catalog.delete(tenant, &actor, &key, "en", 1),
        );
        assert!(!(created.is_ok() && deleted.is_ok()));
        let page_exists = page::Entity::find()
            .filter(page::Column::TenantId.eq(tenant))
            .filter(page::Column::Template.eq(&key)).one(&db).await?.is_some();
        let template_exists = page_template::Entity::find_by_id((tenant, "en".to_string(), key)).one(&db).await?.is_some();
        assert!(!page_exists || template_exists);
    }
    Ok(())
}
