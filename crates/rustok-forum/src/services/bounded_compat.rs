use std::ops::Deref;

use uuid::Uuid;

use rustok_core::SecurityContext;

use crate::dto::{
    ListRepliesFilter, ListTopicsFilter, ReplyListItem, ReplyResponse, TopicListItem,
    bounded_forum_read_limit,
};
use crate::error::ForumResult;
use crate::state_machine::ReplyStatus;

use super::reply_owner::ReplyService;
use super::topic_owner::TopicService;

impl TopicService {
    pub async fn list(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::topic::TopicService = Deref::deref(self);
        inner.list(tenant_id, security, filter).await
    }

    pub async fn list_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::topic::TopicService = Deref::deref(self);
        inner
            .list_with_locale_fallback(tenant_id, security, filter, fallback_locale)
            .await
    }

    pub async fn list_storefront_visible_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        mut filter: ListTopicsFilter,
        fallback_locale: Option<&str>,
        channel_slug: Option<&str>,
    ) -> ForumResult<(Vec<TopicListItem>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::topic::TopicService = Deref::deref(self);
        inner
            .list_storefront_visible_with_locale_fallback(
                tenant_id,
                security,
                filter,
                fallback_locale,
                channel_slug,
            )
            .await
    }
}

impl ReplyService {
    pub async fn list_for_topic(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
    ) -> ForumResult<(Vec<ReplyListItem>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::reply::ReplyService = Deref::deref(self);
        inner
            .list_for_topic(tenant_id, security, topic_id, filter)
            .await
    }

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

    pub async fn list_response_for_topic_with_locale_fallback(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_id: Uuid,
        mut filter: ListRepliesFilter,
        fallback_locale: Option<&str>,
    ) -> ForumResult<(Vec<ReplyResponse>, u64)> {
        filter.per_page = bounded_forum_read_limit(Some(filter.per_page));
        let inner: &super::reply::ReplyService = Deref::deref(self);
        inner
            .list_response_for_topic_with_locale_fallback(
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
