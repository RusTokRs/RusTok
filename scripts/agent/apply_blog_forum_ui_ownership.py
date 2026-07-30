import json
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(
            f"{path}: expected one replacement, found {count}: {old[:100]!r}"
        )
    target.write_text(text.replace(old, new, 1))


package_path = Path("package.json")
package = json.loads(package_path.read_text())
scripts = package["scripts"]
scripts["verify:blog:forum-ui-ownership"] = (
    "node scripts/verify/verify-blog-forum-ui-ownership.mjs"
)
old_fba = scripts["verify:blog:fba"]
marker = "npm run verify:blog:richtext-offline-backfill && "
if marker not in old_fba or "verify:blog:forum-ui-ownership" in old_fba:
    raise RuntimeError("package.json Blog FBA command drift")
scripts["verify:blog:fba"] = old_fba.replace(
    marker,
    marker + "npm run verify:blog:forum-ui-ownership && ",
    1,
)
package_path.write_text(json.dumps(package, indent=2) + "\n")

replace_once(
    "crates/rustok-blog/docs/implementation-plan.md",
    """reindex; those remain explicit operator steps.

The Blog storefront selected-post path now consumes the owner read projection""",
    """reindex; those remain explicit operator steps.

The Next admin Forum reply composer is no longer owned by Blog. Forum navigation,
GraphQL helpers, the reply editor, and its contained `rt_json_v1` compatibility
adapter now live under `apps/next-admin/packages/forum/src`; the host registers
that package independently. Blog and Forum consume the same thin shared React
lifecycle adapter at `apps/next-admin/src/shared/ui/rich-text-editor.tsx`, while
profile selection remains owner-specific (`article` versus `discussion`).

The Blog storefront selected-post path now consumes the owner read projection""",
)
replace_once(
    "crates/rustok-blog/docs/implementation-plan.md",
    """- Blog article offline backfill: `executable_no_run`; dry-run preflight,
  content-free reporting, explicit apply/Markdown acknowledgement, orphan
  detection, stable cursoring, optimistic writes, and post-apply verification
  are implemented.
- Comments thread write invariants:""",
    """- Blog article offline backfill: `executable_no_run`; dry-run preflight,
  content-free reporting, explicit apply/Markdown acknowledgement, orphan
  detection, stable cursoring, optimistic writes, and post-apply verification
  are implemented.
- Next admin Forum UI ownership: `source_verified_no_compile`; Blog no longer
  registers or exports Forum navigation, GraphQL helpers, reply UI, or legacy
  format adapters, and both owners use the shared richtext lifecycle adapter.
- Comments thread write invariants:""",
)
replace_once(
    "crates/rustok-blog/docs/implementation-plan.md",
    """- `crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json`
- `crates/rustok-blog/docs/richtext-cutover-inventory.md`""",
    """- `crates/rustok-blog/contracts/evidence/blog-richtext-offline-backfill.json`
- `crates/rustok-blog/contracts/evidence/blog-forum-ui-ownership.json`
- `crates/rustok-blog/docs/richtext-cutover-inventory.md`""",
)
replace_once(
    "crates/rustok-blog/docs/implementation-plan.md",
    """- `scripts/verify/verify-blog-richtext-offline-backfill.mjs`
- `scripts/verify/verify-blog-fba.mjs`""",
    """- `scripts/verify/verify-blog-richtext-offline-backfill.mjs`
- `scripts/verify/verify-blog-forum-ui-ownership.mjs`
- `scripts/verify/verify-blog-fba.mjs`""",
)
replace_once(
    "crates/rustok-blog/docs/implementation-plan.md",
    """24. Added the owner-specific Blog article offline backfill: dry-run-first scanning
    of current owner tables, content-free NDJSON reporting, explicit apply and
    Markdown acknowledgement, fail-closed format/profile validation, optimistic
    batch writes, and post-apply verification.

## Next results""",
    """24. Added the owner-specific Blog article offline backfill: dry-run-first scanning
    of current owner tables, content-free NDJSON reporting, explicit apply and
    Markdown acknowledgement, fail-closed format/profile validation, optimistic
    batch writes, and post-apply verification.
25. Removed Forum Next admin ownership from the Blog package, introduced the
    Forum-owned package registration/navigation/API/editor boundary, and moved
    the reusable React richtext lifecycle adapter to the host shared UI layer.

## Next results""",
)
replace_once(
    "crates/rustok-blog/docs/implementation-plan.md",
    """- `npm run verify:blog:richtext-offline-backfill`
- `cargo run -p rustok-blog --bin blog_article_richtext_backfill -- --help`""",
    """- `npm run verify:blog:richtext-offline-backfill`
- `npm run verify:blog:forum-ui-ownership`
- `cargo run -p rustok-blog --bin blog_article_richtext_backfill -- --help`""",
)

replace_once(
    "docs/modules/rich-text-implementation-plan.md",
    """headers; Firefox/WebKit evidence and wiring the first owner Leptos form remain
required before Phase 2 is marked complete.""",
    """headers. The Blog owner Leptos form now mounts the same framed lifecycle
adapter; Firefox/WebKit evidence remains required before Phase 2 is marked
complete.""",
)
replace_once(
    "docs/modules/rich-text-implementation-plan.md",
    """- the Blog package also owns a Forum reply editor and Forum API helpers, which
  violates module UI ownership;""",
    """- resolved 2026-07-30: the Forum Next admin package owns its navigation,
  GraphQL helpers, reply editor, and contained legacy format adapter; Blog and
  Forum share only the neutral React richtext lifecycle adapter;""",
)
replace_once(
    "docs/modules/rich-text-implementation-plan.md",
    """- Leptos Blog and Forum forms are raw textareas. Their current adapters either
  omit `content_json`, have no native `#[server]` path, or retry failed writes
  through another protocol;
- storefronts do not have a real richtext read path; some surfaces display raw
  payload summaries;""",
    """- resolved for the Blog Leptos owner form: it mounts the framed `article`
  lifecycle adapter; Forum Leptos authoring still requires its owner cutover;
- resolved for Blog storefront reads: native and GraphQL paths consume the
  server-owned projection; Forum storefront richtext parity remains pending;""",
)

replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    "last_reviewed: 2026-07-28",
    "last_reviewed: 2026-07-30",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """`content_json` fields, or a Forum-local renderer.

## Verification""",
    """`content_json` fields, or a Forum-local renderer.

The Next admin Forum surface is module-owned under
`apps/next-admin/packages/forum/src`. It owns Forum navigation, topic/reply
GraphQL helpers, the reply composer, and the contained legacy reply-format
adapter. The host only registers and mounts the package; the reusable framed
React richtext lifecycle adapter remains neutral shared UI.

## Verification""",
)
replace_once(
    "crates/rustok-forum/docs/implementation-plan.md",
    """Run `npm run verify:forum:storefront-boundary`
(`scripts/verify/verify-forum-storefront-boundary.mjs`) after a storefront""",
    """Run `npm run verify:blog:forum-ui-ownership`
(`scripts/verify/verify-blog-forum-ui-ownership.mjs`) after changing the Next
admin Forum package or its former Blog ownership boundary.

Run `npm run verify:forum:storefront-boundary`
(`scripts/verify/verify-forum-storefront-boundary.mjs`) after a storefront""",
)

replace_once(
    "docs/modules/UI_PACKAGES_INDEX.md",
    """- `rustok-blog`: `apps/next-admin/packages/blog/`
- `rustok-product`:""",
    """- `rustok-blog`: `apps/next-admin/packages/blog/`
- `rustok-forum`: `apps/next-admin/packages/forum/`
- `rustok-product`:""",
)
