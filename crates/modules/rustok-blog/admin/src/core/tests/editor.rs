use super::*;

#[test]
fn pending_creation_retains_original_command_until_a_durable_outcome() {
    let post = editor_post();
    let original = editor_draft(&post);
    let mut session = PostEditorSession::Creating;
    let request = session.retain_creation(original.clone());
    let mut changed = original;
    changed.command_id = uuid::Uuid::new_v4();
    changed.title = "Different input after timeout".into();
    let retried = session.retain_creation(changed.clone());
    assert_eq!(retried.command_id, request.command_id);
    assert_eq!(retried.title, request.title);
    assert!(session.can_save(), "retry remains available");
    session = PostEditorSession::Creating; // only durable rejection unlocks editing
    assert_eq!(
        session.retain_creation(changed.clone()).command_id,
        changed.command_id
    );
}

#[test]
fn editor_principal_preserves_same_actor_refresh_but_separates_actor_tenant_and_logout() {
    let signed_in = EditorPrincipal {
        actor_id: Some("actor-a".into()),
        tenant: Some("tenant-a".into()),
    };
    // The replacement credential is intentionally absent from the editor key.
    // A host user snapshot refreshed for the same actor compares equal, so the
    // render adapter's Memo retains the unsaved draft and active request.
    let refreshed = EditorPrincipal {
        actor_id: Some("actor-a".into()),
        tenant: Some("tenant-a".into()),
    };
    assert_eq!(signed_in, refreshed);
    assert_ne!(
        signed_in,
        EditorPrincipal {
            actor_id: Some("actor-b".into()),
            ..refreshed.clone()
        }
    );
    assert_ne!(
        signed_in,
        EditorPrincipal {
            tenant: Some("tenant-b".into()),
            ..refreshed.clone()
        }
    );
    assert_ne!(
        signed_in,
        EditorPrincipal {
            actor_id: None,
            ..refreshed
        }
    );
}

#[test]
fn fallback_editor_global_edit_emits_no_localized_fields() {
    let mut post = editor_post();
    post.requested_locale = "fr".into();
    let state = BlogPostEditorFormState::from_post(&post);
    assert!(state.title.is_empty());
    assert!(state.excerpt.is_empty());
    assert!(state.seo_title.is_empty());
    assert_eq!(state.content, RichTextDocument::empty());
    let mut draft = editor_draft(&post);
    draft.category_id = Some("next-category".into());
    let command = prepare_blog_post_save_command(
        Some(post.id.clone()),
        Some(&post),
        draft,
        "required".into(),
        "slug required".into(),
    )
    .unwrap();
    let BlogPostSaveOperation::Update { update, .. } = command.operation else {
        panic!("update expected");
    };
    assert_eq!(
        serde_json::to_value(update).unwrap(),
        serde_json::json!({"version":8,"categoryId":"next-category"})
    );
}
#[test]
fn missing_target_translation_requires_own_title_and_body() {
    let mut post = editor_post();
    post.requested_locale = "fr".into();
    let mut draft = editor_draft(&post);
    draft.title = "Cible".into();
    assert!(
        prepare_blog_post_save_command(
            Some(post.id.clone()),
            Some(&post),
            draft.clone(),
            "required".into(),
            "slug required".into()
        )
        .is_err()
    );
    draft.content = RichTextDocument::single_paragraph("Texte cible");
    let command = prepare_blog_post_save_command(
        Some(post.id.clone()),
        Some(&post),
        draft,
        "required".into(),
        "slug required".into(),
    )
    .unwrap();
    let BlogPostSaveOperation::Update { update, .. } = command.operation else {
        panic!("update expected");
    };
    assert_eq!(update.locale.as_deref(), Some("fr"));
    assert_eq!(update.title.as_deref(), Some("Cible"));
    assert!(update.excerpt.is_keep());
    assert!(update.seo_title.is_keep());
}
#[test]
fn update_serialization_preserves_nullable_keep_clear_and_set() {
    let post = editor_post();
    let mut draft = editor_draft(&post);
    draft.excerpt.clear();
    draft.seo_title = Some("Next SEO".into());
    let update = build_post_update(&draft, &post);
    assert_eq!(
        serde_json::to_value(update).unwrap(),
        serde_json::json!({"version":8,"locale":"en","excerpt":null,"seoTitle":"Next SEO"})
    );
    assert_eq!(
        serde_json::to_value(build_post_update(&editor_draft(&post), &post)).unwrap(),
        serde_json::json!({"version":8})
    );
}
#[test]
fn editor_rejects_changed_locale_or_revision_before_transport() {
    let post = editor_post();
    let mut draft = editor_draft(&post);
    draft.locale = "de".into();
    assert!(
        prepare_blog_post_save_command(
            Some(post.id.clone()),
            Some(&post),
            draft,
            "reload".into(),
            "slug required".into()
        )
        .is_err()
    );
    let mut draft = editor_draft(&post);
    draft.version = Some(7);
    assert!(
        prepare_blog_post_save_command(
            Some(post.id.clone()),
            Some(&post),
            draft,
            "reload".into(),
            "slug required".into()
        )
        .is_err()
    );
    assert_eq!(post.version, 8);
}
#[test]
fn category_fallback_editor_requires_its_own_name() {
    let mut category = category_snapshot();
    category.locale = "fr".into();
    assert_eq!(
        category_editor_copy(&category),
        CategoryEditorCopy {
            name: String::new(),
            slug: String::new(),
            description: String::new()
        }
    );
    let result = prepare_blog_category_save_command(
        BlogCategoryFormInput {
            editing_category_id: Some(&category.id),
            loaded_category: Some(&category),
            locale: "fr",
            name: "",
            slug: "next-route",
            description: "",
            parent_id: None,
            position: "0",
        },
        "name required".into(),
        "position".into(),
        "slug required".into(),
    );
    assert!(result.is_err());
}

#[test]
fn unavailable_category_options_preserve_current_assignment() {
    let options = post_category_options(&[], "current", "None".into(), "Current category".into());
    assert_eq!(options.len(), 2);
    assert_eq!(options[1].value, "current");
    let items = [crate::model::BlogCategoryOption {
        id: "current".into(),
        name: "Visible".into(),
    }];
    let options =
        post_category_options(&items, "current", "None".into(), "Current category".into());
    assert_eq!(options.len(), 2);
    assert_eq!(options[1].label, "Visible");
}

#[test]
fn acknowledged_write_consumes_revision_and_cannot_be_resubmitted_after_failed_reload() {
    use crate::core::PostEditorSession;
    let post = editor_post();
    let draft = editor_draft(&post);
    let mut session = PostEditorSession::Loaded(Box::new(post.clone()));
    assert!(session.can_save());
    assert_eq!(session.version(), Some(8));
    session = PostEditorSession::Acknowledged {
        post_id: post.id.clone(),
    };
    assert_eq!(session.reload_notice_key(), None);
    session = PostEditorSession::ReloadFailed {
        post_id: post.id.clone(),
    };
    assert_eq!(
        session.reload_notice_key(),
        Some("blog.editor.savedReloadFailed")
    );
    // A failed reload keeps the saved identity: notice, no predecessor
    // snapshot, and an edited identity (so it cannot become a duplicate create).
    assert!(session.awaiting_reload());
    assert!(!session.can_save());
    assert_eq!(session.post_id(), Some(post.id.as_str()));
    assert_eq!(session.version(), None);
    assert!(session.snapshot().is_none());
    assert!(
        prepare_blog_post_save_command(
            session.post_id().map(str::to_string),
            session.snapshot(),
            draft,
            "reload".into(),
            "slug required".into()
        )
        .is_err()
    );
    let refreshed = PostEditorSession::Loaded(Box::new(crate::model::BlogPostDetail {
        version: 9,
        ..post
    }));
    assert!(refreshed.can_save());
    assert!(!refreshed.awaiting_reload());
    assert_eq!(refreshed.version(), Some(9));
}

#[test]
fn superseded_completions_cannot_relabel_or_replace_the_selected_snapshot() {
    let a = editor_post();
    let first = EditorRequest::new(Some(a.id.clone()), "en".into());
    let mut b = a.clone();
    b.id = "other-post".into();
    b.requested_locale = "fr".into();
    let second = EditorRequest::new(Some(b.id.clone()), "fr".into());
    assert!(!first.is_current(Some(&second)));
    assert!(!first.accepts_post(Some(&second), &a));
    assert!(second.accepts_post(Some(&second), &b));
    assert!(!second.accepts_post(Some(&second), &a));
    // Returning to the same post and locale starts a different request; an old
    // write acknowledgement or load response is still superseded.
    let third = EditorRequest::new(Some(a.id.clone()), "en".into());
    assert!(!first.is_current(Some(&third)));
    assert!(!third.is_current(None));
    let mut wrong_locale = a.clone();
    wrong_locale.requested_locale = "ru".into();
    assert!(!third.accepts_post(Some(&third), &wrong_locale));
    let create = EditorRequest::new(None, "en".into());
    assert!(create.accepts_post(Some(&create), &a));
}

#[test]
fn canonical_post_slug_cannot_be_silently_cleared_on_update() {
    let post = editor_post();
    let mut draft = editor_draft(&post);
    draft.slug = "   ".into();
    let issue = prepare_blog_post_save_command(
        Some(post.id.clone()),
        Some(&post),
        draft,
        "copy required".into(),
        "slug required".into(),
    )
    .unwrap_err();
    assert_eq!(issue.message, "slug required");
}

#[test]
fn category_route_clear_fails_for_existing_copy_but_missing_target_may_generate_route() {
    let mut category = category_snapshot();
    let make_command = |category: &crate::model::BlogCategoryDetail| {
        prepare_blog_category_save_command(
            BlogCategoryFormInput {
                editing_category_id: Some(&category.id),
                loaded_category: Some(category),
                locale: &category.locale,
                name: "Target name",
                slug: "",
                description: "",
                parent_id: None,
                position: "0",
            },
            "name".into(),
            "position".into(),
            "slug required".into(),
        )
    };
    assert_eq!(
        make_command(&category).unwrap_err().message,
        "slug required"
    );
    category.locale = "fr".into();
    let command = make_command(&category).unwrap();
    let BlogCategorySaveOperation::Update { update, .. } = command.operation else {
        panic!("update expected");
    };
    assert_eq!(update.name.as_deref(), Some("Target name"));
    assert!(update.slug.is_none());
}
