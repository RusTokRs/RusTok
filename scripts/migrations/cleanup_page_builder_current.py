from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

SKIP_DIRS = {".git", "target", "node_modules", ".next", "dist", "build"}
TEXT_SUFFIXES = {
    ".rs", ".md", ".mjs", ".js", ".ts", ".tsx", ".json", ".toml", ".yml", ".yaml",
    ".sh", ".ps1", ".dart", ".txt",
}

GLOBAL_REPLACEMENTS = {
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

CURRENT_ROOTS = (
    ROOT / "crates" / "fly",
    ROOT / "crates" / "fly-ui",
    ROOT / "crates" / "fly-leptos",
    ROOT / "crates" / "rustok-page-builder",
    ROOT / "crates" / "rustok-page-builder-storefront",
    ROOT / "crates" / "rustok-pages" / "admin",
    ROOT / "crates" / "rustok-pages" / "storefront",
)


def iter_text_files(root: Path):
    for path in root.rglob("*"):
        if not path.is_file():
            continue
        if any(part in SKIP_DIRS for part in path.parts):
            continue
        if path.suffix.lower() in TEXT_SUFFIXES or path.name in {"Cargo.toml", "package.json"}:
            yield path


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def replace_all(path: Path, replacements: dict[str, str]) -> None:
    text = read(path)
    updated = text
    for old, new in replacements.items():
        updated = updated.replace(old, new)
    if updated != text:
        write(path, updated)


def remove_regex(path: Path, pattern: str, *, flags: int = 0) -> None:
    text = read(path)
    updated = re.sub(pattern, "", text, flags=flags)
    if updated != text:
        write(path, updated)


def replace_regex(path: Path, pattern: str, replacement: str, *, flags: int = 0) -> None:
    text = read(path)
    updated = re.sub(pattern, replacement, text, flags=flags)
    if updated != text:
        write(path, updated)


def remove_function(path: Path, function_name: str) -> None:
    text = read(path)
    marker = f"    fn {function_name}"
    start = text.find(marker)
    if start < 0:
        return
    attr_start = text.rfind("    #[", 0, start)
    if attr_start >= 0 and text[attr_start:start].strip().startswith("#["):
        start = attr_start
    brace = text.find("{", start)
    if brace < 0:
        return
    depth = 0
    end = brace
    for index in range(brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                end = index + 1
                break
    while end < len(text) and text[end] == "\n":
        end += 1
    write(path, text[:start] + text[end:])


def strip_json_keys(value):
    if isinstance(value, dict):
        for key in list(value):
            if key in {
                "schema_version", "schemaVersion", "contract_version", "builder_contract_version",
                "consumer_min_version", "minimum_version", "legacy_bridge_readonly",
            }:
                del value[key]
            else:
                strip_json_keys(value[key])
    elif isinstance(value, list):
        for item in value:
            strip_json_keys(item)


def cleanup_json(path: Path) -> None:
    try:
        value = json.loads(read(path))
    except Exception:
        return
    strip_json_keys(value)
    write(path, json.dumps(value, ensure_ascii=False, indent=2) + "\n")


# Rename the current APIs and external adapter identifiers everywhere they are referenced.
for path in iter_text_files(ROOT):
    replace_all(path, GLOBAL_REPLACEMENTS)

# DTO: one module metadata surface and request payloads without selectors.
dto = ROOT / "crates/rustok-page-builder/src/dto.rs"
text = read(dto)
text = text.replace("use fly::GRAPESJS_FORMAT;\n", "")
start = text.find("/// Compatibility selector accepted")
end = text.find("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]", start)
if start >= 0 and end >= 0:
    text = text[:start] + text[end:]
text = re.sub(
    r"\n    /// Compatibility-only field[^\n]*\n    #\[serde\([^\n]*\)\]\n    pub schema_version: String,",
    "",
    text,
)
text = re.sub(r"\n            schema_version: String::new\(\),", "", text)
text = text.replace(
    "/// The module version is `CARGO_PKG_VERSION`; the runtime contract itself is intentionally\n"
    "/// versionless and evolves additively inside the module major.\n",
    "/// Canonical metadata for the active Page Builder module API.\n",
)
write(dto, text)

# Rollout: remove the obsolete bridge flag entirely.
rollout = ROOT / "crates/rustok-page-builder/src/rollout.rs"
remove_regex(rollout, r"^\s*pub legacy_bridge_readonly: bool,\n", flags=re.M)
remove_regex(rollout, r"^\s*legacy_bridge_readonly: (?:true|false),\n", flags=re.M)
replace_regex(rollout, r"pub fn atomic_flag_keys\(\) -> \[&'static str; 5\]", "pub fn atomic_flag_keys() -> [&'static str; 4]")
remove_regex(rollout, r'^\s*"builder\.legacy_bridge_readonly",\n', flags=re.M)

# Fly registry and component model: provider identity is enough; no schema/plugin version matrix.
model = ROOT / "crates/fly/src/model.rs"
remove_regex(
    model,
    r"\n    #\[serde\(\n        rename = \"schemaVersion\",\n        default,\n        skip_serializing_if = \"Option::is_none\"\n    \)\]\n    pub schema_version: Option<String>,",
)
remove_regex(model, r"^\s*schema_version: None,\n", flags=re.M)

registry = ROOT / "crates/fly/src/registry.rs"
remove_regex(registry, r"^\s*pub schema_version: String,\n", flags=re.M)
remove_regex(registry, r"^\s*schema_version: [^\n]+,\n", flags=re.M)
remove_regex(registry, r"\n    #\[serde\(default, skip_serializing_if = \"Option::is_none\"\)\]\n    pub minimum_version: Option<String>,")
remove_regex(registry, r"^\s*pub version: String,\n", flags=re.M)
remove_regex(registry, r"^\s*minimum_version: [^\n]+,\n", flags=re.M)
remove_regex(registry, r"^\s*version: [^\n]+,\n", flags=re.M)

# Landing manifest: component type/provider only.
landing_contract = ROOT / "crates/fly/src/landing_contract.rs"
remove_regex(landing_contract, r"\n    #\[serde\(default, skip_serializing_if = \"Option::is_none\"\)\]\n    pub schema_version: Option<String>,")
remove_regex(landing_contract, r"^\s*schema_version: [^\n]+,\n", flags=re.M)
remove_regex(
    landing_contract,
    r"\n            if required\n                \.schema_version[\s\S]*?\n            \}\n",
)
remove_regex(landing_contract, r"^\s*SchemaVersionMismatch,\n", flags=re.M)

# Landing inspection: only canonical decode/decode_with.
landing = ROOT / "crates/rustok-page-builder/src/landing.rs"
text = read(landing)
text = text.replace("use crate::dto::PAGE_BUILDER_SUPPORTED_DOCUMENT_CONTRACTS;\n", "")
text = text.replace("    GRAPESJS_FORMAT,\n", "")
impl_start = text.find("impl LandingProjectInspection {")
doc_start = text.find("    pub fn document", impl_start)
if impl_start >= 0 and doc_start >= 0:
    methods = '''impl LandingProjectInspection {
    pub fn decode(project_data: &Value) -> LandingProjectResult<Self> {
        Self::decode_with(
            project_data,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        )
    }

    pub fn decode_with(
        project_data: &Value,
        registries: &RegistrySet,
        limits: ValidationLimits,
    ) -> LandingProjectResult<Self> {
        let document = GrapesJsCodec::decode_value(project_data.clone())
            .map_err(LandingProjectError::Fly)?;
        let registry = ComponentRegistryManifest::for_document(&document, registries);
        let validation = validate_project(&document, registries, limits);
        let registry_compatibility = registry_compatibility(&document, &registry, registries);
        Ok(Self {
            document,
            registry,
            validation,
            registry_compatibility,
        })
    }

'''
    text = text[:impl_start] + methods + text[doc_start:]
text = re.sub(
    r"\n    #\[error\(\"unsupported compatibility schema[^\n]*\n    UnsupportedSchema \{\n        supported: &'static \[&'static str\],\n        actual: String,\n    \},",
    "",
    text,
)
text = text.replace("decode_current_with", "decode_with").replace("decode_current", "decode")
write(landing, text)
remove_function(landing, "compatibility_transport_still_decodes_during_the_current_major")
remove_function(landing, "unsupported_compatibility_schema_is_rejected")
replace_all(landing, {"without_a_version_selector": "through_the_current_api"})

# Generic Fly adapter: remove selector-based decode and unsupported-schema errors.
adapters = ROOT / "crates/rustok-page-builder/src/adapters.rs"
text = read(adapters)
text = text.replace(
    "use crate::dto::{BuilderTreeNode, PageBuilderCapabilityRequest, PageBuilderContractMetadata};",
    "use crate::dto::{BuilderTreeNode, PageBuilderCapabilityRequest};",
)
impl_start = text.find("impl FlyProjectInspection {")
doc_start = text.find("    pub fn document", impl_start)
if impl_start >= 0 and doc_start >= 0:
    methods = '''impl FlyProjectInspection {
    pub fn decode(project_data: &Value) -> FlyProjectAdapterResult<Self> {
        Self::decode_with(
            project_data,
            &RegistrySet::with_builtins(),
            ValidationLimits::default(),
        )
    }

    pub fn decode_with(
        project_data: &Value,
        registries: &RegistrySet,
        limits: ValidationLimits,
    ) -> FlyProjectAdapterResult<Self> {
        let document = GrapesJsCodec::decode_value(project_data.clone())
            .map_err(|error| FlyProjectAdapterError::Decode(error.to_string()))?;
        let validation = validate_project(&document, registries, limits);
        Ok(Self {
            document,
            validation,
        })
    }

'''
    text = text[:impl_start] + methods + text[doc_start:]
text = re.sub(
    r"\n    UnsupportedSchema \{\n        expected: &'static str,\n        actual: String,\n    \},",
    "",
    text,
)
text = re.sub(
    r"\n            Self::UnsupportedSchema \{ expected, actual \} => write!\([\s\S]*?\n            \),",
    "",
    text,
)
text = re.sub(
    r"    fn validate_project\(\n        &self,\n        schema_version: &str,\n        project_data: &Value,\n    \) -> Result<\(\), PageBuilderServiceError> \{\n        let inspection = FlyProjectInspection::decode_with\(\n            schema_version,\n            project_data,\n            &self\.registries,\n            self\.limits,\n        \)\?;",
    "    fn validate_project(&self, project_data: &Value) -> Result<(), PageBuilderServiceError> {\n        let inspection = FlyProjectInspection::decode_with(\n            project_data,\n            &self.registries,\n            self.limits,\n        )?;",
    text,
)
text = text.replace("self.validate_project(&input.schema_version, &input.project_data)?;", "self.validate_project(&input.project_data)?;")
text = re.sub(r"FlyProjectInspection::decode\(\"[^\"]+\", &", "FlyProjectInspection::decode(&", text)
write(adapters, text)
remove_function(adapters, "fly_inspection_rejects_contract_drift")

# Canonical telemetry lives in service.rs; remove the parallel bridge module.
service = ROOT / "crates/rustok-page-builder/src/service.rs"
replace_all(
    service,
    {
        "PageBuilderAdapterOperation": "PageBuilderRuntimeOperation",
        "PageBuilderAdapterCallStatus": "PageBuilderRuntimeCallStatus",
        "PageBuilderAdapterCallEvidence": "PageBuilderRuntimeCallEvidence",
        "PageBuilderAdapterTelemetry": "PageBuilderRuntimeTelemetry",
        "NoopPageBuilderAdapterTelemetry": "NoopPageBuilderRuntimeTelemetry",
        "validate_grapesjs_payload": "validate_project_payload",
    },
)
text = read(service)
text = text.replace("PageBuilderContractMetadata", "PageBuilderModuleMetadata")
text = re.sub(r"^\s*pub contract: &'static str,\n", "", text, flags=re.M)
text = re.sub(r"^\s*contract: PageBuilderModuleMetadata::BASELINE\.contract,\n", "", text, flags=re.M)
text = text.replace("PageBuilderModuleMetadata::BASELINE.module_slug", "PageBuilderModuleMetadata::CURRENT.module_slug")
text = re.sub(
    r"validate_project_payload\(\s*&input\.page_id,\s*&input\.schema_version,\s*&input\.project_data,\s*\)\?;",
    "validate_project_payload(&input.page_id, &input.project_data)?;",
    text,
)
text = re.sub(
    r"fn validate_project_payload\(\n    page_id: &str,\n    schema_version: &str,\n    project_data: &serde_json::Value,\n\) -> PageBuilderServiceResult<\(\)> \{\n    validate_non_empty\(\"page_id\", page_id\)\?;\n    if schema_version != PageBuilderModuleMetadata::BASELINE\.contract \{[\s\S]*?\n    \}\n    ensure_object_payload\(\"project_data\", project_data\)\n\}",
    "fn validate_project_payload(\n    page_id: &str,\n    project_data: &serde_json::Value,\n) -> PageBuilderServiceResult<()> {\n    validate_non_empty(\"page_id\", page_id)?;\n    ensure_object_payload(\"project_data\", project_data)\n}",
    text,
)
text = re.sub(r"^\s*schema_version: [^\n]+,\n", "", text, flags=re.M)
text = text.replace("validates_schema_contract", "validates_project_payload")
write(service, text)
remove_function(service, "reference_service_rejects_obsolete_schema")

# Fly-backed service uses the single runtime telemetry API from service.rs.
fly_service = ROOT / "crates/rustok-page-builder/src/adapters/fly_service.rs"
text = read(fly_service)
text = re.sub(
    r"use crate::runtime_telemetry::\{\n    NoopPageBuilderRuntimeTelemetry, PageBuilderRuntimeCallEvidence, PageBuilderRuntimeTelemetry,\n\};\n",
    "",
    text,
)
text = text.replace(
    "    PageBuilderCapabilityService, PageBuilderProjectStore, PageBuilderRenderingAdapter,\n    PageBuilderServiceError, PageBuilderServiceResult,",
    "    NoopPageBuilderRuntimeTelemetry, PageBuilderCapabilityService, PageBuilderProjectStore,\n    PageBuilderRenderingAdapter, PageBuilderRuntimeCallEvidence, PageBuilderRuntimeTelemetry,\n    PageBuilderServiceError, PageBuilderServiceResult,",
)
write(fly_service, text)

runtime_telemetry = ROOT / "crates/rustok-page-builder/src/runtime_telemetry.rs"
if runtime_telemetry.exists():
    runtime_telemetry.unlink()

# Request producers: remove selector assignments/imports.
for source in list((ROOT / "crates/rustok-page-builder").rglob("*.rs")) + list((ROOT / "crates/rustok-pages").rglob("*.rs")):
    text = read(source)
    text = re.sub(r"^\s*schema_version: [^\n]+,\n", "", text, flags=re.M)
    text = text.replace(", PageBuilderContractMetadata", "")
    text = text.replace("PageBuilderContractMetadata, ", "")
    text = text.replace("PageBuilderContractMetadata", "PageBuilderModuleMetadata")
    write(source, text)

# Remove obsolete context migration/compatibility modules and exports.
obsolete_modules = [
    ROOT / "crates/fly/src/context_compatibility.rs",
    ROOT / "crates/fly/src/context_migration.rs",
    ROOT / "crates/rustok-page-builder/src/runtime_context_compatibility.rs",
    ROOT / "crates/rustok-page-builder/src/runtime_context_migration.rs",
]
for path in obsolete_modules:
    if path.exists():
        path.unlink()

for lib in [ROOT / "crates/fly/src/lib.rs", ROOT / "crates/rustok-page-builder/src/lib.rs"]:
    remove_regex(lib, r"^.*(?:context_compatibility|context_migration|runtime_context_compatibility|runtime_context_migration|runtime_telemetry).*$\n", flags=re.M)
    remove_regex(lib, r"^\s*schema_version: [^\n]+,\n", flags=re.M)

# Bundle is a single current artifact: no format selectors.
bundle = ROOT / "crates/fly/src/bundle.rs"
text = read(bundle)
text = re.sub(r"pub const FLY_PROJECT_BUNDLE_FORMAT: &str = \"fly_project_bundle\";\n\n", "", text)
text = re.sub(r"^\s*pub bundle_format: String,\n", "", text, flags=re.M)
text = re.sub(r"^\s*pub project_format: String,\n", "", text, flags=re.M)
text = re.sub(r"^\s*pub require_supported_project_format: bool,\n", "", text, flags=re.M)
text = re.sub(r"^\s*require_supported_project_format: true,\n", "", text, flags=re.M)
text = re.sub(r"^\s*bundle_format: [^\n]+,\n", "", text, flags=re.M)
text = re.sub(r"^\s*project_format: [^\n]+,\n", "", text, flags=re.M)
text = re.sub(r"\n    if bundle\.bundle_format != FLY_PROJECT_BUNDLE_FORMAT \{[\s\S]*?\n    \}", "", text)
text = re.sub(r"\n    if policy\.require_supported_project_format && bundle\.project_format != GRAPESJS_FORMAT \{[\s\S]*?\n    \}", "", text)
text = text.replace('!object.contains_key("bundle_format")', '!object.contains_key("project_hash")')
text = re.sub(r"^\s*assert_eq!\(decoded\.bundle\.bundle_format, FLY_PROJECT_BUNDLE_FORMAT\);\n", "", text, flags=re.M)
write(bundle, text)

# Remove obsolete bridge verification and its npm entry.
obsolete_files = [
    ROOT / "crates/rustok-page-builder/scripts/verify/verify-page-builder-pages-legacy-bridge.mjs",
]
for path in obsolete_files:
    if path.exists():
        path.unlink()

package_json = ROOT / "package.json"
package = json.loads(read(package_json))
package.get("scripts", {}).pop("verify:page-builder:pages:legacy-bridge", None)
write(package_json, json.dumps(package, ensure_ascii=False, indent=2) + "\n")

# Remove version/legacy fields from page-builder JSON contracts and evidence.
for root in [ROOT / "crates/rustok-page-builder", ROOT / "crates/fly"]:
    for path in root.rglob("*.json"):
        cleanup_json(path)

# Terminology is current-only in the affected architecture documents and checks.
for root in [
    ROOT / "crates/fly",
    ROOT / "crates/rustok-page-builder",
    ROOT / "crates/rustok-page-builder-storefront",
    ROOT / "crates/rustok-pages",
    ROOT / "docs/modules",
    ROOT / "docs/architecture",
    ROOT / "DECISIONS",
]:
    if not root.exists():
        continue
    for path in iter_text_files(root):
        text = read(path)
        text = re.sub(r"\blegacy\b", "old", text, flags=re.I)
        text = re.sub(r"\bversioned\b", "separate", text, flags=re.I)
        text = text.replace("schema/version selector", "format selector")
        text = text.replace("version selector", "format selector")
        write(path, text)

# Normalize obvious selector leftovers in current Rust sources.
for root in CURRENT_ROOTS:
    if not root.exists():
        continue
    for path in root.rglob("*.rs"):
        text = read(path)
        text = re.sub(r"^\s*schema_version: [^\n]+,\n", "", text, flags=re.M)
        write(path, text)

# Verification: no obsolete API markers in current Rust source.
forbidden = re.compile(
    r"PageBuilderContractMetadata|PAGE_BUILDER_SUPPORTED_DOCUMENT_CONTRACTS|"
    r"GrapesJsV1|GRAPESJS_V1|FLY_[A-Z_]*_V1|schema_version|schemaVersion|"
    r"legacy_bridge_readonly|runtime_context_compatibility|runtime_context_migration"
)
violations: list[str] = []
for root in CURRENT_ROOTS:
    if not root.exists():
        continue
    for path in root.rglob("*.rs"):
        for line_number, line in enumerate(read(path).splitlines(), 1):
            if forbidden.search(line):
                violations.append(f"{path.relative_to(ROOT)}:{line_number}: {line.strip()}")
if violations:
    raise SystemExit("obsolete Page Builder API markers remain:\n" + "\n".join(violations))
