from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SKIP = {".git", "target", "node_modules", ".next", "dist", "build"}
TEXT_SUFFIXES = {".rs", ".md", ".mjs", ".js", ".ts", ".tsx", ".json", ".toml", ".yml", ".yaml", ".sh", ".ps1", ".dart", ".txt"}

REPLACEMENTS = {
    "GrapesJsV1Codec": "GrapesJsCodec",
    "GrapesJsV1": "GrapesJs",
    "GRAPESJS_V1": "GRAPESJS_FORMAT",
    "grapesjs_v1": "grapesjs",
    "FLY_FRAGMENT_V1": "FLY_FRAGMENT_FORMAT",
    "fly_fragment_v1": "fly_fragment",
    "RICH_TEXT_PAYLOAD_V1": "RICH_TEXT_PAYLOAD_FORMAT",
    "fly_rich_text_payload_v1": "fly_rich_text_payload",
    "FLY_PROJECT_BUNDLE_V1": "FLY_PROJECT_BUNDLE_FORMAT",
    "fly_project_bundle_v1": "fly_project_bundle",
}


def files(root: Path):
    if not root.exists():
        return
    for path in root.rglob("*"):
        if path.is_file() and not any(part in SKIP for part in path.parts):
            if path.suffix.lower() in TEXT_SUFFIXES or path.name in {"Cargo.toml", "package.json"}:
                yield path


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def replace_all(path: Path, replacements: dict[str, str] = REPLACEMENTS) -> None:
    text = read(path)
    updated = text
    for old, new in replacements.items():
        updated = updated.replace(old, new)
    if updated != text:
        write(path, updated)


def remove_function(path: Path, name: str) -> None:
    text = read(path)
    match = re.search(rf"(?m)^\s*(?:#\[[^\n]+\]\s*)*(?:pub(?:\([^)]*\))?\s+)?fn\s+{re.escape(name)}\b", text)
    if not match:
        return
    start = match.start()
    brace = text.find("{", match.end())
    if brace < 0:
        return
    depth = 0
    end = brace
    for index in range(brace, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    while end < len(text) and text[end] == "\n":
        end += 1
    write(path, text[:start] + text[end:])


def remove_json_keys(value):
    if isinstance(value, dict):
        for key in list(value):
            if key in {"schema_version", "schemaVersion", "provider_version", "contract_version", "builder_contract_version", "consumer_min_version", "minimum_version", "legacy_bridge_readonly", "bundle_format", "project_format"}:
                del value[key]
            else:
                remove_json_keys(value[key])
    elif isinstance(value, list):
        for item in value:
            remove_json_keys(item)


# Rename current APIs and external adapter names throughout the affected ecosystem.
for root in [ROOT / "crates/fly", ROOT / "crates/fly-ui", ROOT / "crates/fly-leptos", ROOT / "crates/rustok-page-builder", ROOT / "crates/rustok-page-builder-storefront", ROOT / "crates/rustok-pages", ROOT / "scripts", ROOT / "docs", ROOT / "DECISIONS"]:
    for path in files(root) or []:
        replace_all(path)

# One ProjectDocument shape; the adapter format is not part of the domain object.
model = ROOT / "crates/fly/src/model.rs"
text = read(model)
text = text.replace("use crate::{FlyError, FlyResult, IdGenerator, ProjectHash, GRAPESJS_FORMAT};", "use crate::{FlyError, FlyResult, IdGenerator, ProjectHash};")
text = re.sub(r"#\[derive\(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq\)\]\n#\[serde\(rename_all = \"snake_case\"\)\]\npub enum ProjectFormat \{[\s\S]*?\n\}\n\nimpl ProjectFormat \{[\s\S]*?\n\}\n\n", "", text, count=1)
text = text.replace("    pub format: ProjectFormat,\n", "")
text = text.replace("        Self {\n            format: ProjectFormat::GrapesJs,\n            project,\n        }", "        Self { project }")
text = re.sub(r"\n    #\[serde\(\n        rename = \"schemaVersion\",\n        default,\n        skip_serializing_if = \"Option::is_none\"\n    \)\]\n    pub schema_version: Option<String>,", "", text)
text = text.replace("            schema_version: None,\n", "")
write(model, text)

# Component patch only exposes current component fields.
patch = ROOT / "crates/fly/src/command/patch.rs"
text = read(patch)
text = text.replace('const SCHEMA_VERSION_FIELD: &str = "schemaVersion";\n', "")
text = re.sub(r"\n\s*SCHEMA_VERSION_FIELD => component\.schema_version = None,", "", text)
text = re.sub(r"\n\s*SCHEMA_VERSION_FIELD => \{\n\s*component\.schema_version = value\.as_str\(\)\.map\(ToString::to_string\)\n\s*\}", "", text)
text = text.replace("            .set_schema_version(\"2\")\n", "")
text = text.replace("        assert_eq!(component.schema_version.as_deref(), Some(\"2\"));\n", "")
text = text.replace("            .clear_schema_version()\n", "")
text = text.replace("        assert!(component.schema_version.is_none());\n", "")
write(patch, text)
remove_function(patch, "set_schema_version")
remove_function(patch, "clear_schema_version")

# Current fragments contain only useful authoring data.
fragment = ROOT / "crates/fly/src/fragment.rs"
write(fragment, '''use crate::{
    ComponentNode, EditorCommand, FlyEditor, FlyError, FlyResult, IdGenerator, ProjectDocument,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRequirement {
    pub provider: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectFragment {
    pub components: Vec<ComponentNode>,
    #[serde(default)]
    pub styles: Vec<Value>,
    #[serde(default)]
    pub assets: Vec<Value>,
    #[serde(default)]
    pub provider_requirements: Vec<ProviderRequirement>,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl ProjectFragment {
    pub fn from_component(document: &ProjectDocument, component_id: &str) -> FlyResult<Self> {
        let component = document
            .component(component_id)
            .ok_or_else(|| FlyError::ComponentNotFound(component_id.to_string()))?;
        let node = ComponentNode::Object(Box::new(component.clone()));
        let mut requirements = BTreeSet::new();
        node.visit(0, "fragment.components[0]", &mut |component, _, _| {
            if let Some(provider) = component.provider.as_ref() {
                requirements.insert(provider.clone());
            }
        });
        Ok(Self {
            components: vec![node],
            styles: document.project.styles.clone(),
            assets: document.project.assets.clone(),
            provider_requirements: requirements
                .into_iter()
                .map(|provider| ProviderRequirement { provider })
                .collect(),
            extensions: Map::new(),
        })
    }

    pub fn remap_ids(&mut self, generator: &mut impl IdGenerator) -> BTreeMap<String, String> {
        let mut source_ids = Vec::new();
        for component in &self.components {
            component.collect_ids(&mut source_ids);
        }
        let mapping = source_ids
            .into_iter()
            .map(|source| (source, generator.next_id("paste")))
            .collect::<BTreeMap<_, _>>();
        for component in &mut self.components {
            component.remap_ids(&mapping);
        }
        for style in &mut self.styles {
            replace_value_references(style, &mapping);
        }
        for asset in &mut self.assets {
            replace_value_references(asset, &mapping);
        }
        replace_map_references(&mut self.extensions, &mapping);
        mapping
    }

    pub fn insert(
        mut self,
        editor: &mut FlyEditor,
        parent_id: Option<String>,
        index: usize,
    ) -> FlyResult<Vec<String>> {
        let mut staged = editor.clone();
        self.remap_ids(&mut staged.id_generator);
        let mut inserted_ids = Vec::new();
        let commands = self
            .components
            .into_iter()
            .enumerate()
            .map(|(offset, component)| {
                if let Some(id) = component.id() {
                    inserted_ids.push(id.to_string());
                }
                EditorCommand::Insert {
                    parent_id: parent_id.clone(),
                    index: index + offset,
                    component,
                }
            })
            .collect::<Vec<_>>();
        if commands.is_empty() {
            return Ok(inserted_ids);
        }
        staged.apply(EditorCommand::batch(commands))?;
        *editor = staged;
        Ok(inserted_ids)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RichTextPayload {
    pub capability: String,
    pub payload: Value,
    #[serde(flatten)]
    pub extensions: Map<String, Value>,
}

impl RichTextPayload {
    pub fn opaque(capability: impl Into<String>, payload: Value) -> Self {
        Self {
            capability: capability.into(),
            payload,
            extensions: Map::new(),
        }
    }
}

fn replace_map_references(map: &mut Map<String, Value>, mapping: &BTreeMap<String, String>) {
    for value in map.values_mut() {
        replace_value_references(value, mapping);
    }
}

fn replace_value_references(value: &mut Value, mapping: &BTreeMap<String, String>) {
    match value {
        Value::String(string) => {
            if let Some(replacement) = mapping.get(string) {
                *string = replacement.clone();
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_value_references(value, mapping);
            }
        }
        Value::Object(map) => replace_map_references(map, mapping),
        _ => {}
    }
}
''')

# Bundle has one active representation and no format selectors.
bundle = ROOT / "crates/fly/src/bundle.rs"
text = read(bundle)
text = text.replace("    ProjectDocument, ProjectHash, RegistrySet, ValidationLimits, ValidationReport, GRAPESJS_FORMAT,\n", "    ProjectDocument, ProjectHash, RegistrySet, ValidationLimits, ValidationReport,\n")
text = re.sub(r"pub const FLY_PROJECT_BUNDLE_FORMAT: &str = \"fly_project_bundle\";\n\n", "", text)
for line in ["    pub bundle_format: String,\n", "    pub project_format: String,\n", "    pub require_supported_project_format: bool,\n", "            require_supported_project_format: true,\n", "        bundle_format: FLY_PROJECT_BUNDLE_FORMAT.to_string(),\n", "        project_format: GRAPESJS_FORMAT.to_string(),\n", "            bundle_format: FLY_PROJECT_BUNDLE_FORMAT.to_string(),\n", "            project_format: GRAPESJS_FORMAT.to_string(),\n", "    pub bundle_format: String,\n", "    pub project_format: String,\n"]:
    text = text.replace(line, "")
text = text.replace('!object.contains_key("bundle_format")', '!object.contains_key("project_hash")')
text = re.sub(r"\n    if bundle\.bundle_format != FLY_PROJECT_BUNDLE_FORMAT \{[\s\S]*?\n    \}", "", text)
text = re.sub(r"\n    if policy\.require_supported_project_format && bundle\.project_format != GRAPESJS_FORMAT \{[\s\S]*?\n    \}", "", text)
text = text.replace("        bundle_format: decoded.bundle.bundle_format.clone(),\n", "")
text = text.replace("        project_format: decoded.bundle.project_format.clone(),\n", "")
text = re.sub(r"^\s*assert_eq!\([^\n]*(?:bundle_format|project_format)[^\n]*\);\n", "", text, flags=re.M)
write(bundle, text)

# Registry compatibility is component/provider based only.
registry = ROOT / "crates/fly/src/registry.rs"
text = read(registry)
text = re.sub(r"^\s*pub schema_version: String,\n", "", text, flags=re.M)
text = re.sub(r"^\s*pub version: String,\n", "", text, flags=re.M)
text = re.sub(r"\n\s*#\[serde\(default, skip_serializing_if = \"Option::is_none\"\)\]\n\s*pub minimum_version: Option<String>,", "", text)
text = re.sub(r"^\s*(?:schema_version|version|minimum_version): [^\n]+,\n", "", text, flags=re.M)
write(registry, text)

landing_contract = ROOT / "crates/fly/src/landing_contract.rs"
text = read(landing_contract)
text = re.sub(r"\n\s*#\[serde\(default, skip_serializing_if = \"Option::is_none\"\)\]\n\s*pub schema_version: Option<String>,", "", text)
text = re.sub(r"^\s*schema_version: [^\n]+,\n", "", text, flags=re.M)
text = re.sub(r"\n\s*if required\n\s*\.schema_version[\s\S]*?\n\s*\}\n", "\n", text)
text = text.replace("    SchemaVersionMismatch,\n", "")
write(landing_contract, text)

# Page Builder service uses the single runtime telemetry module.
service = ROOT / "crates/rustok-page-builder/src/service.rs"
text = read(service)
text = text.replace("    PageBuilderCapabilityResponse, PageBuilderContractMetadata, PageBuilderErrorKind,", "    PageBuilderCapabilityResponse, PageBuilderErrorKind,")
start = text.find("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]\n#[serde(rename_all = \"snake_case\")]\npub enum PageBuilderAdapterOperation")
end_marker = "impl PageBuilderAdapterTelemetry for NoopPageBuilderAdapterTelemetry {\n    fn record_adapter_call(&self, _evidence: &PageBuilderAdapterCallEvidence) {}\n}\n\n"
end = text.find(end_marker, start)
if start >= 0 and end >= 0:
    text = text[:start] + text[end + len(end_marker):]
text = text.replace("use serde::Serialize;\n", "use serde::Serialize;\nuse crate::runtime_telemetry::{NoopPageBuilderRuntimeTelemetry, PageBuilderRuntimeCallEvidence, PageBuilderRuntimeTelemetry};\n")
text = text.replace("PageBuilderAdapterCallEvidence", "PageBuilderRuntimeCallEvidence")
text = text.replace("PageBuilderAdapterTelemetry", "PageBuilderRuntimeTelemetry")
text = text.replace("NoopPageBuilderAdapterTelemetry", "NoopPageBuilderRuntimeTelemetry")
text = text.replace("validate_grapesjs_payload", "validate_project_payload")
text = re.sub(r"validate_project_payload\(\s*&input\.page_id,\s*&input\.schema_version,\s*&input\.project_data,\s*\)\?;", "validate_project_payload(&input.page_id, &input.project_data)?;", text)
text = re.sub(r"fn validate_project_payload\(\n    page_id: &str,\n    schema_version: &str,\n    project_data: &serde_json::Value,\n\) -> PageBuilderServiceResult<\(\)> \{\n    validate_non_empty\(\"page_id\", page_id\)\?;\n    if schema_version != [\s\S]*?\n    \}\n    ensure_object_payload\(\"project_data\", project_data\)\n\}", "fn validate_project_payload(\n    page_id: &str,\n    project_data: &serde_json::Value,\n) -> PageBuilderServiceResult<()> {\n    validate_non_empty(\"page_id\", page_id)?;\n    ensure_object_payload(\"project_data\", project_data)\n}", text)
text = re.sub(r"^\s*schema_version: [^\n]+,\n", "", text, flags=re.M)
write(service, text)

# Fly adapter exposes only decode/decode_with without selector arguments.
adapters = ROOT / "crates/rustok-page-builder/src/adapters.rs"
text = read(adapters)
text = text.replace("PageBuilderContractMetadata", "PageBuilderModuleMetadata")
text = re.sub(r"pub fn decode\(schema_version: &str, project_data: &Value\)", "pub fn decode(project_data: &Value)", text)
text = re.sub(r"Self::decode_with\(\n\s*schema_version,\n", "Self::decode_with(\n", text)
text = re.sub(r"\n\s*schema_version: &str,", "", text)
text = re.sub(r"\n\s*let expected = PageBuilderModuleMetadata::BASELINE\.contract;[\s\S]*?\n\s*\}\n", "\n", text, count=1)
text = re.sub(r"\n\s*UnsupportedSchema \{[\s\S]*?\n\s*\},", "", text)
text = re.sub(r"\n\s*Self::UnsupportedSchema \{[\s\S]*?\n\s*\),", "", text)
text = text.replace("self.validate_project(&input.schema_version, &input.project_data)?;", "self.validate_project(&input.project_data)?;")
text = re.sub(r"fn validate_project\(\n\s*&self,\n\s*schema_version: &str,", "fn validate_project(\n        &self,", text)
text = re.sub(r"FlyProjectInspection::decode_with\(\n\s*schema_version,\n", "FlyProjectInspection::decode_with(\n", text)
text = re.sub(r"FlyProjectInspection::decode\(\"[^\"]+\", &", "FlyProjectInspection::decode(&", text)
write(adapters, text)
remove_function(adapters, "fly_inspection_rejects_contract_drift")

# Remove selector fields from every Page Builder producer/test.
for root in [ROOT / "crates/rustok-page-builder", ROOT / "crates/rustok-pages"]:
    for path in root.rglob("*.rs"):
        text = read(path)
        text = re.sub(r"^\s*schema_version: [^\n]+,\n", "", text, flags=re.M)
        text = text.replace("PageBuilderContractMetadata", "PageBuilderModuleMetadata")
        write(path, text)

# Rollout has only active capability flags.
rollout = ROOT / "crates/rustok-page-builder/src/rollout.rs"
text = read(rollout)
text = re.sub(r"^\s*pub legacy_bridge_readonly: bool,\n", "", text, flags=re.M)
text = re.sub(r"^\s*legacy_bridge_readonly: (?:true|false),\n", "", text, flags=re.M)
text = text.replace("pub fn atomic_flag_keys() -> [&'static str; 5]", "pub fn atomic_flag_keys() -> [&'static str; 4]")
text = text.replace('        "builder.legacy_bridge_readonly",\n', "")
write(rollout, text)

# Remove abandoned compatibility/migration source files and exports.
for path in [
    ROOT / "crates/fly/src/context_compatibility.rs",
    ROOT / "crates/fly/src/context_migration.rs",
    ROOT / "crates/rustok-page-builder/src/runtime_context_compatibility.rs",
    ROOT / "crates/rustok-page-builder/src/runtime_context_migration.rs",
]:
    if path.exists():
        path.unlink()
for path in [ROOT / "crates/fly/src/lib.rs", ROOT / "crates/rustok-page-builder/src/lib.rs"]:
    text = read(path)
    text = re.sub(r"^.*(?:context_compatibility|context_migration|runtime_context_compatibility|runtime_context_migration).*$\n", "", text, flags=re.M)
    write(path, text)

# Machine-readable Page Builder artifacts expose current data only.
for root in [ROOT / "crates/rustok-page-builder", ROOT / "crates/fly"]:
    for path in root.rglob("*.json"):
        try:
            value = json.loads(read(path))
        except Exception:
            continue
        remove_json_keys(value)
        write(path, json.dumps(value, ensure_ascii=False, indent=2) + "\n")

obsolete = ROOT / "crates/rustok-page-builder/scripts/verify/verify-page-builder-pages-legacy-bridge.mjs"
if obsolete.exists():
    obsolete.unlink()
package = ROOT / "package.json"
value = json.loads(read(package))
value.get("scripts", {}).pop("verify:page-builder:pages:legacy-bridge", None)
write(package, json.dumps(value, ensure_ascii=False, indent=2) + "\n")

failure = ROOT / "PAGE_BUILDER_CLEANUP_FAILURE.md"
if failure.exists():
    failure.unlink()
