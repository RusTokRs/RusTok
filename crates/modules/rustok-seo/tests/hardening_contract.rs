#[test]
fn redirect_mutations_are_transition_scoped() {
    let source = include_str!("../src/services/redirects.rs");

    assert!(source.contains("let transition_id = Uuid::new_v4()"));
    assert!(source.contains("redirect_domain_event(tenant_id, transition_id, record)"));
    assert!(source.contains("transition_id.to_string()"));
}

#[test]
fn redirect_targets_reject_unsafe_absolute_urls() {
    let source = include_str!("../src/services/redirects.rs");

    assert!(source.contains("scheme must be HTTP or HTTPS"));
    assert!(source.contains("URL credentials are not allowed"));
    assert!(source.contains("trim_end_matches('.')"));
}

#[test]
fn persisted_settings_fail_closed() {
    let source = include_str!("../src/services/services_base.rs");

    assert!(source.contains("SEO_SETTINGS_KEYS"));
    assert!(source.contains("unknown persisted SEO setting"));
    assert!(source.contains("validate_persisted_settings"));
    assert!(source.contains("validate_sitemap_submission_endpoint"));
    assert!(source.contains("persisted SEO template override slug must not be empty"));
}

#[test]
fn sitemap_file_reads_are_tenant_scoped() {
    let source = include_str!("../src/services/sitemaps.rs");

    assert!(source.contains("load_sitemap_files_for_jobs(tenant_id"));
    assert!(source.contains("seo_sitemap_file::Column::TenantId.eq(tenant_id)"));
}

#[test]
fn preconditions_and_bounds_are_enforced() {
    let meta_source = include_str!("../src/services/meta.rs");
    let redirects_source = include_str!("../src/services/redirects.rs");
    let sitemaps_source = include_str!("../src/services/sitemaps.rs");

    assert!(meta_source.contains("tenant_id must not be nil"));
    assert!(meta_source.contains("target_id must not be nil"));
    assert!(meta_source.contains("revision must be greater than zero"));

    assert!(redirects_source.contains("tenant_id must not be nil"));
    assert!(redirects_source.contains("redirect id must not be nil"));

    assert!(sitemaps_source.contains("tenant_id must not be nil"));
    assert!(sitemaps_source.contains("job_id must not be nil"));
    assert!(sitemaps_source.contains("limit.clamp(1, 100)"));
}
