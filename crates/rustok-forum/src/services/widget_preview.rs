use rustok_api::{Action, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::dto::{
    ForumReplyStreamWidgetPreview, ForumTopicDetailWidgetPreview, ForumTopicListWidgetPreview,
    ForumWidgetPreviewPayload, ForumWidgetPreviewResponse, ListRepliesFilter,
    PreviewForumWidgetInput, ValidateForumWidgetPropsInput,
};
use crate::error::{ForumError, ForumResult};
use crate::services::topic::TopicService as TopicPersistenceService;
use crate::services::topic_visibility::ForumTopicVisibilityService;
use crate::services::widget_contract::{
    FORUM_WIDGET_CONTRACT_VERSION, FORUM_WIDGET_TYPE_REPLY_STREAM,
    FORUM_WIDGET_TYPE_TOPIC_DETAIL, FORUM_WIDGET_TYPE_TOPIC_LIST, ForumWidgetContractService,
};
use crate::services::{ReplyService, TopicService};
use crate::state_machine::ReplyStatus;

const TOPIC_DETAIL_PREVIEW_REPLIES: u64 = 20;
const APPROVED_PREVIEW_REPLY_STATUSES: [ReplyStatus; 1] = [ReplyStatus::Approved];
const MODERATOR_PREVIEW_REPLY_STATUSES: [ReplyStatus; 5] = [
    ReplyStatus::Pending,
    ReplyStatus::Approved,
    ReplyStatus::Rejected,
    ReplyStatus::Hidden,
    ReplyStatus::Flagged,
];

/// Forum owner runtime for Page Builder widget previews.
///
/// All widget configuration crosses `ForumWidgetContractService` first. Data then comes only from
/// Forum owner read services under the caller's exact security snapshot. The service never accepts
/// tenant or actor identity inside widget props.
pub struct ForumWidgetPreviewService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl ForumWidgetPreviewService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }

    pub async fn preview(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        input: PreviewForumWidgetInput,
    ) -> ForumResult<ForumWidgetPreviewResponse> {
        let validation = ForumWidgetContractService::validate_props(ValidateForumWidgetPropsInput {
            widget_type: input.widget_type,
            props: input.props,
        });
        let widget_type = validation.widget_type.clone();
        if !validation.valid {
            return Ok(ForumWidgetPreviewResponse {
                widget_type,
                data_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
                valid: false,
                normalized_props: validation.normalized_props,
                issues: validation.issues,
                payload: None,
            });
        }

        let normalized_props = validation.normalized_props.clone();
        let payload = match widget_type.as_str() {
            FORUM_WIDGET_TYPE_TOPIC_LIST => ForumWidgetPreviewPayload::TopicList(
                self.preview_topic_list(
                    tenant_id,
                    security,
                    locale,
                    fallback_locale,
                    &normalized_props,
                )
                .await?,
            ),
            FORUM_WIDGET_TYPE_TOPIC_DETAIL => ForumWidgetPreviewPayload::TopicDetail(
                self.preview_topic_detail(
                    tenant_id,
                    security,
                    locale,
                    fallback_locale,
                    &normalized_props,
                )
                .await?,
            ),
            FORUM_WIDGET_TYPE_REPLY_STREAM => ForumWidgetPreviewPayload::ReplyStream(
                self.preview_reply_stream(
                    tenant_id,
                    security,
                    locale,
                    fallback_locale,
                    &normalized_props,
                )
                .await?,
            ),
            _ => {
                return Err(ForumError::Validation(format!(
                    "Unsupported normalized Forum widget type: {widget_type}"
                )));
            }
        };

        Ok(ForumWidgetPreviewResponse {
            widget_type,
            data_contract_version: FORUM_WIDGET_CONTRACT_VERSION.to_string(),
            valid: true,
            normalized_props,
            issues: validation.issues,
            payload: Some(payload),
        })
    }

    async fn preview_topic_list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        props: &Value,
    ) -> ForumResult<ForumTopicListWidgetPreview> {
        let category_id = optional_uuid(props, "category_id")?;
        let page = required_u64(props, "page")?;
        let per_page = required_u64(props, "per_page")?;
        let include_pinned = required_bool(props, "include_pinned")?;
        let sort = required_string(props, "sort")?;
        let hidden_category_ids = ForumTopicVisibilityService::new(self.db.clone())
            .hidden_category_ids_for_viewer(tenant_id, !security.is_public_read())
            .await?;
        let (items, total) = TopicPersistenceService::new(
            self.db.clone(),
            self.event_bus.clone(),
        )
        .list_widget_preview_with_locale_fallback_and_hidden_categories(
            tenant_id,
            security,
            category_id,
            page,
            per_page,
            include_pinned,
            sort,
            locale,
            fallback_locale,
            &hidden_category_ids,
        )
        .await?;

        Ok(ForumTopicListWidgetPreview {
            items,
            total,
            page,
            per_page,
            sort: sort.to_string(),
            include_pinned,
        })
    }

    async fn preview_topic_detail(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        props: &Value,
    ) -> ForumResult<ForumTopicDetailWidgetPreview> {
        let topic_id = required_uuid(props, "topic_id")?;
        let include_replies = required_bool(props, "include_replies")?;
        let requested_locale = props
            .get("locale")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(locale);
        let topic = TopicService::new(self.db.clone(), self.event_bus.clone())
            .get_with_locale_fallback(
                tenant_id,
                security.clone(),
                topic_id,
                requested_locale,
                fallback_locale,
            )
            .await?;

        let (replies, replies_total) = if include_replies {
            ReplyService::new(self.db.clone(), self.event_bus.clone())
                .list_response_for_topic_by_statuses_with_locale_fallback(
                    tenant_id,
                    security,
                    topic_id,
                    ListRepliesFilter {
                        locale: Some(requested_locale.to_string()),
                        page: 1,
                        per_page: TOPIC_DETAIL_PREVIEW_REPLIES,
                    },
                    fallback_locale,
                    Some(&APPROVED_PREVIEW_REPLY_STATUSES),
                )
                .await?
        } else {
            (Vec::new(), 0)
        };

        Ok(ForumTopicDetailWidgetPreview {
            topic,
            replies,
            replies_total,
            include_replies,
        })
    }

    async fn preview_reply_stream(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        locale: &str,
        fallback_locale: Option<&str>,
        props: &Value,
    ) -> ForumResult<ForumReplyStreamWidgetPreview> {
        let topic_id = required_uuid(props, "topic_id")?;
        let page = required_u64(props, "page")?;
        let per_page = required_u64(props, "per_page")?;
        let approved_only = required_bool(props, "approved_only")?;
        let statuses = reply_stream_preview_statuses(approved_only, &security)?;
        let (items, total) = ReplyService::new(self.db.clone(), self.event_bus.clone())
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                ListRepliesFilter {
                    locale: Some(locale.to_string()),
                    page,
                    per_page,
                },
                fallback_locale,
                Some(statuses),
            )
            .await?;

        Ok(ForumReplyStreamWidgetPreview {
            topic_id: topic_id.to_string(),
            items,
            total,
            page,
            per_page,
            approved_only,
        })
    }
}

fn reply_stream_preview_statuses(
    approved_only: bool,
    security: &SecurityContext,
) -> ForumResult<&'static [ReplyStatus]> {
    if approved_only {
        return Ok(&APPROVED_PREVIEW_REPLY_STATUSES);
    }
    if security.get_scope(Resource::ForumReplies, Action::Moderate) == PermissionScope::None {
        return Err(ForumError::forbidden(
            "Forum widget preview requires forum_replies:moderate when approved_only=false",
        ));
    }
    Ok(&MODERATOR_PREVIEW_REPLY_STATUSES)
}

fn required_string<'a>(props: &'a Value, field: &str) -> ForumResult<&'a str> {
    props
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| ForumError::Validation(format!("Normalized widget field `{field}` is missing")))
}

fn required_u64(props: &Value, field: &str) -> ForumResult<u64> {
    props
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| ForumError::Validation(format!("Normalized widget field `{field}` is missing")))
}

fn required_bool(props: &Value, field: &str) -> ForumResult<bool> {
    props
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| ForumError::Validation(format!("Normalized widget field `{field}` is missing")))
}

fn required_uuid(props: &Value, field: &str) -> ForumResult<Uuid> {
    let value = required_string(props, field)?;
    Uuid::parse_str(value).map_err(|_| {
        ForumError::Validation(format!("Normalized widget field `{field}` is not a UUID"))
    })
}

fn optional_uuid(props: &Value, field: &str) -> ForumResult<Option<Uuid>> {
    props
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            Uuid::parse_str(value).map_err(|_| {
                ForumError::Validation(format!("Normalized widget field `{field}` is not a UUID"))
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{Action, Permission, Resource};

    fn security(permissions: Vec<Permission>) -> SecurityContext {
        SecurityContext::from_permission_snapshot(Some(Uuid::new_v4()), &permissions)
    }

    #[test]
    fn approved_reply_stream_never_requires_moderation_scope() {
        let statuses = reply_stream_preview_statuses(
            true,
            &security(vec![Permission::FORUM_TOPICS_READ]),
        )
        .expect("approved-only widget preview should not require moderation");
        assert_eq!(statuses, &[ReplyStatus::Approved]);
    }

    #[test]
    fn non_approved_reply_stream_requires_effective_moderation_scope() {
        let denied = reply_stream_preview_statuses(
            false,
            &security(vec![Permission::FORUM_TOPICS_READ]),
        )
        .expect_err("non-approved preview must require reply moderation");
        assert!(denied.to_string().contains("forum_replies:moderate"));

        let statuses = reply_stream_preview_statuses(
            false,
            &security(vec![Permission::new(Resource::ForumReplies, Action::Manage)]),
        )
        .expect("manage should satisfy the effective moderation scope");
        assert_eq!(statuses, &MODERATOR_PREVIEW_REPLY_STATUSES);
        assert!(statuses.contains(&ReplyStatus::Pending));
        assert!(statuses.contains(&ReplyStatus::Hidden));
        assert!(statuses.contains(&ReplyStatus::Flagged));
        assert!(!statuses.contains(&ReplyStatus::Deleted));
    }
}
