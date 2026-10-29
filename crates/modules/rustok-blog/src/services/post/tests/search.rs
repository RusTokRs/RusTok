#[tokio::test]
async fn search_audience_rechecks_live_publication_and_channel_binding() {
    let db = setup_test_db().await;
    ensure_blog_schema(&db).await;
    let events = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
    let posts = PostService::new(db.clone(), events);
    let tenant = Uuid::new_v4();
    seed_tenant(&db, tenant).await;
    seed_channel(&db, tenant, "web").await;
    seed_channel(&db, tenant, "mobile").await;
    let admin = SecurityContext::new(UserRole::Admin, Some(Uuid::new_v4()));

    let mut global = crate::tests::hardening::post_input("search-global");
    global.publish = true;
    let global_id = posts.create_post(tenant, admin.clone(), global)
        .await.expect("global published post");

    let mut mobile = crate::tests::hardening::post_input("search-mobile-only");
    mobile.publish = true;
    mobile.channel_slugs = Some(vec!["mobile".to_string()]);
    let mobile_id = posts.create_post(tenant, admin.clone(), mobile)
        .await.expect("mobile published post");

    let draft = crate::tests::hardening::post_input("search-draft");
    let draft_id = posts.create_post(tenant, admin, draft)
        .await.expect("draft post");

    let requested = vec![global_id, mobile_id, draft_id, Uuid::new_v4()];
    let owner = crate::search_projection::filter_public_search_post_ids;
    assert_eq!(
        owner(&db, tenant, Some(" WEB "), &requested).await.expect("web audience"),
        vec![global_id],
        "a search row for another channel or a draft is not discoverable"
    );
    assert_eq!(
        owner(&db, tenant, Some("mobile"), &requested).await.expect("mobile audience"),
        vec![global_id, mobile_id],
    );
    let channels = rustok_channel::ChannelService::new(db.clone());
    let mobile_channel = channels
        .get_channel_by_slug(tenant, "mobile")
        .await.expect("lookup mobile channel").expect("mobile exists");
    channels
        .bind_module(
            mobile_channel.id,
            rustok_channel::BindChannelModuleInput {
                module_slug: "blog".to_string(),
                is_enabled: false,
                settings: None,
            },
        ).await.expect("disable Blog on mobile");
    assert!(
        owner(&db, tenant, Some("mobile"), &requested)
            .await.expect("disabled channel must be hidden").is_empty(),
        "even indexed global Blog posts must not be exposed by a disabled channel"
    );
    channels
        .bind_module(
            mobile_channel.id,
            rustok_channel::BindChannelModuleInput {
                module_slug: "blog".to_string(),
                is_enabled: true,
                settings: None,
            },
        ).await.expect("re-enable Blog on mobile");
    assert_eq!(
        owner(&db, tenant, Some("mobile"), &requested).await.expect("reenabled mobile"),
        vec![global_id, mobile_id],
    );
    assert_eq!(
        owner(&db, tenant, None, &requested).await.expect("unscoped audience"),
        vec![global_id],
        "unscoped search must not publish channel-specific posts"
    );
    assert!(
        owner(&db, tenant, Some("missing-channel"), &requested)
            .await.expect("missing channel is closed").is_empty()
    );
    assert!(
        owner(&db, Uuid::new_v4(), Some("mobile"), &requested)
            .await.expect("other tenant is isolated").is_empty()
    );
}
