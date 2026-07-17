use crate::editor::{
    AdminEditorRuntime, AssetSection, CapabilityFieldset, DiagnosticsSection, PropertiesSection,
    StyleSection,
};
use fly_ui::EditorCapability;
use leptos::prelude::*;

#[component]
pub fn PropertiesAssetsPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    let properties_gate_runtime = runtime.clone();
    let properties_runtime = runtime.clone();
    let style_gate_runtime = runtime.clone();
    let style_runtime = runtime.clone();
    let asset_gate_runtime = runtime.clone();
    let asset_runtime = runtime.clone();
    let diagnostics_runtime = runtime;

    view! {
        <aside class="space-y-4 overflow-auto rounded-xl border border-border bg-card p-3">
            <CapabilityFieldset
                runtime=properties_gate_runtime
                capability=EditorCapability::Properties
            >
                <PropertiesSection runtime=properties_runtime />
            </CapabilityFieldset>
            <CapabilityFieldset
                runtime=style_gate_runtime
                capability=EditorCapability::Styles
            >
                <StyleSection runtime=style_runtime />
            </CapabilityFieldset>
            <CapabilityFieldset
                runtime=asset_gate_runtime
                capability=EditorCapability::Assets
            >
                <AssetSection runtime=asset_runtime />
            </CapabilityFieldset>
            <DiagnosticsSection runtime=diagnostics_runtime />
        </aside>
    }
}
