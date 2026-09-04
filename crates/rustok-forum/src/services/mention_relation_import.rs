impl MentionRelationService {
    /// Persist one already-admitted FORUM-34M relation projection inside an
    /// existing Forum owner transaction.
    ///
    /// The bridge deliberately reuses the established relation persistence
    /// path so source locking, persisted-body fingerprint validation, replay
    /// detection and relation revision writes stay single-owned. Import callers
    /// provide the historical actor explicitly; no operator security context is
    /// consulted or substituted for the source author.
    pub(crate) async fn persist_import_admitted_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
        document: &RichTextDocument,
        actor_id: Option<Uuid>,
        event_mode: crate::import_relation_preparation::ForumImportRelationEventMode,
    ) -> ForumResult<Option<MentionRelationSyncResult>> {
        if tenant_id.is_nil() || relation.target.id().is_nil() {
            return Err(ForumError::Validation(
                "Forum import relation persistence requires non-nil tenant and target IDs"
                    .to_string(),
            ));
        }
        if actor_id.is_some_and(|actor_id| actor_id.is_nil()) {
            return Err(ForumError::Validation(
                "Forum import relation persistence actor ID cannot be nil".to_string(),
            ));
        }
        validate_import_relation_source(relation)?;

        let locale = normalize_locale_tag(&relation.locale).ok_or_else(|| {
            ForumError::Validation(
                "Forum import relation persistence requires a valid locale".to_string(),
            )
        })?;
        if locale != relation.locale {
            return Err(ForumError::Validation(
                "Forum import relation persistence locale must already be normalized".to_string(),
            ));
        }
        if !relation.quotes.is_empty() {
            return Err(ForumError::Validation(
                "Forum import relation persistence does not support admitted quote revisions yet"
                    .to_string(),
            ));
        }

        let policy = ForumMentionPolicy {
            max_targets: crate::mentions::FORUM_MAX_MENTION_TARGETS_PER_REVISION,
            allow_moderator_audience: true,
        };
        let extracted = extract_forum_mention_candidates(document, policy)?;

        match relation.mode {
            crate::import_relation_preparation::ForumImportRelationMode::SuppressRelations => {
                if !relation.mentions.is_empty()
                    || !relation.audiences.is_empty()
                    || !relation.quotes.is_empty()
                {
                    return Err(ForumError::Validation(
                        "Suppressed Forum import relations must not contain relation facts"
                            .to_string(),
                    ));
                }
                if event_mode
                    == crate::import_relation_preparation::ForumImportRelationEventMode::EmitAddedTargetEvents
                    && extracted.target_count() > 0
                {
                    return Err(ForumError::Validation(
                        "Forum import relation event emission requires mention relation materialization"
                            .to_string(),
                    ));
                }
                Ok(None)
            }
            crate::import_relation_preparation::ForumImportRelationMode::MaterializeRelations => {
                let resolved = ForumResolvedMentions::from_import_admission(
                    relation
                        .mentions
                        .iter()
                        .map(|mention| (mention.user_id, mention.handle.clone())),
                    relation.audiences.iter().copied(),
                )?;
                let handles = resolved
                    .users()
                    .iter()
                    .map(|mention| mention.handle().to_string())
                    .collect::<Vec<_>>();
                if handles.as_slice() != extracted.handles() {
                    return Err(ForumError::Validation(
                        "Forum import admitted mention handles do not match source RichText"
                            .to_string(),
                    ));
                }
                if resolved.audiences() != extracted.audiences() {
                    return Err(ForumError::Validation(
                        "Forum import admitted mention audiences do not match source RichText"
                            .to_string(),
                    ));
                }

                let quotes = Vec::new();
                let canonical_body = crate::richtext::serialize_discussion(document.clone())?;
                let projection_fingerprint = projection_fingerprint(
                    &canonical_body,
                    resolved.users(),
                    resolved.audiences(),
                    &quotes,
                );
                let prepared = PreparedMentionRelations {
                    tenant_id,
                    actor_id,
                    target: relation.target,
                    locale,
                    projection_fingerprint,
                    resolved,
                    quotes,
                };

                let result = match event_mode {
                    crate::import_relation_preparation::ForumImportRelationEventMode::SuppressAddedTargetEvents => {
                        let persistence = MentionRelationService {
                            profiles: self.profiles.clone(),
                            event_bus: None,
                        };
                        persistence.persist_in_tx(txn, prepared).await?
                    }
                    crate::import_relation_preparation::ForumImportRelationEventMode::EmitAddedTargetEvents => {
                        if extracted.target_count() > 0 && self.event_bus.is_none() {
                            return Err(ForumError::Validation(
                                "Forum import relation event emission requires an owner event bus"
                                    .to_string(),
                            ));
                        }
                        self.persist_in_tx(txn, prepared).await?
                    }
                };
                Ok(Some(result))
            }
        }
    }
}

fn validate_import_relation_source(
    relation: &crate::import_relation_preparation::ForumPreparedImportContentRelations,
) -> ForumResult<()> {
    if relation.source.source != crate::import_mapping::FORUM_IMPORT_SOURCE_NODEBB {
        return Err(ForumError::Validation(
            "Forum import relation persistence requires a NodeBB source reference".to_string(),
        ));
    }
    let expected_target_kind = match relation.source.kind {
        crate::import_mapping::ForumImportEntityKind::Topic => ForumContentTargetKind::Topic,
        crate::import_mapping::ForumImportEntityKind::Post => ForumContentTargetKind::Reply,
        _ => {
            return Err(ForumError::Validation(
                "Forum import relation persistence source kind cannot target content relations"
                    .to_string(),
            ));
        }
    };
    if relation.target.kind() != expected_target_kind {
        return Err(ForumError::Validation(
            "Forum import relation persistence source kind does not match owner target kind"
                .to_string(),
        ));
    }
    Ok(())
}
