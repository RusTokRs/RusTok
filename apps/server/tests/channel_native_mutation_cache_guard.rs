#[test]
fn native_channel_mutations_invalidate_the_same_host_cache() {
    let wrapper = include_str!("../src/middleware/channel.rs");
    let base = include_str!("../src/middleware/channel_base.rs");

    assert!(wrapper.contains("#[path = \"channel_base.rs\"]"));
    assert!(wrapper.contains("base::resolve(State(ctx.clone()), req, next).await?"));
    assert!(wrapper.contains("response.status().is_success()"));
    assert!(wrapper.contains("base::invalidate_tenant_channel_cache(&ctx, tenant_id).await"));

    for endpoint in [
        "channel/create-channel",
        "channel/set-default",
        "channel/create-target",
        "channel/update-target",
        "channel/delete-target",
        "channel/bind-module",
        "channel/delete-module-binding",
        "channel/bind-oauth-app",
        "channel/delete-oauth-app-binding",
        "channel/create-resolution-policy-set",
        "channel/activate-resolution-policy-set",
        "channel/create-resolution-rule",
        "channel/update-resolution-rule",
        "channel/reorder-resolution-rules",
        "channel/delete-resolution-rule",
    ] {
        assert!(wrapper.contains(endpoint), "missing native mutation endpoint {endpoint}");
    }

    assert!(base.contains("pub async fn invalidate_tenant_channel_cache"));
    assert!(base.contains("pub async fn resolve("));
}
