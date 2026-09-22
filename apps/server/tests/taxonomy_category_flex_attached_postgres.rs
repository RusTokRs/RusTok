#![cfg(all(feature = "mod-taxonomy", feature = "mod-flex"))]

use std::collections::HashMap;

use flex::graphql::AttachedValuesGraphqlPort;
use flex::{
    AttachedEntityRef, CreateFieldDefinitionCommand, FieldDefinitionService, FlexModule,
    GenericAttachedFieldDefinitionService, TAXONOMY_CATEGORY_ENTITY_TYPE, load_exact_locale_values,
    load_generic_attached_shared_values,
};
use rustok_core::{
    MigrationSource, SecurityContext, UserRole,
    field_schema::{FieldType, FlexError},
};
use rustok_server::services::flex_attached_values::{
    FlexAttachedValuesGraphqlAdapter, FlexTaxonomyCategoryDeleteCleanup,
};
use rustok_taxonomy::{
    CreateTaxonomyTermInput, TaxonomyModule, TaxonomyScopeType, TaxonomyService, TaxonomyTermKind,
    taxonomy_term_identity_exists,
};
use sea_orm::{ConnectionTrait, Database};
use sea_orm_migration::prelude::SchemaManager;
use serde_json::json;
use uuid::Uuid;

fn admin() -> SecurityContext {
    SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()))
}

fn definition(field_key: &str, is_localized: bool) -> CreateFieldDefinitionCommand {
    CreateFieldDefinitionCommand {
        field_key: field_key.to_string(),
        field_type: FieldType::Text,
        label: HashMap::from([("en".to_string(), field_key.to_string())]),
        description: None,
        is_localized,
        is_required: false,
        default_value: None,
        validation: None,
        position: None,
    }
}

async fn create_term(
    service: &TaxonomyService,
    tenant_id: Uuid,
    kind: TaxonomyTermKind,
    name: &str,
) -> Uuid {
    service
        .create_term(
            tenant_id,
            admin(),
            CreateTaxonomyTermInput {
                kind,
                scope_type: TaxonomyScopeType::Global,
                scope_value: None,
                locale: "en".to_string(),
                name: name.to_string(),
                slug: None,
                canonical_key: Some(format!("{}-{}", name.to_ascii_lowercase(), Uuid::new_v4())),
                description: None,
                aliases: Vec::new(),
            },
        )
        .await
        .expect("taxonomy fixture should create")
}

#[tokio::test]
#[ignore = "requires RUSTOK_FLEX_TEST_POSTGRES_URL"]
async fn category_flex_transport_roundtrips_real_owner_and_hard_delete_cleans_values() {
    let url = std::env::var("RUSTOK_FLEX_TEST_POSTGRES_URL")
        .expect("RUSTOK_FLEX_TEST_POSTGRES_URL must be set");
    let db = Database::connect(url)
        .await
        .expect("PostgreSQL Category Flex fixture should connect");
    let manager = SchemaManager::new(&db);

    if !manager
        .has_table("flex_attached_field_definitions")
        .await
        .expect("Flex table inspection should succeed")
    {
        for migration in FlexModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("Flex migration should apply on PostgreSQL");
        }
    }
    if !manager
        .has_table("taxonomy_terms")
        .await
        .expect("Taxonomy table inspection should succeed")
    {
        for migration in TaxonomyModule.migrations() {
            migration
                .up(&manager)
                .await
                .expect("Taxonomy migration should apply on PostgreSQL");
        }
    }
    db.execute_unprepared(
        "CREATE TABLE IF NOT EXISTS flex_attached_localized_values (\
            id UUID PRIMARY KEY, \
            tenant_id UUID NOT NULL, \
            entity_type VARCHAR(64) NOT NULL, \
            entity_id UUID NOT NULL, \
            field_key VARCHAR(128) NOT NULL, \
            locale VARCHAR(32) NOT NULL, \
            value JSONB NOT NULL, \
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP, \
            UNIQUE (tenant_id, entity_type, entity_id, field_key, locale)\
        )",
    )
    .await
    .expect("localized attached storage should exist");

    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();
    let taxonomy = TaxonomyService::new(db.clone());
    let category_id = create_term(
        &taxonomy,
        tenant_id,
        TaxonomyTermKind::Category,
        "Flex Category",
    )
    .await;
    let tag_id = create_term(&taxonomy, tenant_id, TaxonomyTermKind::Tag, "Flex Tag").await;

    let definitions = GenericAttachedFieldDefinitionService::new(TAXONOMY_CATEGORY_ENTITY_TYPE);
    definitions
        .create(
            &db,
            tenant_id,
            Some(Uuid::new_v4()),
            definition("badge", false),
        )
        .await
        .expect("shared Category definition should create");
    definitions
        .create(
            &db,
            tenant_id,
            Some(Uuid::new_v4()),
            definition("tagline", true),
        )
        .await
        .expect("localized Category definition should create");

    let adapter = FlexAttachedValuesGraphqlAdapter::new(db.clone());
    let english = adapter
        .update_values(
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "en",
            Some(json!({"badge": "gold", "tagline": "Hello"})),
        )
        .await
        .expect("real Category values should author")
        .expect("English Category values should resolve");
    assert_eq!(english, json!({"badge": "gold", "tagline": "Hello"}));

    let arabic = adapter
        .update_values(
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "ar",
            Some(json!({"tagline": "مرحبا"})),
        )
        .await
        .expect("Arabic Category values should author")
        .expect("Arabic Category values should resolve");
    assert_eq!(arabic, json!({"badge": "gold", "tagline": "مرحبا"}));

    let fallback = adapter
        .resolve_values(
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "fr",
            "en",
        )
        .await
        .expect("Category fallback should resolve")
        .expect("fallback values should exist");
    assert_eq!(fallback, json!({"badge": "gold", "tagline": "Hello"}));

    assert!(matches!(
        adapter
            .update_values(
                tenant_id,
                TAXONOMY_CATEGORY_ENTITY_TYPE,
                tag_id,
                "en",
                Some(json!({"badge": "forbidden"})),
            )
            .await,
        Err(FlexError::NotFound(id)) if id == tag_id
    ));
    assert!(matches!(
        adapter
            .resolve_values(
                other_tenant_id,
                TAXONOMY_CATEGORY_ENTITY_TYPE,
                category_id,
                "en",
                "en",
            )
            .await,
        Err(FlexError::NotFound(id)) if id == category_id
    ));

    taxonomy
        .delete_category_with_cleanup(
            tenant_id,
            category_id,
            admin(),
            &FlexTaxonomyCategoryDeleteCleanup,
        )
        .await
        .expect("Category hard delete with Flex cleanup should commit atomically");

    assert!(
        !taxonomy_term_identity_exists(&db, tenant_id, TaxonomyTermKind::Category, category_id)
            .await
            .expect("deleted Category identity lookup should succeed")
    );
    let entity = AttachedEntityRef {
        tenant_id,
        entity_type: TAXONOMY_CATEGORY_ENTITY_TYPE,
        entity_id: category_id,
    };
    assert_eq!(
        load_generic_attached_shared_values(&db, entity)
            .await
            .expect("shared storage lookup should succeed"),
        json!({})
    );
    assert!(
        load_exact_locale_values(
            &db,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "en",
        )
        .await
        .expect("English localized lookup should succeed")
        .is_none()
    );
    assert!(
        load_exact_locale_values(
            &db,
            tenant_id,
            TAXONOMY_CATEGORY_ENTITY_TYPE,
            category_id,
            "ar",
        )
        .await
        .expect("Arabic localized lookup should succeed")
        .is_none()
    );
}
