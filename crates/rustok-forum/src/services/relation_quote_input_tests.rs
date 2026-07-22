#[tokio::test]
async fn inline_quote_preserve_detects_concurrent_relation_replacement() {
    let db = setup_db().await;
    let tenant_id = Uuid::new_v4();
    let topic_id = Uuid::new_v4();
    let reply_id = Uuid::new_v4();
    insert_topic_source(&db, tenant_id, topic_id, "Quoted source").await;
    insert_reply_source(&db, tenant_id, topic_id, reply_id, "Reply body").await;
    apply_relation_migrations(&db).await;

    let security = SecurityContext::system();
    let relation_service = MentionRelationService::with_profiles(Arc::new(FakeProfilesReader {
        records: HashMap::new(),
    }));
    let prepared = relation_service
        .prepare(
            tenant_id,
            ForumContentTarget::topic(topic_id),
            "en",
            "Quoted source",
            "markdown",
            &security,
            [],
        )
        .await
        .expect("quoted source should prepare");
    let txn = db.begin().await.expect("source transaction should begin");
    let source_revision = relation_service
        .persist_in_tx(&txn, prepared)
        .await
        .expect("quoted source should persist");
    txn.commit().await.expect("source transaction should commit");

    let quote = ForumQuoteReferenceInput {
        target_kind: ForumQuoteTargetKindInput::Topic,
        target_id: topic_id,
        revision_id: source_revision.source().revision_id(),
    };
    let command = ForumQuoteCommandService::new(db.clone());
    command
        .set_reply_quotes(
            tenant_id,
            reply_id,
            security.clone(),
            SetForumQuotesInput {
                locale: "en".to_string(),
                quotes: vec![quote],
            },
        )
        .await
        .expect("initial quote replacement should commit");

    let resolved = super::relation_quote_input::resolve_inline_update_quotes(
        &db,
        tenant_id,
        ForumContentTarget::reply(reply_id),
        "en",
        None,
    )
    .await
    .expect("omitted inline quotes should resolve the latest snapshot");
    let (quotes, stale_expectation) = resolved.into_parts();
    assert_eq!(quotes.len(), 1);

    command
        .set_reply_quotes(
            tenant_id,
            reply_id,
            security,
            SetForumQuotesInput {
                locale: "en".to_string(),
                quotes: Vec::new(),
            },
        )
        .await
        .expect("concurrent explicit clear should commit");

    let txn = db.begin().await.expect("CAS transaction should begin");
    let error = super::relation_quote_input::lock_source_and_assert_latest_in_tx(
        &txn,
        tenant_id,
        ForumContentTarget::reply(reply_id),
        "en",
        stale_expectation,
    )
    .await
    .expect_err("stale omitted snapshot must conflict instead of restoring old quotes");
    txn.rollback().await.expect("conflicting transaction should roll back");
    assert_eq!(error.stable_code(), "FORUM_RELATION_REVISION_CONFLICT");
    assert!(error.is_retryable());

    let explicit = super::relation_quote_input::resolve_inline_update_quotes(
        &db,
        tenant_id,
        ForumContentTarget::reply(reply_id),
        "en",
        Some(Vec::new()),
    )
    .await
    .expect("explicit clear should resolve independently of the previous snapshot");
    let (quotes, expectation) = explicit.into_parts();
    assert!(quotes.is_empty());
    assert_eq!(
        expectation,
        super::relation_quote_input::InlineQuoteExpectation::Any
    );
}
