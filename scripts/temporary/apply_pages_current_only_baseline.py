from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text()


def write(path: str, text: str) -> None:
    Path(path).write_text(text)


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if text.count(old) != 1:
        raise RuntimeError(f"{label}: expected exactly one match, found {text.count(old)}")
    return text.replace(old, new, 1)


# Remove legacy builder preparation helpers and fix the version guard.
path = "crates/rustok-pages/src/services/page/helpers.rs"
text = read(path)
text = replace_once(
    text,
    "use crate::services::PageBuilderArtifactService;\nuse crate::services::page_builder_artifact::CompiledLandingArtifact;\n",
    "",
    "obsolete helper imports",
)
start = text.index("pub(super) fn collect_builder_project_values(")
end = text.index("pub(super) fn collect_builder_sources(", start)
text = text[:start] + text[end:]
start = text.index("pub(super) fn compile_builder_sources(")
end = text.index("pub(super) fn enforce_expected_version", start)
text = text[:start] + text[end:]
old = '''pub(super) fn enforce_expected_version(expected: Option<i32>, actual: i32) -> PagesResult<()> {
    if let Some(expected_version) = expected {
        if expected_version != actual {
            return Err(PagesError::VersionConflict {
                expected_version,
                actual_version: actual,
            });
        }
    }
    Ok(())
}
'''
new = '''pub(super) fn enforce_expected_version(expected: Option<i32>, actual: i32) -> PagesResult<()> {
    if let Some(expected_version) = expected
        && expected_version != actual
    {
        return Err(PagesError::VersionConflict {
            expected_version,
            actual_version: actual,
        });
    }
    Ok(())
}
'''
text = replace_once(text, old, new, "version guard")
write(path, text)


# Keep non-builder lifecycle current-only and fix body revision hashing.
path = "crates/rustok-pages/src/services/page/lifecycle.rs"
text = read(path)
text = replace_once(
    text,
    "    FEATURE_BUILDER_PUBLISH_ENABLED, PagesError, PagesResult,\n",
    "    PagesError, PagesResult,\n",
    "publish feature import",
)
text = replace_once(
    text,
    "    is_builder_preview_enabled, is_builder_properties_enabled, is_builder_publish_enabled,\n    transition_event,\n",
    "    is_builder_preview_enabled, is_builder_properties_enabled, transition_event,\n",
    "publish helper import",
)
start = text.index("    pub(super) async fn ensure_builder_publish_enabled(")
end = text.index("    pub(super) async fn ensure_builder_enabled(", start)
text = text[:start] + text[end:]
old = '''            let digest = Sha256::digest(format!("{}\\0{}", body.format, body.content).as_bytes());
            (
                body.locale.clone(),
                format!("{}:{digest:x}", body.updated_at),
            )
'''
new = '''            let digest = Sha256::digest(format!("{}\\0{}", body.format, body.content).as_bytes());
            (
                body.locale.clone(),
                format!("{}:{}", body.updated_at, encode_digest(&digest)),
            )
'''
text = replace_once(text, old, new, "body revision digest")
marker = "\nfn format_body_revisions(revisions: &BodyRevisionSnapshot) -> String {"
helper = '''
fn encode_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}
'''
text = replace_once(text, marker, helper + marker, "digest helper insertion")
write(path, text)


# Remove the obsolete default-runtime compiler and stale binding helper/tests.
path = "crates/rustok-pages/src/services/page_builder_artifact.rs"
text = read(path)
old_import = '''use rustok_page_builder::{
    ComponentRegistryManifest, LandingSectionSnapshot, PageBuilderMaterializedStaticLandingArtifact,
    PageBuilderPreviewRuntime, PageBuilderStaticLandingMaterializationIdentity, PageHead,
    StaticLandingArtifact, StaticLandingBuildIdentity, StaticLandingPage,
    compile_materialized_static_landing,
};
'''
new_import = '''use rustok_page_builder::{
    ComponentRegistryManifest, LandingSectionSnapshot, PageBuilderMaterializedStaticLandingArtifact,
    PageBuilderStaticLandingMaterializationIdentity, PageHead, StaticLandingArtifact,
    StaticLandingBuildIdentity, StaticLandingPage,
};
'''
text = replace_once(text, old_import, new_import, "artifact imports")
start = text.index("    pub(crate) fn compile_source(")
end = text.index("    pub(crate) async fn stage_compiled_in_tx(", start)
text = text[:start] + text[end:]
start = text.index("    pub(crate) async fn clear_existing_body_binding_in_tx(")
end = text.index("    /// Loads the currently published artifact", start)
text = text[:start] + text[end:]
text = replace_once(
    text,
    '''fn artifact_compile_error(error: impl std::fmt::Display) -> PagesError {
    PagesError::validation(format!("Page Builder static artifact error: {error}"))
}

''',
    "",
    "artifact compile error helper",
)
start = text.index("    #[test]\n    fn compiler_produces_a_verified_single_page_materialization()")
end = text.index("    #[test]\n    fn unrestricted_artifact_is_visible_without_a_channel()", start)
text = text[:start] + text[end:]
write(path, text)


# Read SEO images from current Fly JSON, never from removed legacy block DTOs.
path = "crates/rustok-pages/src/seo_targets.rs"
text = read(path)
text = replace_once(
    text,
    '''use crate::{
    BlockPayload, ListPagesFilter, PageListItem, PageResponse, PageService, PageTranslationResponse,
};''',
    '''use crate::{
    ListPagesFilter, PageListItem, PageResponse, PageService, PageTranslationResponse,
};''',
    "SEO legacy import",
)
start = text.index("fn page_primary_image_descriptor(")
end = text.index("fn normalize_image_text(", start)
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
            if let Some(url) = image_url
                && let Some(record) =
                    SeoTargetImageRecord::from_parts(url.to_string(), local_alt, None, None, None)
            {
                return Some(record);
            }
            object
                .values()
                .find_map(|item| find_image_descriptor(item, fallback_alt))
        }
        _ => None,
    }
}

'''
text = text[:start] + replacement + text[end:]
write(path, text)


# Remove an unused SeaORM trait import.
path = "crates/rustok-pages/src/services/page/read.rs"
text = read(path)
text = replace_once(
    text,
    "use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect};",
    "use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};",
    "read QuerySelect import",
)
write(path, text)


# Apply the two reviewed publish Clippy corrections without changing behavior.
path = "crates/rustok-pages/src/services/page/reviewed_publish.rs"
text = read(path)
text = replace_once(
    text,
    "    if &selected.context != &reviewed.context {",
    "    if selected.context != reviewed.context {",
    "reviewed context comparison",
)
text = replace_once(
    text,
    "        published_at: Set(timestamp.clone()),",
    "        published_at: Set(timestamp),",
    "publish timestamp copy",
)
write(path, text)
