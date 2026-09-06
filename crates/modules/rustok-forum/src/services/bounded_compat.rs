use std::ops::Deref;

use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::{
    ListRepliesFilter, ReplyListItem, ReplyResponse, bounded_forum_read_limit,
};
use crate::error::ForumResult;
use crate::state_machine::ReplyStatus;

use super::reply_owner::ReplyService;

impl ReplyService {
    pub async fn list_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::reply::ReplyService = Deref::deref(self);
        inner
            .list_for_topic_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
            )
            .await
    }

    pub async fn list_response_for_topic_by_statuses_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
        statuses: Option<&[ReplyStatus]>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::reply::ReplyService = Deref::deref(self);
        inner
            .list_response_for_topic_by_statuses_with_locale_fallback(
                tenant_id,
                security,
                topic_id,
                filter,
                fallback_locale,
                statuses,
            )
            .await
    }
}
