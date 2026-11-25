use std::sync::Arc;

use rustok_core::{MigrationSource, SecurityContext, UserRole};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, ModuleTermCreateInput, TaxonomyModule, TaxonomyOwnerReader,
    TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
};
use rustok_taxonomy_ports::TaxonomyTermNamesReadPort;
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
    assert_product_reads_taxonomy_names_from_another_database(
        Arc::new(TaxonomyService::new(service.database().clone())),
        tenant,
        term,
    )
    .await;

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

async fn assert_product_reads_taxonomy_names_from_another_database(
    taxonomy: Arc<dyn TaxonomyTermNamesReadPort>,
    tenant_id: Uuid,
    term_id: Uuid,
) {
    use rustok_api::{PortActor, PortContext};
    use rustok_outbox_storage::TransactionalEventBus;
    use rustok_product::{
        CatalogService, ProductStorefrontTagHydrationRequest, ProductStorefrontTagReadPort,
        entities::{product, product_tag},
    };
    use rustok_test_utils::MockEventTransport;
    use sea_orm::{ActiveModelTrait, Schema, Set};

    let db = setup_test_db().await;
    let manager = SchemaManager::new(&db);
    let schema = Schema::new(db.get_database_backend());
    manager
        .create_table(schema.create_table_from_entity(product::Entity))
        .await
        .unwrap();
    manager
        .create_table(schema.create_table_from_entity(product_tag::Entity))
        .await
        .unwrap();
    let product_id = Uuid::new_v4();
    let now = chrono::Utc::now();
    product::ActiveModel {
        id: Set(product_id),
        tenant_id: Set(tenant_id),
        status: Set(product::ProductStatus::Active),
        metadata: Set(serde_json::json!({})),
        created_at: Set(now.into()),
        updated_at: Set(now.into()),
        revision: Set(1),
        ..Default::default()
    }
    .insert(&db)
    .await
    .unwrap();
    product_tag::ActiveModel {
        product_id: Set(product_id),
        term_id: Set(term_id),
        tenant_id: Set(tenant_id),
        created_at: Set(now.into()),
    }
    .insert(&db)
    .await
    .unwrap();

    let catalog = CatalogService::new(
        db,
        TransactionalEventBus::new(Arc::new(MockEventTransport::new())),
    )
    .with_taxonomy_term_names_port(taxonomy);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::service("index"),
        "de",
        "tag-projection-test",
    )
    .with_deadline(std::time::Duration::from_secs(2));
    let names = catalog
        .hydrate_storefront_product_tags(
            context,
            ProductStorefrontTagHydrationRequest {
                product_ids: vec![product_id],
                fallback_locale: "en".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(names.items.len(), 1);
    assert_eq!(names.items[0].product_id, product_id);
    assert_eq!(names.items[0].tags, ["Shoes"]);
}

#[cfg(feature = "service")]
#[tokio::test]
async fn grpc_term_names_serves_product_and_enforces_credential_scope() {
    use std::time::Duration;

    use rustok_api::{PortActor, PortContext, PortErrorKind};
    use rustok_taxonomy_ports::{
        ReadTaxonomyTermNamesRequest, TAXONOMY_TERM_NAMES_READ_CLAIM,
        grpc::{GrpcTaxonomyTermNamesProvider, TaxonomyTermNamesGrpcService},
    };
    use tokio::{net::TcpListener, sync::oneshot};
    use tokio_stream::wrappers::TcpListenerStream;
    use tonic::transport::Server;

    let (_, owner) = setup().await;
    let tenant = Uuid::new_v4();
    let foreign_tenant = Uuid::new_v4();
    let term = create_term(
        &owner,
        tenant,
        TaxonomyScopeType::Module,
        Some("product"),
        "Shoes",
        "shoes",
    )
    .await;
    let foreign_term = create_term(
        &owner,
        foreign_tenant,
        TaxonomyScopeType::Global,
        None,
        "Hidden",
        "hidden",
    )
    .await;
    let token = "test-taxonomy-term-names-token";
    let service = TaxonomyTermNamesGrpcService::new(Arc::new(owner), token, [tenant])
        .unwrap()
        .into_server();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let (stop, shutdown) = oneshot::channel();
    let server = tokio::spawn(
        Server::builder()
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                shutdown.await.expect("test server should shut down");
            }),
    );
    assert!(GrpcTaxonomyTermNamesProvider::new(&endpoint, token, false, None).is_err());
    assert!(
        GrpcTaxonomyTermNamesProvider::new("http://192.0.2.1:7446", token, true, None).is_err()
    );

    let client = GrpcTaxonomyTermNamesProvider::new(&endpoint, token, true, None).unwrap();
    let context = PortContext::new(
        tenant.to_string(),
        PortActor::service("product"),
        "de",
        "taxonomy-grpc-test",
    )
    .with_claim(TAXONOMY_TERM_NAMES_READ_CLAIM)
    .with_traceparent("00-12345678901234567890123456789012-1234567890123456-01")
    .with_deadline(Duration::from_secs(2));
    let request = ReadTaxonomyTermNamesRequest {
        term_ids: vec![term, foreign_term, Uuid::new_v4()],
        fallback_locale: Some("en".into()),
    };
    let names = client
        .read_term_names(context.clone(), request.clone())
        .await
        .unwrap();
    assert_eq!(names, [(term, "Shoes".into())].into_iter().collect());
    let mut foreign = context.clone();
    foreign.tenant_id = foreign_tenant.to_string();
    let error = client
        .read_term_names(foreign, request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    let invalid_token =
        GrpcTaxonomyTermNamesProvider::new(&endpoint, "different-token", true, None).unwrap();
    let error = invalid_token
        .read_term_names(context.clone(), request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Forbidden);
    let mut missing_deadline = context.clone();
    missing_deadline.deadline_ms = None;
    let error = client
        .read_term_names(missing_deadline, request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "port.deadline_required");
    let invalid_locale = PortContext {
        locale: "-".into(),
        ..context.clone()
    };
    let error = client
        .read_term_names(invalid_locale, request.clone())
        .await
        .unwrap_err();
    assert_eq!(error.code, "taxonomy.term_names_context_invalid");

    assert_product_reads_taxonomy_names_from_another_database(
        Arc::new(client.clone()),
        tenant,
        term,
    )
    .await;
    stop.send(()).unwrap();
    server.await.unwrap().unwrap();
    let error = client.read_term_names(context, request).await.unwrap_err();
    assert!(matches!(
        error.kind,
        PortErrorKind::Unavailable | PortErrorKind::Timeout
    ));
    assert!(error.retryable);
}
