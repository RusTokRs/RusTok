from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    source = target.read_text()
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one anchor, found {count}")
    target.write_text(source.replace(old, new, 1))


replace_once(
    "crates/rustok-notifications/storefront/src/transport.rs",
    '''fn current_storefront_transport_context() -> NotificationStorefrontTransportContext {
    let auth = use_context::<AuthContext>();
    let access_token = auth.as_ref().and_then(AuthContext::get_token);
    let tenant_slug = auth
        .as_ref()
        .and_then(AuthContext::get_tenant)
        .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
    NotificationStorefrontTransportContext::new(access_token, tenant_slug)
}''',
    '''pub fn current_notification_storefront_transport_context(
) -> NotificationStorefrontTransportContext {
    let auth = use_context::<AuthContext>();
    let access_token = auth.as_ref().and_then(AuthContext::get_token);
    let tenant_slug = auth
        .as_ref()
        .and_then(AuthContext::get_tenant)
        .or_else(|| option_env!("RUSTOK_TENANT_SLUG").map(str::to_string));
    NotificationStorefrontTransportContext::new(access_token, tenant_slug)
}

fn current_storefront_transport_context() -> NotificationStorefrontTransportContext {
    current_notification_storefront_transport_context()
}''',
)

replace_once(
    "crates/rustok-notifications/storefront/src/ui/leptos.rs",
    '''use crate::transport::{
    NativeNotificationStorefrontError, apply_notification_group_state,
    authorize_notification_open, load_notification_group_items,
    load_notification_group_summaries, load_notification_unread_count,
};''',
    '''use crate::transport::{
    NativeNotificationStorefrontError, NotificationStorefrontTransportContext,
    apply_notification_group_state, authorize_notification_open,
    current_notification_storefront_transport_context, load_notification_group_items,
    load_notification_group_summaries, load_notification_group_summaries_selected,
    load_notification_unread_count_selected,
};''',
)

replace_once(
    "crates/rustok-notifications/storefront/src/ui/leptos.rs",
    '''pub fn NotificationsView() -> impl IntoView {
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (refresh_feedback, set_refresh_feedback) = signal(Option::<String>::None);
    let bootstrap = Resource::new_blocking(
        move || refresh_nonce.get(),
        move |_| async move { load_inbox_snapshot().await },
    );
    let on_refresh = Callback::new(move |feedback: String| {
        set_refresh_feedback.set(Some(feedback));
        set_refresh_nonce.update(|value| *value = (*value).saturating_add(1));
    });''',
    '''pub fn NotificationsView() -> impl IntoView {
    let (refresh_nonce, set_refresh_nonce) = signal(0_u64);
    let (refresh_feedback, set_refresh_feedback) = signal(Option::<String>::None);
    let transport_context =
        Memo::new(move |_| current_notification_storefront_transport_context());
    Effect::new(move |_| {
        let _ = transport_context.get();
        set_refresh_feedback.set(None);
    });
    let bootstrap = Resource::new_blocking(
        move || (refresh_nonce.get(), transport_context.get()),
        move |(_, context)| async move { load_inbox_snapshot(context).await },
    );
    let on_refresh = Callback::new(move |feedback: String| {
        set_refresh_feedback.set(Some(feedback));
        set_refresh_nonce.update(|value| *value = (*value).saturating_add(1));
    });''',
)

replace_once(
    "crates/rustok-notifications/storefront/src/ui/leptos.rs",
    '''async fn load_inbox_snapshot(
) -> Result<NotificationStorefrontInboxSnapshot, NativeNotificationStorefrontError> {
    let unread = load_notification_unread_count().await?;
    let summaries = load_notification_group_summaries(NotificationStorefrontGroupSummaryRequest {
        cursor: None,
        limit: SUMMARY_PAGE_SIZE,
    })
    .await?;
    Ok(NotificationStorefrontInboxSnapshot::new(
        unread.unread_count,
        summaries,
    ))
}''',
    '''async fn load_inbox_snapshot(
    context: NotificationStorefrontTransportContext,
) -> Result<NotificationStorefrontInboxSnapshot, NativeNotificationStorefrontError> {
    let unread = load_notification_unread_count_selected(context.clone())
        .await
        .map_err(|error| NativeNotificationStorefrontError(error.to_string()))?;
    let summaries = load_notification_group_summaries_selected(
        context,
        NotificationStorefrontGroupSummaryRequest {
            cursor: None,
            limit: SUMMARY_PAGE_SIZE,
        },
    )
    .await
    .map_err(|error| NativeNotificationStorefrontError(error.to_string()))?;
    Ok(NotificationStorefrontInboxSnapshot::new(
        unread.unread_count,
        summaries,
    ))
}''',
)
