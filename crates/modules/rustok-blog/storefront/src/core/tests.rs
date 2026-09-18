use super::*;

#[test]
    fn storefront_route_state_normalizes_slug_and_segment_contract() {
        let route_state = build_storefront_route_state(
            Some("  release-notes  ".to_string()),
            Some("  stories  ".to_string()),
        );

        assert_eq!(route_state.selected_slug, "release-notes".to_string());
        assert_eq!(route_state.selected_slug_query_key, SELECTED_POST_QUERY_KEY);
        assert_eq!(route_state.route_segment, "stories".to_string());

        let fallback_state = build_storefront_route_state(Some("   ".to_string()), None);
        assert_eq!(fallback_state.selected_slug, DEFAULT_POST_SLUG.to_string());
        assert_eq!(
            fallback_state.route_segment,
            DEFAULT_ROUTE_SEGMENT.to_string()
        );
    }

    #[test]
    fn storefront_shell_view_model_resolves_copy_from_effective_locale() {
        let view = build_storefront_shell_view_model(Some("en"));

        assert_eq!(view.badge, "blog".to_string());
        assert_eq!(
            view.title,
            "Stories published from the module package".to_string()
        );
        assert_eq!(
            view.load_error,
            "Failed to load blog storefront data".to_string()
        );
        assert!(view.subtitle.contains("GraphQL"));
    }

    #[test]
    fn storefront_fetch_request_uses_core_route_state_and_host_locale() {
        let route_state = build_storefront_route_state(
            Some("  release-notes  ".to_string()),
            Some("blog".to_string()),
        );

        let request = build_storefront_fetch_request(&route_state, Some("  ru  ".to_string()));

        assert_eq!(request.post_slug, "release-notes".to_string());
        assert_eq!(request.locale.as_deref(), Some("ru"));

        let empty_locale_request =
            build_storefront_fetch_request(&route_state, Some("   ".to_string()));
        assert_eq!(empty_locale_request.locale, None);
    }

    #[test]
    fn fallback_text_returns_fallback_for_none() {
        assert_eq!(fallback_text(None, "fallback"), "fallback".to_string());
    }

    #[test]
    fn published_posts_total_label_delegates_to_count_label() {
        assert_eq!(
            published_posts_total_label(7, "total"),
            "7 total".to_string()
        );
    }

    #[test]
    fn published_posts_header_view_returns_title_and_total_label() {
        assert_eq!(
            published_posts_header_view("Published posts".to_string(), 3, "total"),
            ("Published posts".to_string(), "3 total".to_string())
        );
    }

    #[test]
    fn published_posts_header_typed_view_builds_struct() {
        let view = published_posts_header_typed_view("Published posts".to_string(), 3, "total");
        assert_eq!(view.title, "Published posts".to_string());
        assert_eq!(view.total_label, "3 total".to_string());
    }

    #[test]
    fn selected_post_empty_state_view_returns_payload_tuple() {
        assert_eq!(
            selected_post_empty_state_view(
                "Pick a published post".to_string(),
                "Open a post from the list below.".to_string(),
            ),
            (
                "Pick a published post".to_string(),
                "Open a post from the list below.".to_string(),
            )
        );
    }

    #[test]
    fn selected_post_empty_state_typed_view_builds_struct() {
        let view = selected_post_empty_state_typed_view(
            "Pick a published post".to_string(),
            "Open a post from the list below.".to_string(),
        );
        assert_eq!(view.title, "Pick a published post".to_string());
        assert_eq!(view.body, "Open a post from the list below.".to_string());
    }

    #[test]
    fn published_posts_empty_state_typed_view_builds_struct() {
        let view = published_posts_empty_state_typed_view("No items".to_string());
        assert_eq!(view.message, "No items".to_string());
    }

    #[test]
    fn status_badge_typed_view_builds_struct() {
        let view = status_badge_typed_view(" archived ".to_string(), "unknown");
        assert_eq!(view.label, "archived".to_string());
        assert_eq!(
            view.badge_css,
            "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
        );
    }

    #[test]
    fn post_link_typed_view_builds_struct() {
        let view = post_link_typed_view("/store/modules/blog", "hello-world", "Open");
        assert_eq!(
            view.href,
            "/store/modules/blog?slug=hello-world".to_string()
        );
        assert_eq!(view.open_label, "Open hello-world".to_string());
    }

    #[test]
    fn published_posts_ready_typed_view_maps_items_and_empty_state() {
        let items_view =
            published_posts_ready_typed_view(vec!["a".to_string()], "empty".to_string());
        match items_view {
            PublishedPostsReadyView::Items(items) => assert_eq!(items, vec!["a".to_string()]),
            PublishedPostsReadyView::Empty(_) => panic!("expected items variant"),
        }

        let empty_view = published_posts_ready_typed_view::<String>(vec![], "empty".to_string());
        match empty_view {
            PublishedPostsReadyView::Items(_) => panic!("expected empty variant"),
            PublishedPostsReadyView::Empty(empty_state) => {
                assert_eq!(empty_state.message, "empty".to_string())
            }
        }
    }

    #[test]
    fn selected_post_meta_view_builds_meta_payload() {
        let view = selected_post_meta_view(
            "slug",
            "hello-world",
            "locale",
            "en",
            "published",
            "2026-01-01T00:00:00Z",
        );
        assert_eq!(view.slug_meta, "slug: hello-world".to_string());
        assert_eq!(view.locale_meta, "locale: en".to_string());
        assert_eq!(
            view.published_meta,
            "published: 2026-01-01T00:00:00Z".to_string()
        );
        assert_eq!(view.separator, "·");
    }

    #[test]
    fn selected_post_tags_view_maps_non_empty_tags() {
        assert!(selected_post_tags_view(vec![]).is_none());
        let view = selected_post_tags_view(vec!["news".to_string(), "release".to_string()])
            .expect("expected tags view");
        assert_eq!(view.items, vec!["news".to_string(), "release".to_string()]);
    }

    #[test]
    fn selected_post_content_view_returns_excerpt_and_body() {
        let view = selected_post_content_view(
            "No excerpt yet.".to_string(),
            "No body content yet.".to_string(),
        );
        assert_eq!(view.excerpt, "No excerpt yet.".to_string());
        assert_eq!(view.body, "No body content yet.".to_string());
    }

    #[test]
    fn selected_post_status_view_returns_status_and_unknown_label() {
        let view = selected_post_status_view("published".to_string(), "unknown".to_string());
        assert_eq!(view.status, "published".to_string());
        assert_eq!(view.unknown_label, "unknown".to_string());
    }

    #[test]
    fn selected_post_header_view_groups_title_meta_and_status() {
        let header = selected_post_header_view(
            "Hello".to_string(),
            selected_post_meta_view(
                "slug",
                "hello-world",
                "locale",
                "en",
                "published",
                "2026-01-01T00:00:00Z",
            ),
            selected_post_status_view("published".to_string(), "unknown".to_string()),
        );
        assert_eq!(header.title, "Hello".to_string());
        assert_eq!(header.meta.slug_meta, "slug: hello-world".to_string());
        assert_eq!(header.status.status, "published".to_string());
    }

    #[test]
    fn published_post_card_view_maps_tuple_to_typed_payload() {
        let view = published_post_card_view(PublishedPostCardInput {
            slug: None,
            missing_slug_fallback: "missing-slug",
            excerpt: None,
            excerpt_fallback: "No excerpt yet.",
            module_route_base: "/store/modules/blog",
            open_label: "Open",
            locale_label: "locale",
            effective_locale: "en",
            status: "published".to_string(),
        });
        assert_eq!(view.status, "published".to_string());
        assert_eq!(view.excerpt, "No excerpt yet.".to_string());
        assert_eq!(
            view.href,
            "/store/modules/blog?slug=missing-slug".to_string()
        );
        assert_eq!(view.open_label, "Open missing-slug".to_string());
        assert_eq!(view.locale_meta, "locale: en".to_string());
    }

    #[test]
    fn error_and_href_helpers_format_expected_values() {
        assert_eq!(
            error_with_context("Failed to load", "timeout"),
            "Failed to load: timeout"
        );
        assert_eq!(
            module_href("/store/modules/blog", "hello-world"),
            "/store/modules/blog?slug=hello-world"
        );
        assert_eq!(
            fallback_slug(None, "missing-slug"),
            "missing-slug".to_string()
        );
        assert_eq!(
            fallback_excerpt(None, "No excerpt yet."),
            "No excerpt yet.".to_string()
        );
        assert_eq!(
            selected_slug_or_default(None, "latest"),
            "latest".to_string()
        );
        assert_eq!(route_segment_or_default(None, "blog"), "blog".to_string());
        assert_eq!(
            post_link("/store/modules/blog", "hello-world", "Open"),
            (
                "/store/modules/blog?slug=hello-world".to_string(),
                "Open hello-world".to_string()
            )
        );
        assert_eq!(
            post_meta_pairs(
                "slug",
                "hello-world",
                "locale",
                "en",
                "published",
                "2026-01-01T00:00:00Z",
            ),
            [
                "slug: hello-world".to_string(),
                "locale: en".to_string(),
                "published: 2026-01-01T00:00:00Z".to_string(),
            ]
        );
        assert_eq!(meta_separator(), "·");
        assert_eq!(
            selected_post_meta_row(
                "slug",
                "hello-world",
                "locale",
                "en",
                "published",
                "2026-01-01T00:00:00Z",
            ),
            (
                "slug: hello-world".to_string(),
                "locale: en".to_string(),
                "published: 2026-01-01T00:00:00Z".to_string(),
                "·",
            )
        );
        assert_eq!(
            selected_post_fallback_fields(
                None,
                "missing-slug",
                None,
                "No excerpt yet.",
                None,
                "Unscheduled",
            ),
            (
                "missing-slug".to_string(),
                "No excerpt yet.".to_string(),
                "Unscheduled".to_string(),
            )
        );
        assert_eq!(
            list_post_summary(
                None,
                "missing-slug",
                None,
                "No excerpt yet.",
                "/store/modules/blog",
                "Open",
            ),
            (
                "No excerpt yet.".to_string(),
                "/store/modules/blog?slug=missing-slug".to_string(),
                "Open missing-slug".to_string(),
            )
        );
        assert_eq!(
            list_post_locale_meta("locale", "en"),
            "locale: en".to_string()
        );
        let card = published_post_card_view(PublishedPostCardInput {
            slug: None,
            missing_slug_fallback: "missing-slug",
            excerpt: None,
            excerpt_fallback: "No excerpt yet.",
            module_route_base: "/store/modules/blog",
            open_label: "Open",
            locale_label: "locale",
            effective_locale: "en",
            status: "published".to_string(),
        });
        assert_eq!(card.status, "published");
        assert_eq!(card.excerpt, "No excerpt yet.");
        assert_eq!(card.href, "/store/modules/blog?slug=missing-slug");
        assert_eq!(card.open_label, "Open missing-slug");
        assert_eq!(card.locale_meta, "locale: en");
    }

    #[test]
    fn status_badge_css_maps_known_statuses() {
        assert_eq!(
            status_badge_css("published"),
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        );
        assert_eq!(
            status_badge_css("archived"),
            "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
        );
        assert_eq!(
            status_badge_css("draft"),
            "inline-flex rounded-full border border-primary/30 bg-primary/10 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-primary"
        );
        assert_eq!(
            status_badge_css("  Published  "),
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        );
    }

    #[test]
    fn status_label_trims_and_falls_back() {
        assert_eq!(
            status_label("  published  ", "unknown"),
            "published".to_string()
        );
        assert_eq!(status_label("   ", "unknown"), "unknown".to_string());
    }

    #[test]
    fn has_items_detects_non_empty_collection() {
        assert!(!has_items::<u8>(&[]));
        assert!(has_items(&[1_u8]));
    }

    #[test]
    fn status_presentation_returns_label_and_css() {
        let (label, css) = status_presentation("  published  ", "unknown");
        assert_eq!(label, "published".to_string());
        assert_eq!(
            css,
            "inline-flex rounded-full border border-emerald-300/50 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-emerald-800 dark:border-emerald-700/40 dark:bg-emerald-900/25 dark:text-emerald-300"
        );
    }

    #[test]
    fn status_badge_view_maps_owned_status() {
        let (label, css) = status_badge_view(" archived ".to_string(), "unknown");
        assert_eq!(label, "archived".to_string());
        assert_eq!(
            css,
            "inline-flex rounded-full border border-border bg-muted px-2.5 py-0.5 text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground"
        );
    }

    #[test]
    fn selected_post_tag_items_filters_empty_vectors() {
        assert_eq!(selected_post_tag_items(vec![]), None);
        assert_eq!(
            selected_post_tag_items(vec!["news".to_string(), "release".to_string()]),
            Some(vec!["news".to_string(), "release".to_string()])
        );
    }

    #[test]
    fn published_posts_or_empty_message_maps_empty_and_non_empty() {
        assert_eq!(
            published_posts_or_empty_message(vec!["a".to_string()], "empty".to_string()),
            Ok(vec!["a".to_string()])
        );
        assert_eq!(
            published_posts_or_empty_message::<String>(vec![], "empty".to_string()),
            Err("empty".to_string())
        );
    }

    #[test]
    fn published_posts_view_state_maps_to_option_pair() {
        assert_eq!(
            published_posts_view_state(vec!["a".to_string()], "empty".to_string()),
            (Some(vec!["a".to_string()]), None)
        );
        assert_eq!(
            published_posts_view_state::<String>(vec![], "empty".to_string()),
            (None, Some("empty".to_string()))
        );
    }

    #[test]
    fn published_posts_items_or_default_returns_empty_for_none() {
        assert_eq!(
            published_posts_items_or_default::<String>(None),
            Vec::<String>::new()
        );
        assert_eq!(
            published_posts_items_or_default(Some(vec!["x".to_string()])),
            vec!["x".to_string()]
        );
    }

    #[test]
    fn published_posts_ready_items_maps_to_result() {
        assert_eq!(
            published_posts_ready_items(vec!["x".to_string()], "empty".to_string()),
            Ok(vec!["x".to_string()])
        );
        assert_eq!(
            published_posts_ready_items::<String>(vec![], "empty".to_string()),
            Err("empty".to_string())
        );
    }

    #[test]
    fn published_posts_empty_state_message_passthrough() {
        assert_eq!(
            published_posts_empty_state_message("No items".to_string()),
            "No items".to_string()
        );
    }

    #[test]
    fn published_posts_empty_state_view_wraps_message() {
        assert_eq!(
            published_posts_empty_state_view("No items".to_string()),
            ("No items".to_string(),)
        );
    }
