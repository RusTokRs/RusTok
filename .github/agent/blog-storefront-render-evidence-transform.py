from pathlib import Path
import json
import shutil

shutil.copyfile(
    ".github/agent/blog-storefront-render-evidence-verifier.mjs",
    "scripts/verify/verify-blog-storefront-boundary.mjs",
)
shutil.copyfile(
    ".github/agent/blog-storefront-render-evidence-selftest.mjs",
    "scripts/verify/verify-blog-storefront-boundary.test.mjs",
)

evidence = {
    "schema_version": 2,
    "owner": "rustok-blog",
    "boundary": "storefront-post-richtext-view",
    "status": "source_verified_no_compile",
    "scope": [
        "crates/rustok-blog/storefront/src/core.rs",
        "crates/rustok-blog/storefront/src/model.rs",
        "crates/rustok-blog/storefront/src/transport/graphql_adapter.rs",
        "crates/rustok-blog/storefront/src/transport/native_server_adapter.rs",
        "crates/rustok-blog/storefront/src/ui/leptos.rs",
    ],
    "contract": {
        "graphql_owner_view": True,
        "native_owner_view": True,
        "server_html_render": True,
        "plain_text_fallback": True,
        "legacy_body_transport": False,
        "legacy_body_format_transport": False,
        "local_format_renderer": False,
        "legacy_summarizer_removed": True,
    },
    "canonical_contract": {
        "read": "rustok_api::RichTextView",
        "html": "server-derived",
        "plain_text": "server-derived",
    },
    "render_contract": {
        "component": "SelectedPostCard",
        "html_sink": "inner_html=content.html",
        "fallback_sink": "selected_post_content.body",
        "forbidden_storefront_markers": [
            "RichTextDocument",
            "content.document",
            "pulldown_cmark",
            "comrak::",
            "markdown_to_html",
            "render_richtext",
            "render_document",
        ],
    },
    "guardrail": "scripts/verify/verify-blog-storefront-boundary.mjs",
    "guardrail_test": "scripts/verify/verify-blog-storefront-boundary.test.mjs",
    "validation": {
        "tests_run": False,
        "verifier_run": False,
        "cargo_run": False,
        "format_run": False,
        "workflow_checks_run": False,
        "ci_run": False,
    },
    "remaining": [
        "execute compile, migration, transport parity, and browser evidence",
    ],
}
Path(
    "crates/rustok-blog/contracts/evidence/blog-storefront-richtext-view.json"
).write_text(json.dumps(evidence, indent=2) + "\n")

plan_path = Path("crates/rustok-blog/docs/implementation-plan.md")
plan = plan_path.read_text()
old_paragraph = """The Blog storefront selected-post path now consumes the owner read projection
across both transports. GraphQL requests `content { document html }` plus
`contentPlainText`; native SSR maps `PostResponse.content` and
`content_plain_text`; Leptos renders only server-rendered `RichTextView` HTML and
uses server-derived plain text when the projection is absent. The storefront DTO
and active UI path expose no legacy body or format field."""
new_paragraph = """The Blog storefront selected-post path now consumes the owner read projection
across both transports. GraphQL requests `content { document html }` plus
`contentPlainText`; native SSR maps `PostResponse.content` and
`content_plain_text`; Leptos renders only server-rendered `RichTextView` HTML and
uses server-derived plain text when the projection is absent. Evidence schema v2
locks the selected-post component to exactly one `content.html` sink, rejects
`RichTextDocument`/`content.document` consumption and local Markdown/richtext
renderers throughout the storefront package, and fail-closes every true/false
transport contract. The storefront DTO and active UI path expose no legacy body
or format field."""
if old_paragraph not in plan:
    raise SystemExit("storefront current-state paragraph not found")
plan = plan.replace(old_paragraph, new_paragraph, 1)

old_slice = """33. Bound the GraphQL target-only richtext verifier to evidence schema v3, exact
    create/update conversion scopes, direct canonical content mapping, and both
    conversion regression markers, with focused negative fixtures for each drift.
"""
new_slice = old_slice + """34. Bound the storefront richtext verifier to evidence schema v2, the exact
    `SelectedPostCard` render scope, one owner `content.html` sink, server-derived
    fallback text, and explicit rejection of local document/Markdown renderers.
"""
if old_slice not in plan:
    raise SystemExit("slice 33 anchor not found")
plan = plan.replace(old_slice, new_slice, 1)
plan_path.write_text(plan)
