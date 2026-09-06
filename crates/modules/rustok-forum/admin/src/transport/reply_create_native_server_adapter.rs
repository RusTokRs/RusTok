use leptos::prelude::*;

use crate::model::{ReplyDraft, ReplyListItem};

#[cfg(feature = "ssr")]
use super::native_server_support::{
    parse_uuid, require_forum_module_enabled, require_permission, require_tenant_scope, runtime,
};

#[server(prefix = "/api/fn", endpoint = "forum/reply-create")]
pub(super) async fn create_reply_native(
    topic_id: String,
    draft: ReplyDraft,
) -> Result<ReplyListItem, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        require_tenant_scope(&auth, &tenant)?;
        let (host, event_bus) = runtime()?;
        require_forum_module_enabled(&host, tenant.id).await?;
        require_permission(
            &auth,
            rustok_api::Permission::FORUM_REPLIES_CREATE,
            "forum_replies:create required",
        )?;

        let topic_id = parse_uuid(topic_id.as_str(), "topic_id")?;
        let parent_reply_id = draft
            .parent_reply_id
            .as_deref()
            .map(|value| parse_uuid(value, "parent_reply_id"))
            .transpose()?;
        let reply = rustok_forum::ReplyService::new(host.db_clone(), event_bus)
            .create(
                tenant.id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                topic_id,
                rustok_forum::CreateReplyInput {
                    locale: draft.locale,
                    content: draft.content,
                    parent_reply_id,
                },
            )
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(ReplyListItem {
            id: reply.id.to_string(),
            locale: reply.locale,
            effective_locale: reply.effective_locale,
            topic_id: reply.topic_id.to_string(),
            author_id: reply.author_id.map(|value| value.to_string()),
            content_preview: reply.content_plain_text,
            status: reply.status,
            parent_reply_id: reply.parent_reply_id.map(|value| value.to_string()),
            created_at: reply.created_at,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (topic_id, draft);
        Err(ServerFnError::new(
            "forum/reply-create requires the `ssr` feature",
        ))
    }
}
