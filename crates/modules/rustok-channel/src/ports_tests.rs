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
