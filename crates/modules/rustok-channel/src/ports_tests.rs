use super::*;
use std::sync::Mutex;

struct SelectedChannelProvider {
    projection: Option<ChannelReadProjection>,
    requests: Mutex<Vec<ChannelReadRequest>>,
    failure: bool,
}

#[async_trait]
impl ChannelReadPort for SelectedChannelProvider {
    async fn read_channel(
        &self,
        _context: PortContext,
        request: ChannelReadRequest,
    ) -> Result<Option<ChannelReadProjection>, PortError> {
        self.requests.lock().unwrap().push(request);
        if self.failure {
            Err(PortError::unavailable(
                "channel.test_unavailable",
                "Selected owner is unavailable",
            ))
        } else {
            Ok(self.projection.clone())
        }
    }

    async fn list_channels_for_tenant(
        &self,
        _context: PortContext,
        _request: ChannelListRequest,
    ) -> Result<ChannelListProjectionPage, PortError> {
        Err(PortError::unavailable(
            "channel.test_no_list",
            "Pair resolution must not list or choose a default",
        ))
    }
}

fn fixture() -> (PortContext, SelectedChannelProvider, Uuid) {
    let tenant_id = Uuid::from_u128(1);
    let channel_id = Uuid::from_u128(2);
    let now = chrono::DateTime::parse_from_rfc3339("2026-10-30T12:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let projection = ChannelReadProjection {
        detail: crate::ChannelDetailResponse {
            channel: crate::ChannelResponse {
                id: channel_id,
                tenant_id,
                slug: "web".to_string(),
                name: "Web".to_string(),
                is_active: true,
                is_default: false,
                status: "active".to_string(),
                settings: serde_json::json!({}),
                created_at: now,
                updated_at: now,
            },
            targets: Vec::new(),
            module_bindings: Vec::new(),
            oauth_apps: Vec::new(),
        },
    };
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "selected-channel-test",
    )
    .with_deadline(std::time::Duration::from_secs(2));
    (
        context,
        SelectedChannelProvider {
            projection: Some(projection),
            requests: Mutex::new(Vec::new()),
            failure: false,
        },
        channel_id,
    )
}

#[tokio::test]
async fn explicit_pair_completes_one_selector_without_context_or_default_borrowing() {
    let (context, provider, id) = fixture();
    let selected = provider
        .read_selected_channel(context.clone().with_channel("other-page"), Some(id), None)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.detail.channel.id, id);
    assert_eq!(selected.detail.channel.slug, "web");
    let selected = provider
        .read_selected_channel(context, None, Some(" WEB ".to_string()))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(selected.detail.channel.id, id);
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].selector, ChannelReadSelector::Id(id));
    assert_eq!(
        requests[1].selector,
        ChannelReadSelector::Slug("web".to_string())
    );
    assert!(requests.iter().all(|request| !request.include_inactive));
}

#[tokio::test]
async fn no_selectors_is_global_even_when_a_page_channel_exists() {
    let (context, provider, _) = fixture();
    assert!(
        provider
            .read_selected_channel(context.with_channel("page"), None, None)
            .await
            .unwrap()
            .is_none()
    );
    assert!(provider.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn contradictory_pair_and_wrong_provider_identity_refuse() {
    let (context, provider, id) = fixture();
    let error = provider
        .read_selected_channel(context.clone(), Some(id), Some("mobile".to_string()))
        .await
        .unwrap_err();
    assert_eq!(error.code, "channel.selector_mismatch");
    let error = provider
        .read_selected_channel(context, Some(Uuid::from_u128(99)), None)
        .await
        .unwrap_err();
    assert_eq!(error.code, "channel.selector_mismatch");
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn foreign_or_inactive_projection_never_becomes_a_global_scope() {
    let (context, mut provider, id) = fixture();
    provider
        .projection
        .as_mut()
        .unwrap()
        .detail
        .channel
        .tenant_id = Uuid::from_u128(99);
    assert_eq!(
        provider
            .read_selected_channel(context.clone(), Some(id), None)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::NotFound
    );
    provider
        .projection
        .as_mut()
        .unwrap()
        .detail
        .channel
        .tenant_id = Uuid::from_u128(1);
    provider
        .projection
        .as_mut()
        .unwrap()
        .detail
        .channel
        .is_active = false;
    assert_eq!(
        provider
            .read_selected_channel(context, Some(id), None)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::InvariantViolation
    );
}

#[tokio::test]
async fn missing_or_failed_selected_owner_never_tries_another_provider_or_default() {
    let (context, mut provider, id) = fixture();
    provider.failure = true;
    let error = provider
        .read_selected_channel(context.clone(), Some(id), None)
        .await
        .unwrap_err();
    assert_eq!(error.code, "channel.test_unavailable");
    assert!(error.retryable);
    assert_eq!(provider.requests.lock().unwrap().len(), 1);
    provider.failure = false;
    provider.projection = None;
    assert_eq!(
        provider
            .read_selected_channel(context, Some(id), None)
            .await
            .unwrap_err()
            .kind,
        PortErrorKind::NotFound
    );
    assert_eq!(provider.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn present_blank_and_nil_selectors_refuse_before_lookup() {
    let (context, provider, id) = fixture();
    assert!(
        provider
            .read_selected_channel(context.clone(), Some(id), Some(" ".to_string()))
            .await
            .is_err()
    );
    assert!(
        provider
            .read_selected_channel(context.clone(), Some(Uuid::nil()), None)
            .await
            .is_err()
    );
    let mut nil_tenant = context;
    nil_tenant.tenant_id = Uuid::nil().to_string();
    assert!(
        provider
            .read_selected_channel(nil_tenant, None, None)
            .await
            .is_err()
    );
    assert!(provider.requests.lock().unwrap().is_empty());
}

async fn setup_db_for_port_tests() -> sea_orm::DatabaseConnection {
    use sea_orm::{ConnectionTrait, Statement};
    use sea_orm_migration::SchemaManager;

    let db = rustok_test_utils::setup_test_db().await;
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE tenants (
            id TEXT PRIMARY KEY NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL UNIQUE,
            domain TEXT NULL UNIQUE,
            settings TEXT NOT NULL DEFAULT '{}',
            default_locale TEXT NOT NULL DEFAULT 'en',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .unwrap();
    db.execute_raw(Statement::from_string(
        db.get_database_backend(),
        r#"
        CREATE TABLE oauth_apps (
            id TEXT PRIMARY KEY NOT NULL,
            tenant_id TEXT NOT NULL,
            name TEXT NOT NULL,
            slug TEXT NOT NULL,
            app_type TEXT NOT NULL DEFAULT 'machine',
            is_active BOOLEAN NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    ))
    .await
    .unwrap();
    let manager = SchemaManager::new(&db);
    for migration in crate::migrations::migrations() {
        migration.up(&manager).await.unwrap();
    }
    db
}

#[tokio::test]
async fn channel_service_read_port_reads_channel_by_all_selectors() {
    use crate::dto::{CreateChannelInput, CreateChannelTargetInput};
    use sea_orm::{ConnectionTrait, Statement};

    let db = setup_db_for_port_tests().await;
    let tenant_id = Uuid::new_v4();
    let other_tenant_id = Uuid::new_v4();

    for (tid, slug) in [(tenant_id, "primary"), (other_tenant_id, "other")] {
        db.execute_raw(Statement::from_sql_and_values(
            db.get_database_backend(),
            "INSERT INTO tenants (id, name, slug, settings, default_locale, is_active, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            [
                tid.into(),
                format!("{slug} tenant").into(),
                slug.to_string().into(),
                "{}".to_string().into(),
                "en".to_string().into(),
                true.into(),
            ],
        ))
        .await
        .unwrap();
    }

    let service = crate::ChannelService::new(db.clone());
    let default_channel = service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "main".to_string(),
            name: "Main Channel".to_string(),
            settings: None,
        })
        .await
        .unwrap();

    let web_channel = service
        .create_channel(CreateChannelInput {
            tenant_id,
            slug: "storefront".to_string(),
            name: "Storefront Channel".to_string(),
            settings: None,
        })
        .await
        .unwrap();

    let foreign_channel = service
        .create_channel(CreateChannelInput {
            tenant_id: other_tenant_id,
            slug: "foreign".to_string(),
            name: "Foreign Channel".to_string(),
            settings: None,
        })
        .await
        .unwrap();

    service
        .add_target(
            web_channel.id,
            CreateChannelTargetInput {
                target_type: "web_domain".to_string(),
                value: "store.example.test".to_string(),
                is_primary: true,
                settings: None,
            },
        )
        .await
        .unwrap();

    let port = in_process_channel_read_port(db);
    let context = PortContext::new(
        tenant_id.to_string(),
        PortActor::system(),
        "en",
        "port-read-test",
    )
    .with_deadline(std::time::Duration::from_secs(5));

    // Read by Default
    let read_default = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::Default,
                include_inactive: false,
            },
        )
        .await
        .unwrap()
        .expect("default channel should be found");
    assert_eq!(read_default.detail.channel.id, default_channel.id);

    // Read by Id
    let read_by_id = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::Id(web_channel.id),
                include_inactive: false,
            },
        )
        .await
        .unwrap()
        .expect("channel by id should be found");
    assert_eq!(read_by_id.detail.channel.id, web_channel.id);

    // Read by Slug
    let read_by_slug = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::Slug("storefront".to_string()),
                include_inactive: false,
            },
        )
        .await
        .unwrap()
        .expect("channel by slug should be found");
    assert_eq!(read_by_slug.detail.channel.id, web_channel.id);

    // Read by HostTargetValue
    let read_by_host = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::HostTargetValue("store.example.test".to_string()),
                include_inactive: false,
            },
        )
        .await
        .unwrap()
        .expect("channel by host should be found");
    assert_eq!(read_by_host.detail.channel.id, web_channel.id);

    // Non-existent ID returns Ok(None)
    let non_existent_id = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::Id(Uuid::new_v4()),
                include_inactive: false,
            },
        )
        .await
        .unwrap();
    assert!(non_existent_id.is_none());

    // Foreign tenant ID returns Ok(None)
    let foreign_read = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::Id(foreign_channel.id),
                include_inactive: false,
            },
        )
        .await
        .unwrap();
    assert!(foreign_read.is_none());

    // Nil ID returns Validation error
    let nil_id_err = port
        .read_channel(
            context.clone(),
            ChannelReadRequest {
                selector: ChannelReadSelector::Id(Uuid::nil()),
                include_inactive: false,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(nil_id_err.kind, PortErrorKind::Validation);

    // Pagination listing
    let list_page = port
        .list_channels_for_tenant(
            context.clone(),
            ChannelListRequest {
                page: 1,
                per_page: 10,
                include_inactive: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(list_page.total, 2);
    assert_eq!(list_page.items.len(), 2);

    // Zero pagination returns Validation error
    let zero_page_err = port
        .list_channels_for_tenant(
            context,
            ChannelListRequest {
                page: 0,
                per_page: 10,
                include_inactive: true,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(zero_page_err.kind, PortErrorKind::Validation);
}
