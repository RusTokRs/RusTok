from pathlib import Path


def patch(path: str, old: str, new: str) -> None:
    file = Path(path)
    content = file.read_text(encoding="utf-8")
    count = content.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old!r}")
    file.write_text(content.replace(old, new), encoding="utf-8")


patch(
    "crates/rustok-seo/README.md",
    "- runtime module: `rustok_seo::SeoModule`\n",
    "- runtime module: `rustok_seo::SeoModule`\n"
    "- application composition: `rustok_seo::SeoApplicationServices`, exposing focused "
    "`settings()`, `metadata()`, `routing()`, `redirects()`, `sitemaps()`, `bulk()`, and "
    "`operations()` services\n",
)

patch(
    "crates/rustok-seo/docs/README.md",
    "- REST handlers use narrow `SeoHttpRuntime` with explicit DB/event bus/runtime extensions handles; the route-state adapter is the only host composition boundary;\n",
    "- REST handlers use narrow `SeoHttpRuntime` with explicit DB/event bus/runtime extensions handles; the route-state adapter is the only host composition boundary;\n"
    "- public application calls enter through `SeoApplicationServices` and select a focused settings, metadata, routing, redirect, sitemap, bulk, or operations service; the broad transactional runtime remains crate-private;\n",
)

patch(
    "docs/modules/crates-registry.md",
    "| `rustok-seo` | Optional SEO module: explicit metadata overrides, canonical storefront read contract, manual redirects, sitemaps, robots, shared SEO capability contracts and cross-cutting admin infrastructure surface. HTTP handlers use narrow `SeoHttpRuntime` through module-owned Axum routes. | `SeoModule`, `SeoService`, `SeoHttpRuntime`, `SeoQuery`, `SeoMutation`, `controllers::*`, `dto::*`. | Duplicate SEO source of truth in storefront hosts, move canonical/redirect resolution to the adapter layer, make host-local metadata precedence, or consider `rustok-seo-admin` a long-term owner screen for other entity editors. |\n",
    "| `rustok-seo` | Optional SEO module: explicit metadata overrides, canonical storefront read contract, manual redirects, sitemaps, robots, shared SEO capability contracts and cross-cutting admin infrastructure surface. HTTP handlers use narrow `SeoHttpRuntime` through module-owned Axum routes. | `SeoModule`, `SeoApplicationServices`, focused settings/metadata/routing/redirect/sitemap/bulk/operations services, `SeoHttpRuntime`, `SeoQuery`, `SeoMutation`, `controllers::*`, `dto::*`. | Duplicate SEO source of truth in storefront hosts, expose the broad internal SEO runtime as a public facade, move canonical/redirect resolution to the adapter layer, make host-local metadata precedence, or consider `rustok-seo-admin` a long-term owner screen for other entity editors. |\n",
)

patch(
    "docs/roadmaps/seo-hardening-progress.md",
    "- [x] Split the broad `SeoService` facade into focused application services. (application services PR)\n",
    "- [x] Split the broad `SeoService` facade into focused application services. (#2067)\n",
)

patch(
    "docs/roadmaps/seo-hardening-progress.md",
    "PRs #2056, #2059, #2061, and #2064 continue the SEO hardening work without fresh test execution at the user's request.",
    "PRs #2056, #2059, #2061, #2064, and #2067 continue the SEO hardening work without fresh test execution at the user's request.",
)
