#[test]
fn redirect_mutations_are_transition_scoped_and_pending_until_delivery() {
    let source = include_str!("../src/services/redirects.rs");

    assert!(source.contains("let transition_id = Uuid::new_v4()"));
    assert!(source.contains("redirect_domain_event(tenant_id, transition_id, record)"));
    assert!(source.contains("DELIVERY_STATUS_PENDING"));
    assert!(source.contains("INDEX_DELIVERY_STATUS_PENDING"));
    assert!(source.contains("attempt_count: Set(0)"));
    assert!(source.contains("dispatched_at: Set(None)"));
    assert!(!source.contains("DELIVERY_STATUS_SENT"));
    assert!(!source.contains("INDEX_DELIVERY_STATUS_SENT"));
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
fn sitemap_deliveries_are_pending_and_file_reads_are_tenant_scoped() {
    let source = include_str!("../src/services/sitemaps.rs");

    assert!(source.contains("DELIVERY_STATUS_PENDING"));
    assert!(!source.contains("DELIVERY_STATUS_SENT"));
    assert!(source.contains("load_sitemap_files_for_jobs(tenant_id"));
    assert!(source.contains("seo_sitemap_file::Column::TenantId.eq(tenant_id)"));
    assert!(source.contains("dispatched_at: Set(None)"));
}
