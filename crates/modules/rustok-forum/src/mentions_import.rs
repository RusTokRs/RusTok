impl ForumResolvedMentions {
    pub(crate) fn from_import_admission(
        mentions: impl IntoIterator<Item = (Uuid, String)>,
        audiences: impl IntoIterator<Item = ForumMentionAudience>,
    ) -> ForumResult<Self> {
        let mut by_handle = BTreeMap::new();
        let mut user_ids = BTreeSet::new();

        for (user_id, handle) in mentions {
            if user_id.is_nil() {
                return Err(ForumError::Validation(
                    "Forum import mention admission requires non-nil user IDs".to_string(),
                ));
            }
            let normalized = ProfileService::normalize_handle(&handle).map_err(|_| {
                ForumError::Validation(
                    "Forum import mention admission contains an invalid profile handle".to_string(),
                )
            })?;
            if normalized != handle {
                return Err(ForumError::Validation(
                    "Forum import mention admission handle must already be normalized".to_string(),
                ));
            }
            if by_handle.insert(normalized.clone(), user_id).is_some() {
                return Err(ForumError::Validation(
                    "Forum import mention admission repeats a normalized profile handle"
                        .to_string(),
                ));
            }
            if !user_ids.insert(user_id) {
                return Err(ForumError::Validation(
                    "Forum import mention admission maps multiple handles onto one user"
                        .to_string(),
                ));
            }
        }

        let mut unique_audiences = BTreeSet::new();
        for audience in audiences {
            if !unique_audiences.insert(audience) {
                return Err(ForumError::Validation(
                    "Forum import mention admission repeats an audience".to_string(),
                ));
            }
        }

        ensure_mention_limit(
            by_handle.len() + unique_audiences.len(),
            FORUM_MAX_MENTION_TARGETS_PER_REVISION,
        )?;

        Ok(Self {
            users: by_handle
                .into_iter()
                .map(|(handle, user_id)| ResolvedForumMention { user_id, handle })
                .collect(),
            audiences: unique_audiences.into_iter().collect(),
        })
    }
}
