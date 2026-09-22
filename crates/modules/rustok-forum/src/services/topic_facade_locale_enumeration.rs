impl TopicService {
    pub const MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS: usize =
        super::topic::TopicService::MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS;

    pub async fn available_locales_for_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        if security.is_public_read() {
            return Err(ForumError::forbidden(
                "Forum topic locale enumeration requires an authenticated operator context",
            ));
        }
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;
        self.inner
            .available_locales_for_topics(tenant_id, security, topic_ids)
            .await
    }
}
