use fly::RegistrySet;
use fly_ui::{ModuleContributionManifest, Presentation};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
pub type PageBuilderContributionPreviewFuture = Pin<
    Box<
        dyn Future<Output = Result<Value, PageBuilderContributionPreviewError>>
            + Send
            + 'static,
    >,
>;

#[cfg(target_arch = "wasm32")]
pub type PageBuilderContributionPreviewFuture = Pin<
    Box<dyn Future<Output = Result<Value, PageBuilderContributionPreviewError>> + 'static>,
>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderContributionPreviewRequest {
    pub provider: String,
    pub component_type: String,
    pub component_id: String,
    pub presentation: Presentation,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderContributionPreviewError {
    pub message: String,
    pub stable_code: Option<String>,
}

impl PageBuilderContributionPreviewError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stable_code: None,
        }
    }

    pub fn with_stable_code(message: impl Into<String>, code: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            stable_code: Some(code.into()),
        }
    }
}

impl std::fmt::Display for PageBuilderContributionPreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.stable_code.as_deref() {
            Some(code) => write!(formatter, "{} ({code})", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for PageBuilderContributionPreviewError {}

/// Async owner-data preview port. Fly contribution adapters remain synchronous and framework
/// neutral; transports live here at the Page Builder admin composition boundary.
pub trait PageBuilderContributionPreviewPort: Send + Sync {
    fn preview(
        &self,
        request: PageBuilderContributionPreviewRequest,
    ) -> PageBuilderContributionPreviewFuture;
}

pub type PageBuilderRegistryInstaller =
    Arc<dyn Fn(&mut RegistrySet) -> Result<(), String> + Send + Sync>;

/// One optional-domain extension supplied by the application composition root.
///
/// The extension carries only public contribution metadata, Fly registry installation and an
/// optional owner preview port. It contains no tenant policy, persistence or domain state.
#[derive(Clone)]
pub struct PageBuilderContributionHostExtension {
    manifest: ModuleContributionManifest,
    registry_installer: PageBuilderRegistryInstaller,
    preview_port: Option<Arc<dyn PageBuilderContributionPreviewPort>>,
}

impl PageBuilderContributionHostExtension {
    pub fn new(
        manifest: ModuleContributionManifest,
        registry_installer: impl Fn(&mut RegistrySet) -> Result<(), String> + Send + Sync + 'static,
    ) -> Self {
        Self {
            manifest,
            registry_installer: Arc::new(registry_installer),
            preview_port: None,
        }
    }

    pub fn with_preview_port(
        mut self,
        preview_port: Arc<dyn PageBuilderContributionPreviewPort>,
    ) -> Self {
        self.preview_port = Some(preview_port);
        self
    }

    pub fn module_id(&self) -> &str {
        &self.manifest.module_id
    }

    pub fn owner_provider(&self) -> &str {
        &self.manifest.owner_provider
    }

    pub fn manifest(&self) -> &ModuleContributionManifest {
        &self.manifest
    }

    fn install(&self, registries: &mut RegistrySet) -> Result<(), String> {
        (self.registry_installer)(registries)
    }
}

/// Host-owned extension set shared with the concrete Page Builder consumer surface.
#[derive(Clone, Default)]
pub struct PageBuilderContributionHostContext {
    extensions: Arc<Vec<PageBuilderContributionHostExtension>>,
}

impl PageBuilderContributionHostContext {
    pub fn new(
        extensions: Vec<PageBuilderContributionHostExtension>,
    ) -> Result<Self, String> {
        let mut modules = BTreeSet::new();
        let mut providers = BTreeSet::new();
        for extension in &extensions {
            let module_id = extension.module_id().trim();
            let provider = extension.owner_provider().trim();
            if module_id.is_empty() || provider.is_empty() {
                return Err("Page Builder contribution host extension requires module/provider identity".to_string());
            }
            if !modules.insert(module_id.to_string()) {
                return Err(format!(
                    "Page Builder contribution host module `{module_id}` is duplicated"
                ));
            }
            if !providers.insert(provider.to_string()) {
                return Err(format!(
                    "Page Builder contribution host provider `{provider}` is duplicated"
                ));
            }
        }
        Ok(Self {
            extensions: Arc::new(extensions),
        })
    }

    pub fn manifests(&self) -> Vec<ModuleContributionManifest> {
        self.extensions
            .iter()
            .map(|extension| extension.manifest().clone())
            .collect()
    }

    pub fn module_ids(&self) -> BTreeSet<String> {
        self.extensions
            .iter()
            .map(|extension| extension.module_id().to_string())
            .collect()
    }

    pub fn owner_providers(&self) -> BTreeSet<String> {
        self.extensions
            .iter()
            .map(|extension| extension.owner_provider().to_string())
            .collect()
    }

    pub fn install_registries(&self, registries: &mut RegistrySet) -> Result<(), String> {
        for extension in self.extensions.iter() {
            extension.install(registries)?;
        }
        Ok(())
    }

    pub fn preview_port(
        &self,
        provider: &str,
    ) -> Option<Arc<dyn PageBuilderContributionPreviewPort>> {
        let provider = provider.trim();
        self.extensions
            .iter()
            .find(|extension| extension.owner_provider().trim() == provider)
            .and_then(|extension| extension.preview_port.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}
