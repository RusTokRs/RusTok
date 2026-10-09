use rustok_api::{Action, PortContext, Resource};
use rustok_core::{PermissionScope, SecurityContext};
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;
use serde_json::Value;
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::dto::{
    ForumReplyStreamWidgetPreview, ForumTopicDetailWidgetPreview, ForumTopicListWidgetPreview,
    ForumWidgetPreviewPayload, ForumWidgetPreviewResponse, ListRepliesFilter,
    PreviewForumWidgetInput, ValidateForumWidgetPropsInput,
};
use crate::error::{ForumError, ForumResult};
use crate::services::topic::WidgetTopicListQuery;
use crate::services::widget_contract::{
    FORUM_WIDGET_CONTRACT_VERSION, FORUM_WIDGET_TYPE_REPLY_STREAM, FORUM_WIDGET_TYPE_TOPIC_DETAIL,
    FORUM_WIDGET_TYPE_TOPIC_LIST, ForumWidgetContractService,
};
use crate::services::{
    ForumReplyAudienceReadService, ForumTopicAudienceReadService,
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService, TopicService,
};
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
/// Forum owner audience reads for the caller's exact `PortContext`. Every topic, reply, and list
/// the preview returns is checked against the parent topic's owner audience, so a preview cannot
/// show content that the caller cannot read through the owner transports. The service never
/// accepts tenant or actor identity inside widget props.
pub struct ForumWidgetPreviewService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
    audience_facts: Option<SharedForumAudienceFactsPort>,
}

impl ForumWidgetPreviewService {
    /// `audience_facts` must be the host-published facts port when one is installed. Without
    /// it, any layer that needs trust, Channel, or group facts is denied (fail closed).
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        audience_facts: Option<SharedForumAudienceFactsPort>,
    ) -> Self {
        Self {
            db,
            event_bus,
            audience_facts,
        }
    }

    /// Previews one widget for an authenticated caller.
    ///
    /// `audience` is the exact read context built by the transport from the authenticated
    /// principal. Its locale is the request locale. A `locale` prop on a topic-detail widget
    /// overrides it for that widget only.
    pub async fn preview(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        audience: PortContext,
        fallback_locale: Option<&str>,
        input: PreviewForumWidgetInput,
    ) -> ForumResult<ForumWidgetPreviewResponse> {
        // Rejects public or non-user security and any actor/tenant mismatch before any data read.
        ForumTopicAudienceViewer::authenticated(security.clone(), audience.clone())?;

        let validation =
            ForumWidgetContractService::validate_props(ValidateForumWidgetPropsInput {
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
                    &audience,
                    fallback_locale,
                    &normalized_props,
                )
                .await?,
            ),
            FORUM_WIDGET_TYPE_TOPIC_DETAIL => ForumWidgetPreviewPayload::TopicDetail(
                self.preview_topic_detail(
                    tenant_id,
                    security,
                    &audience,
                    fallback_locale,
                    &normalized_props,
                )
                .await?,
            ),
            FORUM_WIDGET_TYPE_REPLY_STREAM => ForumWidgetPreviewPayload::ReplyStream(
                self.preview_reply_stream(
                    tenant_id,
                    security,
                    &audience,
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

    fn visibility_service(&self) -> ForumTopicAudienceVisibilityService {
        ForumTopicAudienceVisibilityService::new(self.db.clone(), self.audience_facts.clone())
    }

    fn topic_audience_read_service(&self) -> ForumTopicAudienceReadService {
        match self.audience_facts.clone() {
            Some(facts) => ForumTopicAudienceReadService::with_audience_facts(
                self.db.clone(),
                self.event_bus.clone(),
                facts,
            ),
            None => ForumTopicAudienceReadService::new(self.db.clone(), self.event_bus.clone()),
        }
    }

    fn reply_audience_read_service(&self) -> ForumReplyAudienceReadService {
        match self.audience_facts.clone() {
            Some(facts) => ForumReplyAudienceReadService::with_audience_facts(
                self.db.clone(),
                self.event_bus.clone(),
                facts,
            ),
            None => ForumReplyAudienceReadService::new(self.db.clone(), self.event_bus.clone()),
        }
    }

    async fn preview_topic_list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        audience: &PortContext,
        fallback_locale: Option<&str>,
        props: &Value,
    ) -> ForumResult<ForumTopicListWidgetPreview> {
        let category_id = optional_uuid(props, "category_id")?;
        let page = required_u64(props, "page")?;
        let per_page = required_u64(props, "per_page")?;
        let include_pinned = required_bool(props, "include_pinned")?;
        let sort = required_string(props, "sort")?;
        let visibility = self.visibility_service();
        let (items, total) = TopicService::new(self.db.clone(), self.event_bus.clone())
            .list_widget_preview_owner_visible(
                tenant_id,
                security,
                audience.clone(),
                &visibility,
                WidgetTopicListQuery {
                    category_id,
                    page,
                    per_page,
                    include_pinned,
                    sort,
                },
                fallback_locale,
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
        audience: &PortContext,
        fallback_locale: Option<&str>,
        props: &Value,
    ) -> ForumResult<ForumTopicDetailWidgetPreview> {
        let topic_id = required_uuid(props, "topic_id")?;
        let include_replies = required_bool(props, "include_replies")?;
        let requested_locale = props
            .get("locale")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(audience.locale.as_str())
            .to_string();
        let mut context = audience.clone();
        context.locale = requested_locale.clone();

        let topic = self
            .topic_audience_read_service()
            .get_authenticated_owner_visible_with_audience_context(
                tenant_id,
                security.clone(),
                context.clone(),
                topic_id,
                fallback_locale,
            )
            .await?;

        let (replies, next_cursor) = if include_replies {
            let page = self
                .reply_audience_read_service()
                .list_response_authenticated_owner_visible_with_audience_context(
                    tenant_id,
                    security,
                    context,
                    topic_id,
                    ListRepliesFilter {
                        locale: Some(requested_locale),
                        after: None,
                        per_page: TOPIC_DETAIL_PREVIEW_REPLIES,
                    },
                    fallback_locale,
                    Some(&APPROVED_PREVIEW_REPLY_STATUSES),
                )
                .await?;
            (page.items, page.next_cursor)
        } else {
            (Vec::new(), None)
        };

        Ok(ForumTopicDetailWidgetPreview {
            topic,
            replies,
            next_cursor,
            include_replies,
        })
    }

    async fn preview_reply_stream(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        audience: &PortContext,
        fallback_locale: Option<&str>,
        props: &Value,
    ) -> ForumResult<ForumReplyStreamWidgetPreview> {
        let topic_id = required_uuid(props, "topic_id")?;
        let after = props
            .get("after")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let per_page = required_u64(props, "per_page")?;
        let approved_only = required_bool(props, "approved_only")?;
        let statuses = reply_stream_preview_statuses(approved_only, &security)?;
        let page = self
            .reply_audience_read_service()
            .list_response_authenticated_owner_visible_with_audience_context(
                tenant_id,
                security,
                audience.clone(),
                topic_id,
                ListRepliesFilter {
                    locale: Some(audience.locale.clone()),
                    after,
                    per_page,
                },
                fallback_locale,
                Some(statuses),
            )
            .await?;

        Ok(ForumReplyStreamWidgetPreview {
            topic_id: topic_id.to_string(),
            items: page.items,
            next_cursor: page.next_cursor,
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
        .ok_or_else(|| {
            ForumError::Validation(format!("Normalized widget field `{field}` is missing"))
        })
}

fn required_u64(props: &Value, field: &str) -> ForumResult<u64> {
    props.get(field).and_then(Value::as_u64).ok_or_else(|| {
        ForumError::Validation(format!("Normalized widget field `{field}` is missing"))
    })
}

fn required_bool(props: &Value, field: &str) -> ForumResult<bool> {
    props.get(field).and_then(Value::as_bool).ok_or_else(|| {
        ForumError::Validation(format!("Normalized widget field `{field}` is missing"))
    })
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
    use rustok_api::Permission;

    fn security(permissions: Vec<Permission>) -> SecurityContext {
        SecurityContext::from_permission_snapshot(Some(Uuid::new_v4()), &permissions)
    }

    #[test]
    fn approved_reply_stream_never_requires_moderation_scope() {
        let statuses =
            reply_stream_preview_statuses(true, &security(vec![Permission::FORUM_TOPICS_READ]))
                .expect("approved-only widget preview should not require moderation");
        assert_eq!(statuses, &[ReplyStatus::Approved]);
    }

    #[test]
    fn non_approved_reply_stream_requires_effective_moderation_scope() {
        let denied =
            reply_stream_preview_statuses(false, &security(vec![Permission::FORUM_TOPICS_READ]))
                .expect_err("non-approved preview must require reply moderation");
        assert!(denied.to_string().contains("forum_replies:moderate"));

        let statuses = reply_stream_preview_statuses(
            false,
            &security(vec![Permission::new(
                Resource::ForumReplies,
                Action::Manage,
            )]),
        )
        .expect("manage should satisfy the effective moderation scope");
        assert_eq!(statuses, &MODERATOR_PREVIEW_REPLY_STATUSES);
        assert!(statuses.contains(&ReplyStatus::Pending));
        assert!(statuses.contains(&ReplyStatus::Hidden));
        assert!(statuses.contains(&ReplyStatus::Flagged));
        assert!(!statuses.contains(&ReplyStatus::Deleted));
    }
}
