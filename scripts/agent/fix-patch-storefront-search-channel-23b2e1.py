from pathlib import Path

path = Path("scripts/agent/patch-storefront-search-channel-23b2e1.py")
text = path.read_text()

replacements = [
    (
        '''    count = after.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one replacement after anchor, found {count}\\n{old}"
        )
    target.write_text(before + after.replace(old, new, 1))''',
        '''    if old not in after:
        raise SystemExit(f"{path}: replacement missing after anchor\\n{old}")
    target.write_text(before + after.replace(old, new, 1))''',
        "replace_after helper",
    ),
    (
        '''replace_once(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "        use rustok_api::{HostRuntimeContext, TenantContext};\\n",
    "        use rustok_api::{HostRuntimeContext, RequestContext, TenantContext};\\n",
)''',
        '''replace_after(
    "crates/rustok-search/storefront/src/transport/native_server_adapter.rs",
    "async fn storefront_search_native(\\n",
    "        use rustok_api::{HostRuntimeContext, TenantContext};\\n",
    "        use rustok_api::{HostRuntimeContext, RequestContext, TenantContext};\\n",
)''',
        "native RequestContext target",
    ),
    (
        '''replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """cargo test -p rustok-search storefront_result_eligibility -- --nocapture\\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture\\n""",
    """cargo test -p rustok-search storefront_result_eligibility -- --nocapture\\ncargo test -p rustok-search storefront_channel_authority -- --nocapture\\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture\\n""",
)''',
        '''replace_after(
    "crates/rustok-forum/docs/implementation-plan.md",
    "### Delivered in `FORUM-23B2E1`\\n",
    """cargo test -p rustok-search storefront_result_eligibility -- --nocapture\\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture\\n""",
    """cargo test -p rustok-search storefront_result_eligibility -- --nocapture\\ncargo test -p rustok-search storefront_channel_authority -- --nocapture\\ncargo test -p rustok-search visible_forum_statuses_match_owner_eligibility -- --nocapture\\n""",
)''',
        "FORUM-23 test command target",
    ),
    (
        '''replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """node scripts/verify/verify-forum-search-result-eligibility.mjs\\ncargo check -p rustok-search --features graphql --all-targets\\n""",
    """node scripts/verify/verify-forum-search-result-eligibility.mjs\\nnode scripts/verify/verify-forum-search-trusted-channel-authority.mjs\\ncargo check -p rustok-search --features graphql --all-targets\\n""",
)''',
        '''replace_after(
    "crates/rustok-forum/docs/implementation-plan.md",
    "### Delivered in `FORUM-23B2E1`\\n",
    """node scripts/verify/verify-forum-search-result-eligibility.mjs\\ncargo check -p rustok-search --features graphql --all-targets\\n""",
    """node scripts/verify/verify-forum-search-result-eligibility.mjs\\nnode scripts/verify/verify-forum-search-trusted-channel-authority.mjs\\ncargo check -p rustok-search --features graphql --all-targets\\n""",
)''',
        "FORUM-23 verifier command target",
    ),
]

for old, new, label in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label} drifted: expected 1, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text)
