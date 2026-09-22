use fly::RegistrySet;
use fly_ui::{
    ContributionAssemblyDiagnostic, ContributionAssemblyPolicy, ContributionAssemblyResult,
    ContributionAssemblySeverity, ModuleContributionManifest, Presentation,
    build_admin_contribution_registry_from_manifests,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type PageBuilderContributionPreviewFuture =
    Pin<Box<dyn Future<Output = Result<Value, PageBuilderContributionPreviewError>> + 'static>>;

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
/// neutral; transports live here at the Page Builder admin composition boundary. The future itself
/// is intentionally local because the admin UI executes it through `spawn_local`; the port object
/// remains `Send + Sync` for host-context composition.
pub trait PageBuilderContributionPreviewPort: Send + Sync {
    fn preview(
        &self,
        request: PageBuilderContributionPreviewRequest,
    ) -> PageBuilderContributionPreviewFuture;
}

pub type PageBuilderContributionPropertySchemaFuture = Pin<
    Box<
        dyn Future<
                Output = Result<
                    PageBuilderContributionPropertySchema,
                    PageBuilderContributionPropertyError,
                >,
            > + 'static,
    >,
>;

pub type PageBuilderContributionPropertyValidationFuture = Pin<
    Box<
        dyn Future<
                Output = Result<
                    PageBuilderContributionPropertyValidation,
                    PageBuilderContributionPropertyError,
                >,
            > + 'static,
    >,
>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderContributionPropertySchemaRequest {
    pub provider: String,
    pub component_type: String,
    pub component_id: String,
    pub property_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderContributionPropertyValidationRequest {
    pub provider: String,
    pub component_type: String,
    pub component_id: String,
    pub property_schema: Value,
    #[serde(default)]
    pub props: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderContributionPropertySchema {
    pub schema_id: String,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageBuilderContributionPropertyIssue {
    pub class: String,
    pub code: String,
    pub message: String,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageBuilderContributionPropertyValidation {
    pub valid: bool,
    pub normalized_props: Value,
    #[serde(default)]
    pub issues: Vec<PageBuilderContributionPropertyIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageBuilderContributionPropertyError {
    pub message: String,
    pub stable_code: Option<String>,
}

impl PageBuilderContributionPropertyError {
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

impl std::fmt::Display for PageBuilderContributionPropertyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.stable_code.as_deref() {
            Some(code) => write!(formatter, "{} ({code})", self.message),
            None => formatter.write_str(&self.message),
        }
    }
}

impl std::error::Error for PageBuilderContributionPropertyError {}

/// Owner-backed schema and normalization port for dynamic contribution properties.
///
/// The Page Builder host never persists owner data. It asks the provider for the current schema and
/// normalized configuration, then writes only the validated `props` object through ordinary Fly
/// commands. Owner transports must independently enforce tenant/module/RBAC admission.
pub trait PageBuilderContributionPropertyPort: Send + Sync {
    fn schema(
        &self,
        request: PageBuilderContributionPropertySchemaRequest,
    ) -> PageBuilderContributionPropertySchemaFuture;

    fn validate(
        &self,
        request: PageBuilderContributionPropertyValidationRequest,
    ) -> PageBuilderContributionPropertyValidationFuture;
}

pub type PageBuilderRegistryInstaller =
    Arc<dyn Fn(&mut RegistrySet) -> Result<(), String> + Send + Sync>;

/// One optional-domain extension supplied by the application composition root.
///
/// The extension carries only public contribution metadata, Fly registry installation and optional
/// owner preview/property ports. It contains no tenant policy, persistence or domain state.
#[derive(Clone)]
pub struct PageBuilderContributionHostExtension {
    manifest: ModuleContributionManifest,
    registry_installer: PageBuilderRegistryInstaller,
    preview_port: Option<Arc<dyn PageBuilderContributionPreviewPort>>,
    property_port: Option<Arc<dyn PageBuilderContributionPropertyPort>>,
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
            property_port: None,
        }
    }

    pub fn with_preview_port(
        mut self,
        preview_port: Arc<dyn PageBuilderContributionPreviewPort>,
    ) -> Self {
        self.preview_port = Some(preview_port);
        self
    }

    pub fn with_property_port(
        mut self,
        property_port: Arc<dyn PageBuilderContributionPropertyPort>,
    ) -> Self {
        self.property_port = Some(property_port);
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

/// Host-owned extension set shared with a concrete Page Builder consumer surface.
///
/// `granted_permissions` must come from the authenticated host permission snapshot. It is used for
/// contribution discovery only; every owner transport must still enforce authorization itself.
#[derive(Clone, Default)]
pub struct PageBuilderContributionHostContext {
    extensions: Arc<Vec<PageBuilderContributionHostExtension>>,
    granted_permissions: Arc<BTreeSet<String>>,
}

impl PageBuilderContributionHostContext {
    pub fn new(extensions: Vec<PageBuilderContributionHostExtension>) -> Result<Self, String> {
        let mut modules = BTreeSet::new();
        let mut providers = BTreeSet::new();
        for extension in &extensions {
            let module_id = extension.module_id().trim();
            let provider = extension.owner_provider().trim();
            if module_id.is_empty() || provider.is_empty() {
                return Err(
                    "Page Builder contribution host extension requires module/provider identity"
                        .to_string(),
                );
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
            granted_permissions: Arc::new(BTreeSet::new()),
        })
    }

    pub fn with_granted_permissions(
        mut self,
        permissions: impl IntoIterator<Item = String>,
    ) -> Self {
        self.granted_permissions = Arc::new(
            permissions
                .into_iter()
                .map(|permission| permission.trim().to_string())
                .filter(|permission| !permission.is_empty())
                .collect(),
        );
        self
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

    pub fn granted_permissions(&self) -> BTreeSet<String> {
        self.granted_permissions.as_ref().clone()
    }

    pub fn install_registries(&self, registries: &mut RegistrySet) -> Result<(), String> {
        for extension in self.extensions.iter() {
            extension.install(registries)?;
        }
        Ok(())
    }

    /// Merge host extensions into an existing consumer-owned contribution assembly.
    ///
    /// Consumer contributions stay authoritative in `base`. External manifests pass their own
    /// module/provider/capability/permission filters, and registry conflicts fail closed as
    /// assembly diagnostics rather than silently replacing consumer contracts.
    pub fn merge_admin_assembly(
        &self,
        base: Option<Arc<ContributionAssemblyResult>>,
        capabilities: BTreeSet<String>,
    ) -> Arc<ContributionAssemblyResult> {
        let mut result = base
            .as_deref()
            .cloned()
            .unwrap_or_else(ContributionAssemblyResult::default);
        if self.extensions.is_empty() {
            return Arc::new(result);
        }

        let extension_result = build_admin_contribution_registry_from_manifests(
            self.manifests(),
            &ContributionAssemblyPolicy {
                enabled_modules: self.module_ids(),
                enabled_providers: self.owner_providers(),
                capabilities,
                permissions: self.granted_permissions(),
                ..ContributionAssemblyPolicy::default()
            },
        );
        result
            .diagnostics
            .extend(extension_result.diagnostics.iter().cloned());
        result.skipped_contributions = result
            .skipped_contributions
            .saturating_add(extension_result.skipped_contributions);

        for (_, contribution) in extension_result.registry.iter() {
            match result.registry.register(contribution.clone()) {
                Ok(()) => {
                    result.registered_contributions =
                        result.registered_contributions.saturating_add(1);
                }
                Err(error) => {
                    result.skipped_contributions = result.skipped_contributions.saturating_add(1);
                    result.diagnostics.push(ContributionAssemblyDiagnostic {
                        severity: ContributionAssemblySeverity::Error,
                        code: "contribution_host_registry_conflict".to_string(),
                        module_id: None,
                        contribution_id: Some(contribution.id.clone()),
                        message: error.to_string(),
                    });
                }
            }
        }

        Arc::new(result)
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

    pub fn property_port(
        &self,
        provider: &str,
    ) -> Option<Arc<dyn PageBuilderContributionPropertyPort>> {
        let provider = provider.trim();
        self.extensions
            .iter()
            .find(|extension| extension.owner_provider().trim() == provider)
            .and_then(|extension| extension.property_port.clone())
    }

    pub fn is_empty(&self) -> bool {
        self.extensions.is_empty()
    }
}
