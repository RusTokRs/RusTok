mod admin_shell;
mod authoring;
mod binding_panel;
mod canvas_document;
#[cfg(any(target_arch = "wasm32", test))]
mod canvas_protocol;
mod context_compatibility_panel;
mod context_contract_tools;
mod context_dependency_panel;
mod context_schema_panel;
mod dynamic_runtime;
mod isolated_canvas;
mod modular_canvas;
mod page_manager;
mod palette_layers;
mod properties_assets;
mod resize_handles;
mod responsive_styles;
mod runtime;
mod runtime_publish_gate;
mod runtime_scenario_matrix;
mod runtime_scenario_regression;
mod runtime_scenarios;
mod selection_commands;
mod shortcut_dispatch;
mod ssr_drop;
mod ssr_forms;
mod ssr_inspector;
mod ssr_locale;
mod toolbar;
mod trait_panel;

pub use admin_shell::AdminShell;
pub use authoring::{
    CanvasComponentGeometry, LayerItemView, PaletteBlockView, SelectedComponentView,
};
pub(crate) use binding_panel::BindingPanel;
pub(crate) use canvas_document::render_canvas_srcdoc_with_context;
#[cfg(target_arch = "wasm32")]
pub(crate) use canvas_protocol::decode_canvas_message;
#[cfg(target_arch = "wasm32")]
pub(crate) use canvas_protocol::CanvasBridgeMessage;
pub(crate) use context_compatibility_panel::ContextCompatibilityPanel;
pub(crate) use context_contract_tools::ContextContractToolsPanel;
pub(crate) use context_dependency_panel::ContextDependencyPanel;
pub(crate) use context_schema_panel::ContextSchemaPanel;
pub(crate) use dynamic_runtime::DynamicRuntimePanel;
pub(crate) use isolated_canvas::IsolatedAuthoringCanvas;
pub use modular_canvas::AdminCanvas;
pub(crate) use page_manager::PageManagerPanel;
pub(crate) use palette_layers::PaletteLayersPanel;
pub(crate) use properties_assets::PropertiesAssetsPanel;
pub(crate) use resize_handles::ResizeHandles;
pub(crate) use responsive_styles::ResponsiveStylePanel;
pub(crate) use runtime::AdminEditorRuntime;
pub(crate) use runtime_publish_gate::RuntimePublishGatePanel;
pub(crate) use runtime_scenario_matrix::RuntimeScenarioMatrixPanel;
pub(crate) use runtime_scenario_regression::RuntimeScenarioRegressionPanel;
pub(crate) use runtime_scenarios::RuntimeScenarioPanel;
pub(crate) use shortcut_dispatch::dispatch_shortcut;
pub(crate) use ssr_drop::{SsrDropRequest, SsrDropSource};
pub(crate) use ssr_forms::{
    SsrComponentPropertyKind, SsrComponentPropertyRequest, SsrPageCreateRequest,
    SsrPageMetadataRequest, SsrPageRenameRequest,
};
pub(crate) use ssr_inspector::SsrInspectorPanel;
pub(crate) use ssr_locale::SsrLocalePanel;
pub(crate) use toolbar::AuthoringToolbar;
pub(crate) use trait_panel::TraitPanel;
