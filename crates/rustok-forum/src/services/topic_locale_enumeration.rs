impl TopicService {
    pub const MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS: usize = 512;

    pub(crate) async fn available_locales_for_topics(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        topic_ids: &[Uuid],
    ) -> ForumResult<Vec<(Uuid, Vec<String>)>> {
        enforce_scope(&security, Resource::ForumTopics, Action::Manage)?;

        if tenant_id.is_nil() {
            return Err(ForumError::Validation(
                "Forum topic locale enumeration requires a non-nil tenant id".to_string(),
            ));
        }
        if topic_ids.len() > Self::MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS {
            return Err(ForumError::Validation(format!(
                "Forum topic locale enumeration is limited to {} topic IDs",
                Self::MAX_FORUM_TOPIC_LOCALE_ENUMERATION_IDS
            )));
        }
        if topic_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut seen = std::collections::BTreeSet::new();
        for topic_id in topic_ids {
            if topic_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum topic locale enumeration requires non-nil topic IDs".to_string(),
                ));
            }
            if !seen.insert(*topic_id) {
                return Err(ForumError::Validation(format!(
                    "Forum topic locale enumeration repeats topic {topic_id}"
                )));
            }
        }

        let existing = forum_topic::Entity::find()
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .filter(forum_topic::Column::Id.is_in(topic_ids.to_vec()))
            .all(&self.db)
            .await?;
        let existing_ids = existing
            .into_iter()
            .map(|topic| topic.id)
            .collect::<std::collections::BTreeSet<_>>();
        for topic_id in topic_ids {
            if !existing_ids.contains(topic_id) {
                return Err(ForumError::TopicNotFound(*topic_id));
            }
        }

        let mut translations_by_topic = self
            .load_translations_map_for_topics(tenant_id, topic_ids)
            .await?;
        let mut result = Vec::with_capacity(topic_ids.len());
        for topic_id in topic_ids {
            let translations = translations_by_topic.remove(topic_id).unwrap_or_default();
            let locales = available_locales_from(&translations, |translation| {
                translation.locale.as_str()
            });
            if locales.is_empty() {
                return Err(ForumError::Validation(format!(
                    "Forum topic {topic_id} has no stored locale translation"
                )));
            }
            result.push((*topic_id, locales));
        }

        Ok(result)
    }
}
