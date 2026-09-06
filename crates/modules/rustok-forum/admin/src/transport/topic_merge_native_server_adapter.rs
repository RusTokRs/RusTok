use leptos::prelude::*;

use crate::topic_merge_model::{
    ForumTopicMergeCandidate, ForumTopicMergeCommand, ForumTopicMergeReceipt,
};

#[cfg(feature = "ssr")]
use super::native_server_support::{
    parse_uuid, require_forum_module_enabled, require_permission, require_tenant_scope, runtime,
};

#[cfg(feature = "ssr")]
fn map_receipt(result: rustok_forum::ForumTopicMergeResult) -> ForumTopicMergeReceipt {
    ForumTopicMergeReceipt {
        operation_id: result.operation_id.to_string(),
        event_id: result.event_id.to_string(),
        source_topic_id: result.source_topic_id.to_string(),
        target_topic_id: result.target_topic_id.to_string(),
        category_id: result.category_id.to_string(),
        actor_id: result.actor_id.to_string(),
        reason: result.reason,
        moved_reply_count: result.moved_reply_count,
        moved_published_reply_count: result.moved_published_reply_count,
        resulting_published_reply_count: result.resulting_published_reply_count,
        position_offset: result.position_offset,
        merged_at: result.merged_at.to_rfc3339(),
    }
}

#[server(prefix = "/api/fn", endpoint = "forum/topic-merge-candidates")]
pub(super) async fn fetch_topic_merge_candidates_native(
    locale: String,
) -> Result<Vec<ForumTopicMergeCandidate>, ServerFnError> {
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
            rustok_api::Permission::FORUM_TOPICS_LIST,
            "forum_topics:list required",
        )?;

        let service = rustok_forum::TopicService::new(host.db_clone(), event_bus);
        let (topics, _) = service
            .list_with_locale_fallback(
                tenant.id,
                rustok_core::SecurityContext::from_permission_snapshot(
                    Some(auth.user_id),
                    &auth.permissions,
                ),
                rustok_forum::ListTopicsFilter {
                    category_id: None,
                    status: None,
                    locale: Some(locale),
                    page: 1,
                    per_page: 100,
                },
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(topics
            .into_iter()
            .map(|topic| ForumTopicMergeCandidate {
                id: topic.id.to_string(),
                title: topic.title,
                category_id: topic.category_id.to_string(),
                reply_count: topic.reply_count,
                solution_reply_id: topic.solution_reply_id.map(|value| value.to_string()),
            })
            .collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "forum/topic-merge-candidates requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "forum/topic-merge")]
pub(super) async fn merge_topic_native(
    command: ForumTopicMergeCommand,
) -> Result<ForumTopicMergeReceipt, ServerFnError> {
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
            rustok_api::Permission::FORUM_TOPICS_MANAGE,
            "forum_topics:manage required",
        )?;

        let operation_id = parse_uuid(&command.operation_id, "operation_id")?;
        let source_topic_id = parse_uuid(&command.source_topic_id, "source_topic_id")?;
        let target_topic_id = parse_uuid(&command.target_topic_id, "target_topic_id")?;
        let selected_solution_reply_id = command
            .selected_solution_reply_id
            .as_deref()
            .map(|value| parse_uuid(value, "selected_solution_reply_id"))
            .transpose()?;

        let service = rustok_forum::ForumTopicMergeService::new(host.db_clone(), event_bus);
        let security = rustok_core::SecurityContext::from_permission_snapshot(
            Some(auth.user_id),
            &auth.permissions,
        );
        let input = rustok_forum::MergeForumTopicInput {
            operation_id,
            source_topic_id,
            reason: command.reason,
        };

        let result = match selected_solution_reply_id {
            Some(selected_solution_reply_id) => {
                service
                    .merge_topic_resolving_solution(
                        tenant.id,
                        target_topic_id,
                        security,
                        selected_solution_reply_id,
                        input,
                    )
                    .await
            }
            None => {
                service
                    .merge_topic(tenant.id, target_topic_id, security, input)
                    .await
            }
        }
        .map_err(|error| ServerFnError::new(error.to_string()))?;

        Ok(map_receipt(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = command;
        Err(ServerFnError::new(
            "forum/topic-merge requires the `ssr` feature",
        ))
    }
}
