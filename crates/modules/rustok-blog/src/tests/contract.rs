use rustok_core::module::RusToKModule;

#[test]
fn crate_api_defines_minimal_contract_sections() {
    let api = include_str!("../../CRATE_API.md");
    for marker in [
        "## Platform Capability Bindings",
        "## Domain Collaborations",
        "## Minimum Contract Set",
        "### Input DTOs/Commands",
        "### Domain Invariants",
        "### Events / Outbox Side Effects",
        "### Errors / Failure Codes",
    ] {
        assert!(
            api.contains(marker),
            "CRATE_API.md must contain section: {marker}"
        );
    }
}

/// The extension-point traits (`SeoTargetProvider::slug`, `ReactionSubjectProvider::source`)
/// take an infallible return type, so the adapters construct their key from a
/// compile-time constant. These tests are that constant's guard: a drifted literal
/// fails here instead of panicking on the request that needed the key.
#[test]
fn builtin_seo_slug_constants_satisfy_the_owner_grammar() {
    let slug = rustok_seo_targets::SeoTargetSlug::new(rustok_seo_targets::builtin_slug::BLOG_POST);
    assert!(
        slug.is_ok(),
        "the Blog SEO target slug must satisfy the SEO slug grammar: {:?}",
        slug.err()
    );
    assert_eq!(
        slug.expect("checked above").as_str(),
        crate::services::BLOG_POST_TARGET_TYPE,
        "the SEO target slug and the Blog target kind are the same identity"
    );
}

#[test]
fn blog_reaction_key_constants_satisfy_the_owner_grammar() {
    assert!(
        rustok_reactions::ReactionSourceSlug::new(crate::BLOG_REACTION_SOURCE).is_ok(),
        "BLOG_REACTION_SOURCE must satisfy the Reactions source-slug grammar"
    );
    assert!(
        rustok_reactions::ReactionSubjectKind::new(crate::BLOG_POST_REACTION_KIND).is_ok(),
        "BLOG_POST_REACTION_KIND must satisfy the Reactions subject-kind grammar"
    );
    assert!(
        rustok_reactions::ReactionKey::new(crate::BLOG_REACTION_V1_KEY).is_ok(),
        "BLOG_REACTION_V1_KEY must satisfy the Reactions key grammar"
    );
}

/// The GraphQL sort surface maps onto the owner sort keys, including the defaults the
/// resolver applies when a client sends no sort.
#[test]
fn graphql_sort_enums_map_onto_the_owner_sort_keys() {
    use crate::graphql::{GqlPostSortField, GqlPostSortOrder};

    assert_eq!(
        crate::PostSortField::from(GqlPostSortField::PublishedAt),
        crate::PostSortField::PublishedAt
    );
    assert_eq!(
        crate::PostSortField::from(GqlPostSortField::UpdatedAt),
        crate::PostSortField::UpdatedAt
    );
    assert_eq!(
        crate::PostSortField::from(GqlPostSortField::CreatedAt),
        crate::PostSortField::CreatedAt
    );
    assert_eq!(
        crate::PostSortOrder::from(GqlPostSortOrder::Asc),
        crate::PostSortOrder::Asc
    );
    assert_eq!(
        crate::PostSortOrder::from(GqlPostSortOrder::Desc),
        crate::PostSortOrder::Desc
    );

    let default_sort = crate::PostSortField::default();
    let default_order = crate::PostSortOrder::default();
    assert_eq!(default_sort, crate::PostSortField::CreatedAt);
    assert_eq!(default_order, crate::PostSortOrder::Desc);

    let query = crate::PostListQuery {
        sort_by: Some(default_sort),
        sort_order: Some(default_order),
        ..Default::default()
    };
    assert_eq!(query.sort(), (default_sort, default_order));
}

#[test]
fn module_metadata_matches_contract() {
    let module = crate::BlogModule;
    assert_eq!(module.slug(), "blog");
    assert_eq!(module.name(), "Blog");
    assert_eq!(
        module.dependencies(),
        &["content", "taxonomy", "outbox", "channel"]
    );
}
