from pathlib import Path


def read(path: str) -> str:
    return Path(path).read_text(encoding="utf-8")


def write(path: str, content: str) -> None:
    Path(path).write_text(content, encoding="utf-8")


def replace_exact(path: str, old: str, new: str, expected: int = 1) -> None:
    content = read(path)
    count = content.count(old)
    if count != expected:
        raise RuntimeError(
            f"{path}: expected {expected} occurrences of {old!r}, found {count}"
        )
    write(path, content.replace(old, new))


def route_application_calls(path: str, mapping: dict[str, str]) -> None:
    content = read(path)
    for method, accessor in sorted(mapping.items(), key=lambda item: len(item[0]), reverse=True):
        needle = f".{method}("
        replacement = f".{accessor}().{method}("
        content = content.replace(needle, replacement)
    write(path, content)


applications = r'''use std::sync::Arc;

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_api::TenantContext;
use rustok_core::ModuleRuntimeExtensions;
use rustok_media::MediaAssetReadPort;
use rustok_outbox::TransactionalEventBus;
use rustok_seo_targets::{
    SeoTargetCapabilityKind, SeoTargetRegistry, SeoTargetRegistryEntry, SeoTargetSlug,
};

use crate::dto::{
    SeoBulkApplyInput, SeoBulkExportInput, SeoBulkImportInput, SeoBulkJobRecord, SeoBulkJobStatus,
    SeoBulkListInput, SeoBulkPage, SeoBulkSelectionInput, SeoBulkSelectionPreviewRecord,
    SeoCrossLinkSuggestionRecord, SeoDiagnosticsSummaryRecord, SeoIndexDeliveryStatusRecord,
    SeoIndexRepairReplayResultRecord, SeoMetaInput, SeoMetaRecord, SeoModuleSettings,
    SeoPageContext, SeoRedirectInput, SeoRedirectRecord, SeoRevisionRecord,
    SeoRobotsPreviewRecord, SeoSitemapJobRecord, SeoSitemapStatusRecord,
};
use crate::entities::{seo_bulk_job_artifact, seo_sitemap_file};
use crate::SeoResult;

use super::SeoService;

/// Composition root for the focused SEO application services.
///
/// The underlying runtime remains shared so cross-cutting transactional helpers keep one
/// database connection, event bus, target registry, and optional media provider.
#[derive(Clone)]
pub struct SeoApplicationServices {
    runtime: SeoService,
}

impl SeoApplicationServices {
    pub fn new(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        registry: Arc<SeoTargetRegistry>,
    ) -> Self {
        Self {
            runtime: SeoService::new(db, event_bus, registry),
        }
    }

    pub fn from_runtime_extensions(
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
        extensions: &ModuleRuntimeExtensions,
    ) -> SeoResult<Self> {
        SeoService::from_runtime_extensions(db, event_bus, extensions)
            .map(|runtime| Self { runtime })
    }

    pub fn with_media_asset_read_port(self, port: Arc<dyn MediaAssetReadPort>) -> Self {
        Self {
            runtime: self.runtime.with_media_asset_read_port(port),
        }
    }

    pub fn settings(&self) -> SeoSettingsService {
        SeoSettingsService::new(self.runtime.clone())
    }

    pub fn metadata(&self) -> SeoMetadataService {
        SeoMetadataService::new(self.runtime.clone())
    }

    pub fn routing(&self) -> SeoRoutingService {
        SeoRoutingService::new(self.runtime.clone())
    }

    pub fn redirects(&self) -> SeoRedirectService {
        SeoRedirectService::new(self.runtime.clone())
    }

    pub fn sitemaps(&self) -> SeoSitemapService {
        SeoSitemapService::new(self.runtime.clone())
    }

    pub fn bulk(&self) -> SeoBulkService {
        SeoBulkService::new(self.runtime.clone())
    }

    pub fn operations(&self) -> SeoOperationsService {
        SeoOperationsService::new(self.runtime.clone())
    }
}

#[derive(Clone)]
pub struct SeoSettingsService {
    runtime: SeoService,
}

impl SeoSettingsService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn is_enabled(&self, tenant_id: Uuid) -> SeoResult<bool> {
        self.runtime.is_enabled(tenant_id).await
    }

    pub async fn load_settings(&self, tenant_id: Uuid) -> SeoResult<SeoModuleSettings> {
        self.runtime.load_settings(tenant_id).await
    }

    pub fn normalize_settings(settings: SeoModuleSettings) -> SeoModuleSettings {
        SeoService::normalize_settings(settings)
    }
}

#[derive(Clone)]
pub struct SeoMetadataService {
    runtime: SeoService,
}

impl SeoMetadataService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn seo_meta(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        locale: Option<&str>,
    ) -> SeoResult<Option<SeoMetaRecord>> {
        self.runtime
            .seo_meta(tenant, target_kind, target_id, locale)
            .await
    }

    pub async fn upsert_meta(
        &self,
        tenant: &TenantContext,
        input: SeoMetaInput,
    ) -> SeoResult<SeoMetaRecord> {
        self.runtime.upsert_meta(tenant, input).await
    }

    pub async fn publish_revision(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        note: Option<String>,
    ) -> SeoResult<SeoRevisionRecord> {
        self.runtime
            .publish_revision(tenant, target_kind, target_id, note)
            .await
    }

    pub async fn rollback_revision(
        &self,
        tenant: &TenantContext,
        target_kind: SeoTargetSlug,
        target_id: Uuid,
        revision: i32,
    ) -> SeoResult<SeoMetaRecord> {
        self.runtime
            .rollback_revision(tenant, target_kind, target_id, revision)
            .await
    }
}

#[derive(Clone)]
pub struct SeoRoutingService {
    runtime: SeoService,
}

impl SeoRoutingService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn resolve_page_context(
        &self,
        tenant: &TenantContext,
        locale: &str,
        route: &str,
    ) -> SeoResult<Option<SeoPageContext>> {
        self.runtime.resolve_page_context(tenant, locale, route).await
    }

    pub async fn resolve_page_context_for_channel(
        &self,
        tenant: &TenantContext,
        locale: &str,
        route: &str,
        channel_slug: Option<&str>,
    ) -> SeoResult<Option<SeoPageContext>> {
        self.runtime
            .resolve_page_context_for_channel(tenant, locale, route, channel_slug)
            .await
    }

    pub fn target_registry_entries(
        &self,
        capability: Option<SeoTargetCapabilityKind>,
    ) -> Vec<SeoTargetRegistryEntry> {
        self.runtime.target_registry_entries(capability)
    }

    pub async fn cross_link_suggestions(
        &self,
        tenant: &TenantContext,
        locale: Option<&str>,
        per_target_limit: Option<usize>,
    ) -> SeoResult<Vec<SeoCrossLinkSuggestionRecord>> {
        self.runtime
            .cross_link_suggestions(tenant, locale, per_target_limit)
            .await
    }
}

#[derive(Clone)]
pub struct SeoRedirectService {
    runtime: SeoService,
}

impl SeoRedirectService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn list_redirects(&self, tenant_id: Uuid) -> SeoResult<Vec<SeoRedirectRecord>> {
        self.runtime.list_redirects(tenant_id).await
    }

    pub async fn upsert_redirect(
        &self,
        tenant: &TenantContext,
        input: SeoRedirectInput,
    ) -> SeoResult<SeoRedirectRecord> {
        self.runtime.upsert_redirect(tenant, input).await
    }
}

#[derive(Clone)]
pub struct SeoSitemapService {
    runtime: SeoService,
}

impl SeoSitemapService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn generate_sitemaps(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoSitemapStatusRecord> {
        self.runtime.generate_sitemaps(tenant).await
    }

    pub async fn sitemap_status(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoSitemapStatusRecord> {
        self.runtime.sitemap_status(tenant).await
    }

    pub async fn list_sitemap_jobs(
        &self,
        tenant_id: Uuid,
        limit: usize,
    ) -> SeoResult<Vec<SeoSitemapJobRecord>> {
        self.runtime.list_sitemap_jobs(tenant_id, limit).await
    }

    pub async fn sitemap_job(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> SeoResult<Option<SeoSitemapJobRecord>> {
        self.runtime.sitemap_job(tenant_id, job_id).await
    }

    pub async fn render_robots(&self, tenant: &TenantContext) -> SeoResult<String> {
        self.runtime.render_robots(tenant).await
    }

    pub async fn robots_preview(
        &self,
        tenant: &TenantContext,
    ) -> SeoResult<SeoRobotsPreviewRecord> {
        self.runtime.robots_preview(tenant).await
    }

    pub async fn latest_sitemap_index(
        &self,
        tenant_id: Uuid,
    ) -> SeoResult<Option<seo_sitemap_file::Model>> {
        self.runtime.latest_sitemap_index(tenant_id).await
    }

    pub async fn sitemap_file(
        &self,
        tenant_id: Uuid,
        path: &str,
    ) -> SeoResult<Option<seo_sitemap_file::Model>> {
        self.runtime.sitemap_file(tenant_id, path).await
    }
}

#[derive(Clone)]
pub struct SeoBulkService {
    runtime: SeoService,
}

impl SeoBulkService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn list_bulk_items(
        &self,
        tenant: &TenantContext,
        input: SeoBulkListInput,
    ) -> SeoResult<SeoBulkPage> {
        self.runtime.list_bulk_items(tenant, input).await
    }

    pub async fn preview_bulk_selection_count(
        &self,
        tenant: &TenantContext,
        selection: SeoBulkSelectionInput,
    ) -> SeoResult<SeoBulkSelectionPreviewRecord> {
        self.runtime
            .preview_bulk_selection_count(tenant, selection)
            .await
    }

    pub async fn queue_bulk_apply(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkApplyInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        self.runtime
            .queue_bulk_apply(tenant, created_by, input)
            .await
    }

    pub async fn queue_bulk_export(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkExportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        self.runtime
            .queue_bulk_export(tenant, created_by, input)
            .await
    }

    pub async fn queue_bulk_import(
        &self,
        tenant: &TenantContext,
        created_by: Option<Uuid>,
        input: SeoBulkImportInput,
    ) -> SeoResult<SeoBulkJobRecord> {
        self.runtime
            .queue_bulk_import(tenant, created_by, input)
            .await
    }

    pub async fn list_bulk_jobs(
        &self,
        tenant_id: Uuid,
        limit: usize,
        status: Option<SeoBulkJobStatus>,
    ) -> SeoResult<Vec<SeoBulkJobRecord>> {
        self.runtime.list_bulk_jobs(tenant_id, limit, status).await
    }

    pub async fn bulk_job(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
    ) -> SeoResult<Option<SeoBulkJobRecord>> {
        self.runtime.bulk_job(tenant_id, job_id).await
    }

    pub async fn bulk_artifact(
        &self,
        tenant_id: Uuid,
        job_id: Uuid,
        artifact_id: Uuid,
    ) -> SeoResult<Option<seo_bulk_job_artifact::Model>> {
        self.runtime
            .bulk_artifact(tenant_id, job_id, artifact_id)
            .await
    }

    pub async fn execute_next_bulk_job(&self) -> SeoResult<Option<SeoBulkJobRecord>> {
        self.runtime.execute_next_bulk_job().await
    }
}

#[derive(Clone)]
pub struct SeoOperationsService {
    runtime: SeoService,
}

impl SeoOperationsService {
    fn new(runtime: SeoService) -> Self {
        Self { runtime }
    }

    pub async fn diagnostics_summary(
        &self,
        tenant: &TenantContext,
        locale: Option<&str>,
    ) -> SeoResult<SeoDiagnosticsSummaryRecord> {
        self.runtime.diagnostics_summary(tenant, locale).await
    }

    pub async fn index_delivery_status(
        &self,
        tenant_id: Uuid,
        target_type: Option<&str>,
    ) -> SeoResult<SeoIndexDeliveryStatusRecord> {
        self.runtime
            .index_delivery_status(tenant_id, target_type)
            .await
    }

    pub async fn run_index_repair_replay(
        &self,
        tenant_id: Uuid,
        target_type: Option<&str>,
        limit: usize,
        replay_historical: bool,
    ) -> SeoResult<SeoIndexRepairReplayResultRecord> {
        self.runtime
            .run_index_repair_replay(tenant_id, target_type, limit, replay_historical)
            .await
    }
}
'''
write("crates/rustok-seo/src/services/applications.rs", applications)

replace_exact(
    "crates/rustok-seo/src/services/services_base.rs",
    "mod bulk;\n",
    "mod applications;\nmod bulk;\n",
)
replace_exact(
    "crates/rustok-seo/src/services/services_base.rs",
    "use crate::{SeoError, SeoResult};\n\n",
    "use crate::{SeoError, SeoResult};\n\n"
    "pub use applications::{\n"
    "    SeoApplicationServices, SeoBulkService, SeoMetadataService, SeoOperationsService,\n"
    "    SeoRedirectService, SeoRoutingService, SeoSettingsService, SeoSitemapService,\n"
    "};\n\n",
)
replace_exact(
    "crates/rustok-seo/src/services/services_base.rs",
    "pub struct SeoService {\n",
    "pub(crate) struct SeoService {\n",
)

replace_exact(
    "crates/rustok-seo/src/lib.rs",
    "pub use services::{SeoMediaAssetReadProvider, SeoService};\n",
    "pub use services::{\n"
    "    SeoApplicationServices, SeoBulkService, SeoMediaAssetReadProvider, SeoMetadataService,\n"
    "    SeoOperationsService, SeoRedirectService, SeoRoutingService, SeoSettingsService,\n"
    "    SeoSitemapService,\n"
    "};\n",
)

external_files = [
    "crates/rustok-seo/src/graphql/mod.rs",
    "crates/rustok-seo/src/controllers/mod.rs",
    "apps/server/src/services/app_lifecycle.rs",
    "crates/rustok-seo/admin/src/transport/native_server_adapter.rs",
    "apps/storefront/src/shared/context/seo_page_context_native_server_adapter.rs",
    "crates/rustok-seo/tests/meta_transaction.rs",
    "crates/rustok-seo/tests/redirect_transaction.rs",
    "crates/rustok-seo/tests/sitemap_transaction.rs",
    "crates/rustok-seo/tests/bulk_terminal_transaction.rs",
]
for path in external_files:
    content = read(path)
    content = content.replace("SeoService", "SeoApplicationServices")
    write(path, content)

admin_path = "crates/rustok-seo/admin/src/transport/native_server_adapter.rs"
admin = read(admin_path)
admin = admin.replace(
    "use rustok_seo::SeoApplicationServices;\n",
    "use rustok_seo::{SeoApplicationServices, SeoSettingsService};\n",
)
admin = admin.replace(
    "SeoApplicationServices::normalize_settings(input)",
    "SeoSettingsService::normalize_settings(input)",
)
write(admin_path, admin)

method_groups = {
    "resolve_page_context_for_channel": "routing",
    "resolve_page_context": "routing",
    "target_registry_entries": "routing",
    "cross_link_suggestions": "routing",
    "seo_meta": "metadata",
    "upsert_meta": "metadata",
    "publish_revision": "metadata",
    "rollback_revision": "metadata",
    "list_redirects": "redirects",
    "upsert_redirect": "redirects",
    "generate_sitemaps": "sitemaps",
    "sitemap_status": "sitemaps",
    "list_sitemap_jobs": "sitemaps",
    "sitemap_job": "sitemaps",
    "render_robots": "sitemaps",
    "robots_preview": "sitemaps",
    "latest_sitemap_index": "sitemaps",
    "sitemap_file": "sitemaps",
    "list_bulk_items": "bulk",
    "preview_bulk_selection_count": "bulk",
    "queue_bulk_apply": "bulk",
    "queue_bulk_export": "bulk",
    "queue_bulk_import": "bulk",
    "list_bulk_jobs": "bulk",
    "bulk_job": "bulk",
    "bulk_artifact": "bulk",
    "execute_next_bulk_job": "bulk",
    "diagnostics_summary": "operations",
    "index_delivery_status": "operations",
    "run_index_repair_replay": "operations",
    "is_enabled": "settings",
    "load_settings": "settings",
}
for path in external_files:
    route_application_calls(path, method_groups)

replace_exact(
    "docs/roadmaps/seo-hardening-progress.md",
    "- [ ] Split the broad `SeoService` facade into focused application services.\n",
    "- [x] Split the broad `SeoService` facade into focused application services. (application services PR)\n",
)

# Static boundary checks for this mechanical refactor. These inspect source only and do not run tests.
for path in external_files:
    content = read(path)
    if "SeoService" in content:
        raise RuntimeError(f"{path}: broad SeoService remains in an external entry point")

lib = read("crates/rustok-seo/src/lib.rs")
if "SeoMediaAssetReadProvider, SeoService" in lib or "SeoService," in lib:
    raise RuntimeError("crates/rustok-seo/src/lib.rs: SeoService is still publicly exported")

base = read("crates/rustok-seo/src/services/services_base.rs")
if "pub(crate) struct SeoService" not in base:
    raise RuntimeError("internal SeoService visibility was not narrowed")
