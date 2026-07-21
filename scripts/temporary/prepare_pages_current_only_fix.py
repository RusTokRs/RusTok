from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text()
    if old not in text:
        raise RuntimeError(f"expected source fragment is missing from {path}")
    file.write_text(text.replace(old, new, 1))


replace_once(
    "crates/rustok-pages/src/services/page_builder_artifact.rs",
    """use rustok_page_builder::{
    ComponentRegistryManifest, LandingSectionSnapshot, PageBuilderMaterializedStaticLandingArtifact,
    PageBuilderPreviewRuntime, PageBuilderStaticLandingMaterializationIdentity, PageHead,
    StaticLandingArtifact, StaticLandingBuildIdentity, StaticLandingPage,
    compile_materialized_static_landing,
};
""",
    """use rustok_page_builder::dto::PageBuilderPreviewRuntime;
use rustok_page_builder::{
    ComponentRegistryManifest, LandingSectionSnapshot, PageBuilderMaterializedStaticLandingArtifact,
    PageBuilderStaticLandingMaterializationIdentity, PageHead, StaticLandingArtifact,
    StaticLandingBuildIdentity, StaticLandingPage, compile_materialized_static_landing,
};
""",
)

replace_once(
    "crates/rustok-pages/src/services/page/read.rs",
    "use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};",
    "use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};",
)

replace_once(
    "crates/rustok-pages/src/services/page/lifecycle.rs",
    '''            let digest = Sha256::digest(format!("{}\\0{}", body.format, body.content).as_bytes());
            (
                body.locale.clone(),
                format!("{}:{digest:x}", body.updated_at),
            )
''',
    '''            let digest = Sha256::digest(format!("{}\\0{}", body.format, body.content).as_bytes());
            (
                body.locale.clone(),
                format!("{}:{}", body.updated_at, encode_digest(&digest)),
            )
''',
)

replace_once(
    "crates/rustok-pages/src/services/page/lifecycle.rs",
    "\nfn format_body_revisions(revisions: &BodyRevisionSnapshot) -> String {",
    '''
fn encode_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

fn format_body_revisions(revisions: &BodyRevisionSnapshot) -> String {''',
)

seo_path = Path("crates/rustok-pages/src/seo_targets.rs")
seo = seo_path.read_text()
seo = seo.replace(
    '''use crate::{
    BlockPayload, ListPagesFilter, PageListItem, PageResponse, PageService, PageTranslationResponse,
};''',
    '''use crate::{
    ListPagesFilter, PageListItem, PageResponse, PageService, PageTranslationResponse,
};''',
    1,
)
start = seo.index("fn page_primary_image_descriptor(")
end_marker = "\n    None\n}\n"
end = seo.index(end_marker, start) + len(end_marker)
replacement = '''fn page_primary_image_descriptor(
    page: &PageResponse,
    fallback_alt: &str,
) -> Option<SeoTargetImageRecord> {
    let fallback_alt = normalize_image_text(Some(fallback_alt.to_string()));
    let document = page.body.as_ref()?.content_json.as_ref()?;
    find_image_descriptor(document, fallback_alt.as_deref())
}

fn find_image_descriptor(
    value: &serde_json::Value,
    fallback_alt: Option<&str>,
) -> Option<SeoTargetImageRecord> {
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .find_map(|item| find_image_descriptor(item, fallback_alt)),
        serde_json::Value::Object(object) => {
            let local_alt = ["alt", "caption", "title"]
                .iter()
                .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
                .and_then(|text| normalize_image_text(Some(text.to_string())))
                .or_else(|| fallback_alt.map(str::to_string));
            let image_component = object
                .get("type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|kind| matches!(kind, "image" | "hero" | "gallery"))
                || object
                    .get("tagName")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|tag| tag.eq_ignore_ascii_case("img"));
            let explicit_keys = [
                "background_image_url",
                "backgroundImage",
                "background-image",
                "image_url",
                "imageUrl",
            ];
            let image_url = explicit_keys
                .iter()
                .find_map(|key| object.get(*key).and_then(serde_json::Value::as_str))
                .or_else(|| {
                    image_component
                        .then(|| object.get("src").and_then(serde_json::Value::as_str))
                        .flatten()
                })
                .map(str::trim)
                .filter(|url| !url.is_empty());
            if let Some(url) = image_url {
                if let Some(record) = SeoTargetImageRecord::from_parts(
                    url.to_string(),
                    local_alt,
                    None,
                    None,
                    None,
                ) {
                    return Some(record);
                }
            }
            object
                .values()
                .find_map(|item| find_image_descriptor(item, fallback_alt))
        }
        _ => None,
    }
}
'''
seo_path.write_text(seo[:start] + replacement + seo[end:])
