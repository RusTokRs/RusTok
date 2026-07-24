from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"expected one {label} match, found {count}")
    return text.replace(old, new, 1)


cargo_path = Path("crates/rustok-seo/Cargo.toml")
cargo = cargo_path.read_text()
for dependency in [
    "rustok-blog",
    "rustok-commerce-foundation",
    "rustok-forum",
    "rustok-pages",
    "rustok-product",
]:
    cargo = replace_once(
        cargo,
        f'  "dep:{dependency}",\n',
        "",
        f"server feature dependency {dependency}",
    )
    cargo = replace_once(
        cargo,
        f"{dependency} = {{ workspace = true, optional = true }}\n",
        "",
        f"optional dependency {dependency}",
    )

old_dev_dependencies = """[dev-dependencies]
tokio.workspace = true
rustok-outbox = { workspace = true, features = [\"test-transport-fallback\"] }
rustok-taxonomy.workspace = true
"""
new_dev_dependencies = """[dev-dependencies]
rustok-blog.workspace = true
rustok-forum.workspace = true
rustok-outbox = { workspace = true, features = [\"test-transport-fallback\"] }
rustok-pages.workspace = true
rustok-product.workspace = true
rustok-taxonomy.workspace = true
tokio.workspace = true
"""
cargo = replace_once(
    cargo,
    old_dev_dependencies,
    new_dev_dependencies,
    "dev dependency section",
)
cargo_path.write_text(cargo)

services_path = Path("crates/rustok-seo/src/services/services_base.rs")
services = services_path.read_text()
services = replace_once(
    services,
    "pub use rustok_blog::PostService;\npub use rustok_pages::PageService;\npub use rustok_product::CatalogService;\n\n",
    "",
    "legacy owner service reexports",
)
services_path.write_text(services)

roadmap_path = Path("docs/roadmaps/seo-hardening-progress.md")
roadmap = roadmap_path.read_text()
roadmap = replace_once(
    roadmap,
    "- [ ] Remove avoidable direct owner dependencies from the SEO crate.\n",
    "- [x] Remove avoidable direct owner dependencies from the SEO crate. (owner dependency PR)\n",
    "owner dependency roadmap item",
)
roadmap_path.write_text(roadmap)
