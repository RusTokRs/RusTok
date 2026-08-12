use crate::builder::PagesBuilderSaveSnapshot;
use crate::contributions::{
    PAGES_METADATA_COMPONENT_TYPE, PAGES_METADATA_CONTRIBUTION_ID,
    PAGES_METADATA_PROPERTY_EDITOR_ID, PAGES_OWNER_PROVIDER, pages_metadata_property_schema,
};
use crate::core;
use crate::model::{PageDetail, PageMetadataPatch, PageMutationResult};
use crate::transport;
use rustok_page_builder_admin::{
    ConsumerPropertyEditorError, ConsumerPropertyEditorPort, ConsumerPropertyEditorRuntime,
    ConsumerPropertyEditorSnapshot, ConsumerPropertyLoadFuture, ConsumerPropertySaveFuture,
    ConsumerPropertySaveReceipt, SaveConsumerPropertiesInput,
};
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

const PAGE_METADATA_REVISION_CONFLICT: &str = "REVISION_CONFLICT";

type SnapshotProvider = Arc<dyn Fn() -> PagesBuilderSaveSnapshot + Send + Sync>;
type SavedHandler = Arc<dyn Fn(PageMutationResult) + Send + Sync>;

#[cfg(not(target_arch = "wasm32"))]
type MetadataPageLoadFuture = Pin<
    Box<
        dyn Future<Output = Result<Option<PageDetail>, ConsumerPropertyEditorError>>
            + Send
            + 'static,
    >,
>;

#[cfg(target_arch = "wasm32")]
type MetadataPageLoadFuture = Pin<
    Box<dyn Future<Output = Result<Option<PageDetail>, ConsumerPropertyEditorError>> + 'static>,
>;

#[cfg(not(target_arch = "wasm32"))]
type MetadataPageSaveFuture =
    Pin<Box<dyn Future<Output = Result<PageDetail, ConsumerPropertyEditorError>> + Send + 'static>>;

#[cfg(target_arch = "wasm32")]
type MetadataPageSaveFuture =
    Pin<Box<dyn Future<Output = Result<PageDetail, ConsumerPropertyEditorError>> + 'static>>;

trait PagesMetadataTransport: Send + Sync {
    fn fetch_page(&self, snapshot: PagesBuilderSaveSnapshot) -> MetadataPageLoadFuture;

    fn patch_metadata(&self, request: PageMetadataPatch) -> MetadataPageSaveFuture;
}

struct ServerPagesMetadataTransport;

impl PagesMetadataTransport for ServerPagesMetadataTransport {
    fn fetch_page(&self, snapshot: PagesBuilderSaveSnapshot) -> MetadataPageLoadFuture {
        Box::pin(async move {
            transport::fetch_page(snapshot.token, snapshot.tenant_slug, snapshot.page_id)
                .await
                .map_err(|error| ConsumerPropertyEditorError::unavailable(error.to_string()))
        })
    }

    fn patch_metadata(&self, request: PageMetadataPatch) -> MetadataPageSaveFuture {
        Box::pin(async move {
            transport::patch_page_metadata(request)
                .await
                .map_err(|error| ConsumerPropertyEditorError::save(error.to_string()))
        })
    }
}

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
            transport: Arc::new(ServerPagesMetadataTransport),
        }),
    ))
}

struct PagesMetadataPropertyPort {
    snapshot: SnapshotProvider,
    on_saved: SavedHandler,
    schema: rustok_page_builder_admin::ConsumerPropertyEditorSchema,
    transport: Arc<dyn PagesMetadataTransport>,
}

impl ConsumerPropertyEditorPort for PagesMetadataPropertyPort {
    fn load(&self) -> ConsumerPropertyLoadFuture {
        let snapshot = (self.snapshot)();
        let schema = self.schema.clone();
        let transport = Arc::clone(&self.transport);
        Box::pin(async move {
            let page = fetch_expected_page(transport.as_ref(), &snapshot).await?;
            metadata_snapshot(&schema, &page, &snapshot.default_locale)
        })
    }

    fn save(&self, input: SaveConsumerPropertiesInput) -> ConsumerPropertySaveFuture {
        let snapshot = (self.snapshot)();
        let schema = self.schema.clone();
        let transport = Arc::clone(&self.transport);
        let on_saved = Arc::clone(&self.on_saved);
        Box::pin(async move {
            let command = metadata_save_command(&schema, &snapshot, &input)?;
            let current = fetch_expected_page(transport.as_ref(), &snapshot).await?;
            require_current_metadata_version(command.expected_version, current.version)?;
            let request = PageMetadataPatch {
                token: snapshot.token,
                tenant_slug: snapshot.tenant_slug,
                page_id: snapshot.page_id.clone(),
                expected_version: command.expected_version,
                locale: page_locale(&current, &snapshot.default_locale),
                title: command.title,
                slug: command.slug,
                meta_title: command.meta_title,
                meta_description: command.meta_description,
                template: command.template,
                channel_slugs: command.channel_slugs,
            };
            let page = transport.patch_metadata(request).await?;
            if page.id != snapshot.page_id {
                return Err(ConsumerPropertyEditorError::save(format!(
                    "Pages metadata save returned page `{}` for `{}`",
                    page.id, snapshot.page_id
                )));
            }
            if page.version <= command.expected_version {
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct MetadataSaveCommand {
    expected_version: i32,
    title: String,
    slug: String,
    meta_title: Option<String>,
    meta_description: Option<String>,
    template: Option<String>,
    channel_slugs: Vec<String>,
}

fn metadata_save_command(
    schema: &rustok_page_builder_admin::ConsumerPropertyEditorSchema,
    snapshot: &PagesBuilderSaveSnapshot,
    input: &SaveConsumerPropertiesInput,
) -> Result<MetadataSaveCommand, ConsumerPropertyEditorError> {
    if input.contribution_id != PAGES_METADATA_CONTRIBUTION_ID
        || input.property_editor_id != PAGES_METADATA_PROPERTY_EDITOR_ID
    {
        return Err(ConsumerPropertyEditorError::contract(
            "Pages metadata save does not match the registered contribution",
        ));
    }
    schema.validate_values(&input.values)?;
    let expected_version = expected_metadata_version(&snapshot.page_id, &input.expected_revision)?;
    Ok(MetadataSaveCommand {
        expected_version,
        title: required_value(&input.values, "title")?,
        slug: required_value(&input.values, "slug")?,
        meta_title: optional_value(&input.values, "meta_title")?,
        meta_description: optional_value(&input.values, "meta_description")?,
        template: optional_value(&input.values, "template")?,
        channel_slugs: core::parse_channel_slugs(value(&input.values, "channel_slugs")?),
    })
}

fn require_current_metadata_version(
    expected_version: i32,
    current_version: i32,
) -> Result<(), ConsumerPropertyEditorError> {
    if current_version == expected_version {
        Ok(())
    } else {
        Err(metadata_revision_conflict(
            expected_version,
            current_version,
        ))
    }
}

async fn fetch_expected_page(
    transport: &dyn PagesMetadataTransport,
    snapshot: &PagesBuilderSaveSnapshot,
) -> Result<PageDetail, ConsumerPropertyEditorError> {
    if snapshot.page_id.trim().is_empty() {
        return Err(ConsumerPropertyEditorError::unavailable(
            "Pages metadata properties require a selected page",
        ));
    }
    let page = transport
        .fetch_page(snapshot.clone())
        .await?
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
        ("channel_slugs".to_string(), page.channel_slugs.join(", ")),
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
    use crate::model::{PageBody, PageTranslation};
    use serde_json::json;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Clone)]
    struct RecordingTransport {
        current: PageDetail,
        saved: PageDetail,
        patch_calls: Arc<AtomicUsize>,
        last_patch: Arc<Mutex<Option<PageMetadataPatch>>>,
    }

    impl PagesMetadataTransport for RecordingTransport {
        fn fetch_page(&self, _snapshot: PagesBuilderSaveSnapshot) -> MetadataPageLoadFuture {
            let current = self.current.clone();
            Box::pin(async move { Ok(Some(current)) })
        }

        fn patch_metadata(&self, request: PageMetadataPatch) -> MetadataPageSaveFuture {
            self.patch_calls.fetch_add(1, Ordering::SeqCst);
            *self.last_patch.lock().expect("last patch lock") = Some(request);
            let saved = self.saved.clone();
            Box::pin(async move { Ok(saved) })
        }
    }

    fn page(version: i32) -> PageDetail {
        page_with_title(version, "Home")
    }

    fn page_with_title(version: i32, title: &str) -> PageDetail {
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
                title: Some(title.to_string()),
                slug: Some("home".to_string()),
                meta_title: None,
                meta_description: None,
            }),
            body: Some(PageBody {
                locale: "en".to_string(),
                content: "<section>persisted</section>".to_string(),
                format: "grapesjs".to_string(),
                content_json: Some(json!({
                    "pages": [{
                        "component": {
                            "tagName": "section",
                            "attributes": {"data-persisted": "true"}
                        }
                    }]
                })),
                updated_at: "2026-07-23T00:00:00Z".to_string(),
            }),
        }
    }

    fn snapshot() -> PagesBuilderSaveSnapshot {
        PagesBuilderSaveSnapshot {
            token: Some("token".to_string()),
            tenant_slug: Some("tenant".to_string()),
            page_id: "page-1".to_string(),
            default_locale: "en".to_string(),
        }
    }

    fn save_input(version: i32, title: &str) -> SaveConsumerPropertiesInput {
        let mut values = metadata_values(&page(version));
        values.insert("title".to_string(), title.to_string());
        SaveConsumerPropertiesInput {
            contribution_id: PAGES_METADATA_CONTRIBUTION_ID.to_string(),
            property_editor_id: PAGES_METADATA_PROPERTY_EDITOR_ID.to_string(),
            expected_revision: metadata_revision("page-1", version),
            values,
        }
    }

    fn port(
        transport: RecordingTransport,
        on_saved: impl Fn(PageMutationResult) + Send + Sync + 'static,
    ) -> PagesMetadataPropertyPort {
        PagesMetadataPropertyPort {
            snapshot: Arc::new(snapshot),
            on_saved: Arc::new(on_saved),
            schema: pages_metadata_property_schema(),
            transport: Arc::new(transport),
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

    #[tokio::test]
    async fn stale_metadata_revision_short_circuits_before_patch_transport() {
        let patch_calls = Arc::new(AtomicUsize::new(0));
        let transport = RecordingTransport {
            current: page(8),
            saved: page(9),
            patch_calls: Arc::clone(&patch_calls),
            last_patch: Arc::new(Mutex::new(None)),
        };
        let error = port(transport, |_| {})
            .save(save_input(7, "Changed"))
            .await
            .expect_err("stale metadata revision must fail");

        assert_eq!(error.stable_code, PAGE_METADATA_REVISION_CONFLICT);
        assert_eq!(
            error.message,
            "Pages metadata version changed from 7 to 8; reload and retry"
        );
        assert_eq!(patch_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn metadata_save_is_document_free_and_preserves_dirty_fly_state() {
        let patch_calls = Arc::new(AtomicUsize::new(0));
        let last_patch = Arc::new(Mutex::new(None));
        let dirty_fly_state = Arc::new(Mutex::new(json!({
            "revision": "draft-local-9",
            "dirty": true,
            "projectData": {
                "pages": [{
                    "component": {
                        "tagName": "main",
                        "attributes": {"data-unsaved": "true"}
                    }
                }]
            }
        })));
        let dirty_before = dirty_fly_state.lock().expect("dirty Fly lock").clone();
        let saved_mutation = Arc::new(Mutex::new(None::<PageMutationResult>));
        let saved_mutation_capture = Arc::clone(&saved_mutation);
        let transport = RecordingTransport {
            current: page(7),
            saved: page_with_title(8, "Updated Home"),
            patch_calls: Arc::clone(&patch_calls),
            last_patch: Arc::clone(&last_patch),
        };

        let receipt = port(transport, move |result| {
            *saved_mutation_capture.lock().expect("saved mutation lock") = Some(result);
        })
        .save(save_input(7, "Updated Home"))
        .await
        .expect("metadata save");

        assert_eq!(patch_calls.load(Ordering::SeqCst), 1);
        let request = last_patch
            .lock()
            .expect("last patch lock")
            .clone()
            .expect("metadata patch request");
        assert_eq!(request.page_id, "page-1");
        assert_eq!(request.expected_version, 7);
        assert_eq!(request.title, "Updated Home");
        assert_eq!(request.slug, "home");
        assert_eq!(request.locale, "en");
        assert_eq!(request.channel_slugs, vec!["web".to_string()]);
        assert_eq!(
            *dirty_fly_state.lock().expect("dirty Fly lock"),
            dirty_before
        );
        assert_eq!(receipt.revision, "pages:page-1:metadata:v8");
        assert_eq!(receipt.values["title"], "Updated Home");
        assert_eq!(
            saved_mutation
                .lock()
                .expect("saved mutation lock")
                .as_ref()
                .expect("saved mutation")
                .version,
            8
        );
    }
}
