# Page Builder current-only cleanup failure

```text
obsolete Page Builder API markers remain:
crates/fly/src/runtime_scenario_snapshot.rs:8: pub const FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1: &str = "fly_runtime_scenario_render_snapshot_v1";
crates/fly/src/runtime_scenario_snapshot.rs:71: let format = FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1.to_string();
crates/fly/src/runtime_scenario_snapshot.rs:85: self.format == FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1
crates/fly/src/trait_model.rs:129: "schemaVersion" => component.schema_version.clone().map(Value::String),
crates/fly/src/fragment.rs:12: pub schema_version: Option<String>,
crates/fly/src/fragment.rs:43: .or_insert_with(|| component.schema_version.clone());
crates/fly/src/fragment.rs:54: .map(|(provider, schema_version)| ProviderRequirement {
crates/fly/src/fragment.rs:56: schema_version,
crates/fly/src/fragment.rs:126: pub schema_version: String,
crates/fly/src/binding.rs:361: "schemaVersion" => component.schema_version = value.as_str().map(ToString::to_string),
crates/fly/src/validation.rs:253: if component.provider.is_some() && component.schema_version.is_none() {
crates/fly/src/validation.rs:256: "missing_provider_schema_version",
crates/fly/src/validation.rs:258: "provider-owned component should carry schemaVersion",
crates/fly/src/runtime_scenario_release.rs:5: ValidationSeverity, FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1,
crates/fly/src/runtime_scenario_release.rs:10: pub const FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1: &str =
crates/fly/src/runtime_scenario_release.rs:32: format: FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1.to_string(),
crates/fly/src/runtime_scenario_release.rs:63: if self.format != FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1 {
crates/fly/src/runtime_scenario_release.rs:68: "runtime scenario baseline format `{}` is unsupported; expected `{FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1}`",
crates/fly/src/runtime_scenario_release.rs:110: if self.snapshot.format != FLY_RUNTIME_SCENARIO_RENDER_SNAPSHOT_V1 {
crates/fly/src/command/patch.rs:8: const SCHEMA_VERSION_FIELD: &str = "schemaVersion";
crates/fly/src/command/patch.rs:61: pub fn set_schema_version(mut self, value: impl Into<String>) -> Self {
crates/fly/src/command/patch.rs:66: pub fn clear_schema_version(mut self) -> Self {
crates/fly/src/command/patch.rs:158: SCHEMA_VERSION_FIELD => component.schema_version = None,
crates/fly/src/command/patch.rs:172: component.schema_version = value.as_str().map(ToString::to_string)
crates/fly/src/command/patch.rs:226: .set_schema_version("2")
crates/fly/src/command/patch.rs:231: assert_eq!(component.schema_version.as_deref(), Some("2"));
crates/fly/src/command/patch.rs:238: .clear_schema_version()
crates/fly/src/command/patch.rs:243: assert!(component.schema_version.is_none());
crates/fly-ui/src/contribution.rs:18: pub schema_version: String,
crates/fly-ui/src/contribution.rs:28: pub schema_version: String,
crates/fly-ui/src/contribution.rs:118: let schema_version = schema_version.trim();
crates/fly-ui/src/contribution.rs:129: && renderer.schema_version == schema_version
crates/fly-ui/src/contribution.rs:147: let schema_version = schema_version.trim();
crates/fly-ui/src/contribution.rs:158: && editor.schema_version == schema_version
crates/fly-ui/src/contribution.rs:177: && registered.schema_version == renderer.schema_version
crates/fly-ui/src/contribution.rs:202: && registered.schema_version == editor.schema_version)
crates/fly-ui/src/contribution.rs:254: renderer.schema_version = required_value(
crates/fly-ui/src/contribution.rs:255: &renderer.schema_version,
crates/fly-ui/src/contribution.rs:257: "renderer schema_version",
crates/fly-ui/src/contribution.rs:304: editor.schema_version = required_value(
crates/fly-ui/src/contribution.rs:305: &editor.schema_version,
crates/fly-ui/src/contribution.rs:307: "property editor schema_version",
crates/fly-ui/src/contribution.rs:417: renderer.schema_version,
crates/fly-ui/src/contribution.rs:425: editor.provider, editor.component_type, editor.schema_version
crates/fly-ui/src/contribution_adapter.rs:12: pub schema_version: &'a str,
crates/fly-ui/src/contribution_adapter.rs:22: pub schema_version: &'a str,
crates/fly-ui/src/contribution_adapter.rs:58: request.schema_version,
crates/fly-ui/src/contribution_adapter.rs:66: request.schema_version,
crates/fly-ui/src/contribution_adapter.rs:86: request.schema_version,
crates/fly-ui/src/contribution_adapter.rs:93: request.schema_version,
crates/fly-ui/src/contribution_adapter.rs:108: schema_version.trim(),
crates/fly-ui/src/contribution_adapter.rs:121: schema_version.trim()
crates/fly-leptos/src/lib.rs:10: pub const FLY_IFRAME_PROTOCOL_V1: &str = "fly_iframe_v1";
crates/fly-leptos/src/lib.rs:382: protocol: FLY_IFRAME_PROTOCOL_V1.to_string(),
crates/fly-leptos/src/lib.rs:392: self.protocol == FLY_IFRAME_PROTOCOL_V1
crates/rustok-page-builder/src/lib.rs:112: assert!(encoded.get("schema_version").is_none());
crates/rustok-page-builder/src/lib.rs:117: "schema_version": "grapesjs",
crates/rustok-page-builder/src/service.rs:482: validate_project_payload(&input.page_id, &input.schema_version, &input.project_data)?;
crates/rustok-page-builder/src/service.rs:557: validate_project_payload(&input.page_id, &input.schema_version, &input.project_data)?;
crates/rustok-page-builder/src/service.rs:624: validate_project_payload(&input.page_id, &input.schema_version, &input.project_data)?;
crates/rustok-page-builder/src/service.rs:667: validate_project_payload(&input.page_id, &input.schema_version, &input.project_data)?;
crates/rustok-page-builder/src/service.rs:1032: legacy_bridge_readonly: false,
crates/rustok-page-builder/admin/src/palette_access.rs:165: protocol: fly_browser::FLY_BROWSER_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/browser_intent.rs:383: use fly_browser::FLY_BROWSER_PROTOCOL_V1;
crates/rustok-page-builder/admin/src/browser_intent.rs:413: protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/ssr_actions_forms_browser_tests.rs:2: use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL_V1};
crates/rustok-page-builder/admin/src/ssr_actions_forms_browser_tests.rs:38: protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/capability_access.rs:267: use fly_browser::FLY_BROWSER_PROTOCOL_V1;
crates/rustok-page-builder/admin/src/capability_access.rs:272: protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/ssr_assets_browser_tests.rs:3: use fly_browser::{BrowserIntentEnvelope, BrowserIntentKind, FLY_BROWSER_PROTOCOL_V1};
crates/rustok-page-builder/admin/src/ssr_assets_browser_tests.rs:37: protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/editor/canvas_document.rs:6: use fly_leptos::FLY_IFRAME_PROTOCOL_V1;
crates/rustok-page-builder/admin/src/editor/canvas_document.rs:64: let protocol = serde_json::to_string(FLY_IFRAME_PROTOCOL_V1)
crates/rustok-page-builder/admin/src/editor/context_compatibility_panel.rs:6: RuntimeContextMigrationResult, RuntimeContractCompatibility, FLY_RUNTIME_CONTEXT_CONTRACT_V1,
crates/rustok-page-builder/admin/src/editor/context_compatibility_panel.rs:81: Ok(snapshot) if snapshot.format == FLY_RUNTIME_CONTEXT_CONTRACT_V1 => {
crates/rustok-page-builder/admin/src/editor/runtime_scenario_regression.rs:6: RuntimeScenarioRenderChangeImpact, FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1,
crates/rustok-page-builder/admin/src/editor/runtime_scenario_regression.rs:142: == FLY_RUNTIME_SCENARIO_RELEASE_BASELINE_V1
crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs:2: use fly_leptos::{BrowserPoint, PointerSample, FLY_IFRAME_PROTOCOL_V1};
crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs:52: self.protocol == FLY_IFRAME_PROTOCOL_V1
crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs:78: protocol: FLY_IFRAME_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs:93: protocol: FLY_IFRAME_PROTOCOL_V1.to_string(),
crates/rustok-page-builder/admin/src/editor/canvas_protocol.rs:108: protocol: FLY_IFRAME_PROTOCOL_V1.to_string(),
crates/rustok-pages/admin/src/browser_intent.rs:7: use fly_browser::{BrowserIntentEnvelope, FLY_BROWSER_PROTOCOL_V1};
crates/rustok-pages/admin/src/browser_intent.rs:140: protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
crates/rustok-pages/admin/src/contribution_browser_intent.rs:101: use fly_browser::FLY_BROWSER_PROTOCOL_V1;
crates/rustok-pages/admin/src/contribution_browser_intent.rs:107: protocol: FLY_BROWSER_PROTOCOL_V1.to_string(),
```
