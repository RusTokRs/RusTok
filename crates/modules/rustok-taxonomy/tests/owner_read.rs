use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, ModuleTermCreateInput, TaxonomyModule, TaxonomyOwnerReader,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_test_utils::db::setup_test_db;
use sea_orm::TransactionTrait;
use sea_orm_migration::prelude::SchemaManager;
use uuid::Uuid;

async fn setup() -> (sea_orm::DatabaseConnection, TaxonomyService) {
    let db = setup_test_db().await;
    let schema_manager = SchemaManager::new(&db);
    for migration in TaxonomyModule.migrations() {
        migration
            .up(&schema_manager)
            .await
            .expect("failed to run taxonomy migration");
    }
    let service = TaxonomyService::new(db.clone());
    (db, service)
}

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

async fn create_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    scope_type: TaxonomyScopeType,
    scope_value: Option<&str>,
    name: &str,
    slug: &str,
) -> Uuid {
    if scope_type == TaxonomyScopeType::Module {
        let txn = service
            .database()
            .begin()
            .await
            .expect("transaction should start");
        let term_id = service
            .create_module_term_in_tx(
                &txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                scope_value.expect("module scope value required"),
                ModuleTermCreateInput {
                    locale: "en".to_owned(),
                    name: name.to_owned(),
                    slug: Some(slug.to_owned()),
                    canonical_key: Some(slug.to_owned()),
                },
            )
            .await
            .expect("module term should be created");
        txn.commit().await.expect("transaction should commit");
        term_id
    } else {
        service
            .create_term(
                tenant_id,
                admin(),
                CreateTaxonomyTermInput {
                    kind: TaxonomyTermKind::Tag,
                    scope_type,
                    scope_value: scope_value.map(ToOwned::to_owned),
                    locale: "en".to_owned(),
                    name: name.to_owned(),
                    slug: Some(slug.to_owned()),
                    canonical_key: Some(slug.to_owned()),
                    description: None,
                    aliases: vec![],
                },
            )
            .await
            .expect("taxonomy term should be created")
    }
}

#[tokio::test]
async fn owner_reader_enforces_scope_ids_and_locale_fallback() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let module_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Rust",
        "rust",
    )
    .await;
    let global_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "Systems",
        "systems",
    )
    .await;
    let reader = TaxonomyOwnerReader::new(db);

    let module_terms = reader
        .load_scoped_terms(
            tenant_id,
            TaxonomyTermKind::Tag,
            TaxonomyScopeType::Module,
            Some("blog"),
            Some(&[module_term_id, global_term_id]),
            "fr",
            Some("en"),
        )
        .await
        .expect("module-scoped owner read should succeed");

    assert_eq!(module_terms.len(), 1);
    assert_eq!(module_terms[0].id, module_term_id);
    assert_eq!(module_terms[0].requested_locale, "fr");
    assert_eq!(module_terms[0].effective_locale, "en");
    assert_eq!(module_terms[0].name, "Rust");
    assert_eq!(module_terms[0].slug, "rust");
    assert_eq!(module_terms[0].scope_value.as_deref(), Some("blog"));

    let global_terms = reader
        .load_scoped_terms(
            tenant_id,
            TaxonomyTermKind::Tag,
            TaxonomyScopeType::Global,
            None,
            Some(&[module_term_id, global_term_id]),
            "en",
            None,
        )
        .await
        .expect("global owner read should succeed");

    assert_eq!(global_terms.len(), 1);
    assert_eq!(global_terms[0].id, global_term_id);
    assert_eq!(global_terms[0].scope_value, None);
}

#[tokio::test]
async fn owner_reader_treats_empty_identity_filter_as_empty_result() {
    let (db, _service) = setup().await;
    let reader = TaxonomyOwnerReader::new(db);

    let terms = reader
        .load_scoped_terms(
            Uuid::new_v4(),
            TaxonomyTermKind::Tag,
            TaxonomyScopeType::Module,
            Some("blog"),
            Some(&[]),
            "en",
            None,
        )
        .await
        .expect("empty identity page should be accepted");

    assert!(terms.is_empty());
}

#[tokio::test]
async fn transaction_owner_reader_preserves_mixed_scope_and_rejects_foreign_tenant_ids() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let foreign_tenant_id = Uuid::new_v4();
    let module_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Rust",
        "rust",
    )
    .await;
    let global_term_id = create_term(
        &service,
        tenant_id,
        TaxonomyScopeType::Global,
        None,
        "Systems",
        "systems",
    )
    .await;
    let foreign_term_id = create_term(
        &service,
        foreign_tenant_id,
        TaxonomyScopeType::Module,
        Some("blog"),
        "Foreign",
        "foreign",
    )
    .await;

    let txn = db
        .begin()
        .await
        .expect("owner read transaction should start");
    let terms = TaxonomyOwnerReader::load_terms_by_ids_in_tx(
        &txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        &[foreign_term_id, global_term_id, module_term_id],
        "fr",
        Some("en"),
    )
    .await
    .expect("transaction owner read should succeed");
    txn.rollback()
        .await
        .expect("owner read transaction should roll back");

    assert_eq!(terms.len(), 2);
    assert!(terms.iter().any(|term| term.id == module_term_id));
    assert!(terms.iter().any(|term| term.id == global_term_id));
    assert!(!terms.iter().any(|term| term.id == foreign_term_id));
    assert!(
        terms
            .iter()
            .all(|term| term.requested_locale == "fr" && term.effective_locale == "en")
    );
}

#[tokio::test]
async fn owner_reads_resolve_uncommitted_terms_on_the_caller_transaction() {
    let (db, service) = setup().await;
    let tenant_id = Uuid::new_v4();
    let txn = db.begin().await.unwrap();
    let ids = service
        .ensure_terms_for_module_in_tx(
            &txn,
            tenant_id,
            TaxonomyTermKind::Tag,
            "product",
            "en",
            &["Uncommitted Tag".to_string()],
        )
        .await
        .unwrap();
    let names = service
        .resolve_term_names_in(&txn, tenant_id, &ids, "en", None)
        .await
        .unwrap();
    assert_eq!(
        names.get(&ids[0]).map(String::as_str),
        Some("Uncommitted Tag")
    );
    let foreign = service
        .resolve_term_names_in(&txn, Uuid::new_v4(), &ids, "en", None)
        .await
        .unwrap();
    assert!(foreign.is_empty());

    let scoped = TaxonomyOwnerReader::load_scoped_terms_in(
        &txn,
        tenant_id,
        TaxonomyTermKind::Tag,
        TaxonomyScopeType::Module,
        Some("product"),
        Some(&ids),
        "fr",
        Some("en"),
    )
    .await
    .unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].id, ids[0]);
    assert_eq!(scoped[0].requested_locale, "fr");
    assert_eq!(scoped[0].effective_locale, "en");

    for (scope, expected) in [("product", Some(ids[0])), ("blog", None)] {
        assert_eq!(
            TaxonomyService::resolve_term_id_for_module_in(
                &txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                scope,
                "fr",
                Some("en"),
                "uncommitted-tag",
            )
            .await
            .unwrap(),
            expected,
        );
    }
    txn.rollback().await.unwrap();
    let after = service
        .resolve_term_names(tenant_id, &ids, "en", None)
        .await
        .unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn published_term_names_port_enforces_tenant_capability_and_bounds() {
    use rustok_api::{PortActor, PortContext, PortErrorKind};
    use rustok_taxonomy_ports::{
        MAX_TAXONOMY_TERM_NAMES_PER_CALL, ReadTaxonomyTermNamesRequest,
        TAXONOMY_TERM_NAMES_READ_CLAIM, TaxonomyTermNamesReadPort,
    };

    let (_, service) = setup().await;
    let tenant = Uuid::new_v4();
    let term = create_term(
        &service,
        tenant,
        TaxonomyScopeType::Module,
        Some("product"),
        "Shoes",
        "shoes",
    )
    .await;
    let other = create_term(
        &service,
        Uuid::new_v4(),
        TaxonomyScopeType::Global,
        None,
        "Hidden",
        "hidden",
    )
    .await;
    let request = ReadTaxonomyTermNamesRequest {
        term_ids: vec![term, other, Uuid::new_v4()],
        fallback_locale: Some("en".into()),
    };
    let context = PortContext::new(
        tenant.to_string(),
        PortActor::service("product"),
        "de",
        "term-names-test",
    );
    let error = service
        .read_term_names(context.clone(), request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "port.deadline_required");
    let context = context.with_deadline(std::time::Duration::from_secs(2));
    let error = service
        .read_term_names(context.clone(), request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    let context = context.with_claim(TAXONOMY_TERM_NAMES_READ_CLAIM);
    let names = service
        .read_term_names(context.clone(), request.clone())
        .await
        .unwrap();
    assert_eq!(names, [(term, String::from("Shoes"))].into_iter().collect());

    let mut user_context = context.clone();
    user_context.actor = PortActor::user(Uuid::new_v4().to_string());
    let error = service
        .read_term_names(user_context, request)
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);

    let oversized = ReadTaxonomyTermNamesRequest {
        term_ids: vec![term; MAX_TAXONOMY_TERM_NAMES_PER_CALL + 1],
        fallback_locale: None,
    };
    let error = service
        .read_term_names(context, oversized)
        .await
        .unwrap_err();
    assert_eq!(error.code, "taxonomy.term_name_ids_invalid");
}
