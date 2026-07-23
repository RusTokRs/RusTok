use crate::builder::PagesBuilderSaveSnapshot;
use crate::contributions::{
    PAGES_METADATA_COMPONENT_TYPE, PAGES_METADATA_CONTRIBUTION_ID,
    PAGES_METADATA_PROPERTY_EDITOR_ID, PAGES_OWNER_PROVIDER, pages_metadata_property_schema,
};
use crate::core;
use crate::model::{PageDetail, PageMutationResult};
use crate::transport;
use rustok_page_builder_admin::{
    ConsumerPropertyEditorError, ConsumerPropertyEditorPort, ConsumerPropertyEditorRuntime,
    ConsumerPropertyEditorSnapshot, ConsumerPropertyLoadFuture, ConsumerPropertySaveFuture,
    ConsumerPropertySaveReceipt, SaveConsumerPropertiesInput,
};
use std::collections::BTreeMap;
use std::sync::Arc;

const PAGE_METADATA_REVISION_CONFLICT: &str = "REVISION_CONFLICT";

type SnapshotProvider = Arc<dyn Fn() -> PagesBuilderSaveSnapshot + Send + Sync>;
type SavedHandler = Arc<dyn Fn(PageMutationResult) + Send + Sync>;

pub fn pages_metadata_property_runtime(
    snapshot: impl Fn() -> PagesBuilderSaveSnapshot + Send + Sync + 'static,
    on_saved: impl Fn(PageMutationResult) + Send + Sync + 'static,
) -> Arc<ConsumerPropertyEditorRuntime> {
    let schema = pages_metadata_property_schema();
    Arc::new(ConsumerPropertyEditorRuntime::new(
        PAGES_METADATA_CONTRIBUTION_ID,
        PAGES_METADATA_PROPERTY_EDITOR_ID,
        PAGES_OWNER_PROVIDER,
        PAGES_METADATA_COMPONENT_TYPE,
        schema.clone(),
        Arc::new(PagesMetadataPropertyPort {
            snapshot: Arc::new(snapshot),
            on_saved: Arc::new(on_saved),
            schema,
        }),
    ))
}

struct PagesMetadataPropertyPort {
    snapshot: SnapshotProvider,
    on_saved: SavedHandler,
    schema: rustok_page_builder_admin::ConsumerPropertyEditorSchema,
}

impl ConsumerPropertyEditorPort for PagesMetadataPropertyPort {
    fn load(&self) -> ConsumerPropertyLoadFuture {
        let snapshot = (self.snapshot)();
        let schema = self.schema.clone();
        Box::pin(async move {
            let page = fetch_expected_page(&snapshot).await?;
            metadata_snapshot(&schema, &page, &snapshot.default_locale)
        })
    }

    fn save(&self, input: SaveConsumerPropertiesInput) -> ConsumerPropertySaveFuture {
        let snapshot = (self.snapshot)();
        let schema = self.schema.clone();
        let on_saved = Arc::clone(&self.on_saved);
        Box::pin(async move {
            if input.contribution_id != PAGES_METADATA_CONTRIBUTION_ID
                || input.property_editor_id != PAGES_METADATA_PROPERTY_EDITOR_ID
            {
                return Err(ConsumerPropertyEditorError::contract(
                    "Pages metadata save does not match the registered contribution",
                ));
            }
            schema.validate_values(&input.values)?;
            let expected_version =
                expected_metadata_version(&snapshot.page_id, &input.expected_revision)?;
            let current = fetch_expected_page(&snapshot).await?;
            if current.version != expected_version {
                return Err(metadata_revision_conflict(expected_version, current.version));
            }

            let locale = page_locale(&current, &snapshot.default_locale);
            let title = required_value(&input.values, "title")?;
            let slug = required_value(&input.values, "slug")?;
            let meta_title = optional_value(&input.values, "meta_title")?;
            let meta_description = optional_value(&input.values, "meta_description")?;
            let template = optional_value(&input.values, "template")?;
            let channel_slugs = core::parse_channel_slugs(value(&input.values, "channel_slugs")?);

            let page = transport::patch_page_metadata(
                snapshot.token,
                snapshot.tenant_slug,
                snapshot.page_id.clone(),
                expected_version,
                locale,
                title,
                slug,
                meta_title,
                meta_description,
                template,
                channel_slugs,
            )
            .await
            .map_err(|error| ConsumerPropertyEditorError::save(error.to_string()))?;
            if page.id != snapshot.page_id {
                return Err(ConsumerPropertyEditorError::save(format!(
                    "Pages metadata save returned page `{}` for `{}`",
                    page.id, snapshot.page_id
                )));
            }
            if page.version <= expected_version {
                return Err(ConsumerPropertyEditorError::save(format!(
                    "Pages metadata save returned non-advancing version {}",
                    page.version
                )));
            }

            let receipt_values = metadata_values(&page);
            schema.validate_values(&receipt_values)?;
            let receipt = ConsumerPropertySaveReceipt {
                contribution_id: PAGES_METADATA_CONTRIBUTION_ID.to_string(),
                property_editor_id: PAGES_METADATA_PROPERTY_EDITOR_ID.to_string(),
                revision: metadata_revision(&page.id, page.version),
                values: receipt_values,
            };
            on_saved(PageMutationResult::from(&page));
            Ok(receipt)
        })
    }
}

async fn fetch_expected_page(
    snapshot: &PagesBuilderSaveSnapshot,
) -> Result<PageDetail, ConsumerPropertyEditorError> {
    if snapshot.page_id.trim().is_empty() {
        return Err(ConsumerPropertyEditorError::unavailable(
            "Pages metadata properties require a selected page",
        ));
    }
    let page = transport::fetch_page(
        snapshot.token.clone(),
        snapshot.tenant_slug.clone(),
        snapshot.page_id.clone(),
    )
    .await
    .map_err(|error| ConsumerPropertyEditorError::unavailable(error.to_string()))?
    .ok_or_else(|| ConsumerPropertyEditorError::unavailable("Selected page was not found"))?;
    if page.id != snapshot.page_id {
        return Err(ConsumerPropertyEditorError::unavailable(format!(
            "Pages metadata load returned page `{}` for `{}`",
            page.id, snapshot.page_id
        )));
    }
    Ok(page)
}

fn metadata_snapshot(
    schema: &rustok_page_builder_admin::ConsumerPropertyEditorSchema,
    page: &PageDetail,
    default_locale: &str,
) -> Result<ConsumerPropertyEditorSnapshot, ConsumerPropertyEditorError> {
    let values = metadata_values(page);
    schema.validate_values(&values)?;
    Ok(ConsumerPropertyEditorSnapshot {
        revision: metadata_revision(&page.id, page.version),
        scope_label: format!("{} · {}", page_locale(page, default_locale), page.id),
        values,
    })
}

fn metadata_values(page: &PageDetail) -> BTreeMap<String, String> {
    let translation = page.translation.as_ref();
    BTreeMap::from([
        (
            "title".to_string(),
            translation
                .and_then(|translation| translation.title.clone())
                .unwrap_or_default(),
        ),
        (
            "slug".to_string(),
            translation
                .and_then(|translation| translation.slug.clone())
                .unwrap_or_default(),
        ),
        (
            "meta_title".to_string(),
            translation
                .and_then(|translation| translation.meta_title.clone())
                .unwrap_or_default(),
        ),
        (
            "meta_description".to_string(),
            translation
                .and_then(|translation| translation.meta_description.clone())
                .unwrap_or_default(),
        ),
        ("template".to_string(), page.template.clone()),
        (
            "channel_slugs".to_string(),
            page.channel_slugs.join(", "),
        ),
    ])
}

fn page_locale(page: &PageDetail, default_locale: &str) -> String {
    page.translation
        .as_ref()
        .map(|translation| translation.locale.clone())
        .or_else(|| page.body.as_ref().map(|body| body.locale.clone()))
        .unwrap_or_else(|| default_locale.to_string())
}

fn metadata_revision(page_id: &str, version: i32) -> String {
    format!("pages:{page_id}:metadata:v{version}")
}

fn expected_metadata_version(
    page_id: &str,
    revision: &str,
) -> Result<i32, ConsumerPropertyEditorError> {
    let prefix = format!("pages:{page_id}:metadata:v");
    let version = revision
        .strip_prefix(&prefix)
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|version| *version > 0)
        .ok_or_else(|| {
            ConsumerPropertyEditorError::contract(
                "Pages metadata revision does not match the selected page",
            )
        })?;
    Ok(version)
}

fn metadata_revision_conflict(expected: i32, actual: i32) -> ConsumerPropertyEditorError {
    ConsumerPropertyEditorError::with_stable_code(
        format!("Pages metadata version changed from {expected} to {actual}; reload and retry"),
        PAGE_METADATA_REVISION_CONFLICT,
    )
}

fn value<'a>(
    values: &'a BTreeMap<String, String>,
    field: &str,
) -> Result<&'a str, ConsumerPropertyEditorError> {
    values
        .get(field)
        .map(String::as_str)
        .ok_or_else(|| ConsumerPropertyEditorError::contract(format!("missing `{field}` value")))
}

fn required_value(
    values: &BTreeMap<String, String>,
    field: &str,
) -> Result<String, ConsumerPropertyEditorError> {
    let value = value(values, field)?.trim();
    if value.is_empty() {
        Err(ConsumerPropertyEditorError::contract(format!(
            "`{field}` is required"
        )))
    } else {
        Ok(value.to_string())
    }
}

fn optional_value(
    values: &BTreeMap<String, String>,
    field: &str,
) -> Result<Option<String>, ConsumerPropertyEditorError> {
    Ok(core::optional_ui_text(value(values, field)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::PageTranslation;

    fn page(version: i32) -> PageDetail {
        PageDetail {
            id: "page-1".to_string(),
            version,
            status: "draft".to_string(),
            template: "default".to_string(),
            updated_at: "2026-07-23T00:00:00Z".to_string(),
            available_locales: vec!["en".to_string()],
            channel_slugs: vec!["web".to_string()],
            translation: Some(PageTranslation {
                locale: "en".to_string(),
                title: Some("Home".to_string()),
                slug: Some("home".to_string()),
                meta_title: None,
                meta_description: None,
            }),
            body: None,
        }
    }

    #[test]
    fn metadata_snapshot_uses_page_version_not_document_revision() {
        let snapshot = metadata_snapshot(&pages_metadata_property_schema(), &page(7), "en")
            .expect("metadata snapshot");
        assert_eq!(snapshot.revision, "pages:page-1:metadata:v7");
        assert_eq!(snapshot.values["title"], "Home");
    }

    #[test]
    fn expected_revision_is_scoped_to_the_selected_page() {
        assert_eq!(
            expected_metadata_version("page-1", "pages:page-1:metadata:v7").expect("version"),
            7
        );
        assert!(expected_metadata_version("page-2", "pages:page-1:metadata:v7").is_err());
    }
}
