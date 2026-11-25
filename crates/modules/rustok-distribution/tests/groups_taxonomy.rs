#![cfg(all(feature = "mod-groups", feature = "mod-taxonomy"))]

use std::time::Duration;

use rustok_api::{HostRuntimeContext, PortActor, PortContext, PortErrorKind};
use rustok_core::{MigrationSource, ModuleRegistry};
use rustok_distribution::{build_registry, build_runtime_extensions};
use rustok_groups::{
    CreateGroupInput, GroupCommandPort, GroupJoinPolicy, GroupSummaryReadPort, GroupVisibility,
    GroupsModule, GroupsService, ListGroupsRequest,
};
use rustok_outbox::OutboxModule;
use rustok_taxonomy::{SyncModuleCategoryInput, TaxonomyModule, sync_module_category_in_tx};
use sea_orm::{ConnectionTrait, Database, TransactionTrait};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

#[tokio::test]
async fn selected_groups_uses_taxonomy_from_the_host_runtime() {
    let db = Database::connect("sqlite::memory:").await.unwrap();
    let schema = SchemaManager::new(&db);
    for migration in OutboxModule
        .migrations()
        .into_iter()
        .chain(GroupsModule.migrations())
        .chain(TaxonomyModule.migrations())
    {
        migration.up(&schema).await.unwrap();
    }

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        "groups-taxonomy-test",
    )
    .with_deadline(Duration::from_secs(30));
    let input = CreateGroupInput {
        handle: "neighborhood".into(),
        locale: "en".into(),
        title: "Neighborhood".into(),
        summary: None,
        body: None,
        visibility: GroupVisibility::Public,
        join_policy: GroupJoinPolicy::Open,
        category_id: Some(Uuid::new_v4()),
        avatar_media_id: None,
        cover_media_id: None,
        metadata: serde_json::json!({}),
    };

    let groups_only = ModuleRegistry::new().register(GroupsModule);
    let host = HostRuntimeContext::new(db.clone());
    let without_taxonomy = build_runtime_extensions(&groups_only)
        .unwrap()
        .apply_to_host_runtime(host.clone());
    let error = GroupsService::from_host_runtime(&without_taxonomy)
        .create_group(context.clone().with_idempotency_key("group"), input.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    assert_eq!(error.code, "groups.taxonomy_unavailable");

    let registry = build_registry();
    assert!(registry.contains("groups") && registry.contains("taxonomy"));
    let host = build_runtime_extensions(&registry)
        .unwrap()
        .apply_to_host_runtime(host);
    let groups = GroupsService::from_host_runtime(&host);
    let error = groups
        .create_group(context.clone().with_idempotency_key("group"), input.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Validation);

    let category_id = input.category_id.unwrap();
    let transaction = db.begin().await.unwrap();
    sync_module_category_in_tx(
        &transaction,
        tenant_id,
        SyncModuleCategoryInput {
            category_id,
            module_scope: "groups".into(),
            canonical_key: "local-groups".into(),
            locale: "en".into(),
            name: "Local groups".into(),
            slug: "local-groups".into(),
            aliases: vec![],
            description: None,
            parent_id: None,
            position: 0,
            icon_key: None,
            color: None,
        },
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    let foreign_context = PortContext::new(
        Uuid::new_v4().to_string(),
        PortActor::user(actor_id.to_string()),
        "en",
        "groups-taxonomy-foreign-tenant",
    )
    .with_deadline(Duration::from_secs(30))
    .with_idempotency_key("group");
    let error = groups
        .create_group(foreign_context, input.clone())
        .await
        .unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Validation);

    let command = context.clone().with_idempotency_key("group");
    let created = groups
        .create_group(command.clone(), input.clone())
        .await
        .unwrap();
    let category = created.summary.category.as_ref().unwrap();
    assert_eq!(category.id, category_id);
    assert_eq!(category.name, "Local groups");
    let directory = ListGroupsRequest {
        page: 1,
        per_page: 10,
        search: None,
        include_non_public: false,
        category_id: Some(category_id),
    };
    let listed = groups
        .list_groups(context.clone(), directory.clone())
        .await
        .unwrap();
    assert_eq!(listed.items[0].category.as_ref(), Some(category));

    db.execute_unprepared("DROP TABLE taxonomy_term_translations")
        .await
        .unwrap();
    let error = groups.list_groups(context, directory).await.unwrap_err();
    assert_eq!(error.kind, PortErrorKind::Unavailable);
    let replay = groups.create_group(command, input).await.unwrap();
    assert_eq!(replay, created);
}
