use crate::editor::{AdminEditorRuntime, DiagnosticsSection};
#[cfg(target_arch = "wasm32")]
use crate::editor::{AssetSection, CapabilityFieldset, PropertiesSection, StyleSection};
#[cfg(target_arch = "wasm32")]
use fly_ui::EditorCapability;
use leptos::prelude::*;

#[component]
pub fn PropertiesAssetsPanel(runtime: AdminEditorRuntime) -> impl IntoView {
    #[cfg(target_arch = "wasm32")]
    {
        let properties_gate_runtime = runtime.clone();
        let properties_runtime = runtime.clone();
        let style_gate_runtime = runtime.clone();
        let style_runtime = runtime.clone();
        let asset_gate_runtime = runtime.clone();
        let asset_runtime = runtime.clone();
        let diagnostics_runtime = runtime;

        view! {
            <aside
                class="space-y-4 overflow-auto rounded-xl border border-border bg-card p-3"
                data-fly-property-surface="wasm"
            >
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
        .into_any()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        view! {
            <aside
                class="space-y-4 overflow-auto rounded-xl border border-border bg-card p-3"
                data-fly-property-surface="ssr-diagnostics"
            >
                <DiagnosticsSection runtime=runtime />
            </aside>
        }
        .into_any()
    }
}
