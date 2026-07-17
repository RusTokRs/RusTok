from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8")


def clean_initializers(text: str) -> str:
    text = re.sub(r"^\s*(?:provider_version|schema_version|owner_version): [^\n]+,\n", "", text, flags=re.M)
    return text


contribution = ROOT / "crates/fly-ui/src/contribution.rs"
text = read(contribution)
text = re.sub(r"^\s*pub (?:provider_version|schema_version): String,\n", "", text, flags=re.M)
text = re.sub(r"\n\s*schema_version: &str,", "", text)
text = re.sub(r"^\s*let schema_version = schema_version\.trim\(\);\n", "", text, flags=re.M)
text = re.sub(r"\n\s*&& (?:renderer|editor|registered)\.schema_version == (?:schema_version|renderer\.schema_version|editor\.schema_version)", "", text)
text = re.sub(r"\n\s*contribution\.provider_version = required_value\([\s\S]*?\n\s*\)\?;", "", text)
text = re.sub(r"\n\s*(?:renderer|editor)\.schema_version = required_value\([\s\S]*?\n\s*\)\?;", "", text)
text = re.sub(r"fn renderer_contract_id\(renderer: &RendererDescriptor, presentation: Presentation\) -> String \{[\s\S]*?\n\}", '''fn renderer_contract_id(renderer: &RendererDescriptor, presentation: Presentation) -> String {
    format!(
        "{}:{}:{}",
        renderer.provider,
        renderer.component_type,
        presentation.as_str()
    )
}''', text)
text = re.sub(r"fn property_editor_contract_id\(editor: &PropertyEditorDescriptor\) -> String \{[\s\S]*?\n\}", '''fn property_editor_contract_id(editor: &PropertyEditorDescriptor) -> String {
    format!("{}:{}", editor.provider, editor.component_type)
}''', text)
text = clean_initializers(text)
text = re.sub(r"\.resolve_renderer\(\n(\s*\"[^\"]+\",\n\s*\"[^\"]+\",\n)\s*\"[^\"]+\",\n", r".resolve_renderer(\n\1", text)
text = re.sub(r"\.resolve_property_editor\((\"[^\"]+\",\s*\"[^\"]+\",\s*)\"[^\"]+\",\s*", r".resolve_property_editor(\1", text)
text = re.sub(r"\n\s*descriptor\.provider_version = [^\n]+;", "", text)
text = re.sub(r"^\s*assert_eq!\(stored\.provider_version,[^\n]+\);\n", "", text, flags=re.M)
write(contribution, text)

adapter = ROOT / "crates/fly-ui/src/contribution_adapter.rs"
text = read(adapter)
text = re.sub(r"^\s*pub schema_version: &'a str,\n", "", text, flags=re.M)
text = re.sub(r"\n\s*request\.schema_version,", "", text)
text = re.sub(r"\n\s*schema_version: &str,", "", text)
text = text.replace(
    '        "{}:{}:{}:{}",\n        provider.trim(),\n        component_type.trim(),\n        schema_version.trim(),\n        presentation.as_str()\n',
    '        "{}:{}:{}",\n        provider.trim(),\n        component_type.trim(),\n        presentation.as_str()\n',
)
text = re.sub(r"fn property_editor_lookup_id\([\s\S]*?\n\}", '''fn property_editor_lookup_id(provider: &str, component_type: &str) -> String {
    format!("{}:{}", provider.trim(), component_type.trim())
}''', text, count=1)
text = clean_initializers(text)
write(adapter, text)

factory = ROOT / "crates/fly-ui/src/contribution_factory.rs"
text = read(factory)
text = re.sub(r"^\s*pub provider_version: String,\n", "", text, flags=re.M)
text = re.sub(r"\n\s*\|\| contribution\.provider_version\.trim\(\) != module\.provider_version", "", text)
text = text.replace(
    '"contribution provider/version `{}@{}` does not match module `{}@{}`",\n                        contribution.provider.trim(),\n                        contribution.provider_version.trim(),\n                        module.provider,\n                        module.provider_version',
    '"contribution provider `{}` does not match module `{}`",\n                        contribution.provider.trim(),\n                        module.provider',
)
text = re.sub(r"^\s*module\.provider_version = required\([^\n]+\)\?;\n", "", text, flags=re.M)
text = clean_initializers(text)
write(factory, text)

model = ROOT / "crates/fly-ui/src/contribution_manifest/model.rs"
write(model, '''use crate::ContributionDescriptor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModuleContributionManifest {
    pub module_id: String,
    pub owner_provider: String,
    #[serde(default)]
    pub target_providers: BTreeSet<String>,
    #[serde(default)]
    pub dependencies: BTreeSet<String>,
    #[serde(default)]
    pub required_permissions: BTreeSet<String>,
    #[serde(default)]
    pub admin: Vec<ContributionDescriptor>,
    #[serde(default)]
    pub storefront: Vec<ContributionDescriptor>,
}

impl ModuleContributionManifest {
    pub fn allows_target_provider(&self, provider: &str) -> bool {
        let provider = provider.trim();
        provider == self.owner_provider.trim() || self.target_providers.contains(provider)
    }
}
''')

assemble = ROOT / "crates/fly-ui/src/contribution_manifest/assemble.rs"
text = read(assemble)
text = text.replace("        let target_version = contribution.provider_version.trim();\n", "")
text = text.replace("manifest.allows_target_provider(target_provider, target_version)", "manifest.allows_target_provider(target_provider)")
text = text.replace("contribution targets `{target_provider}@{target_version}`; allowed targets: {allowed}", "contribution targets `{target_provider}`; allowed targets: {allowed}")
text = re.sub(r"fn allowed_target_summary\(manifest: &ModuleContributionManifest\) -> String \{[\s\S]*?\n\}", '''fn allowed_target_summary(manifest: &ModuleContributionManifest) -> String {
    manifest
        .target_providers
        .iter()
        .cloned()
        .chain(std::iter::once(manifest.owner_provider.clone()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(", ")
}''', text, count=1)
text = re.sub(r"^\s*manifest\.owner_version = required\([^\n]+\)\?;\n", "", text, flags=re.M)
text = re.sub(r"\n\s*let mut target_providers = BTreeMap::new\(\);[\s\S]*?\n\s*manifest\.target_providers = target_providers;", '''
    manifest.target_providers = normalize_set(manifest.target_providers, "target provider")?;''', text, count=1)
text = clean_initializers(text)
write(assemble, text)

# Remove version fields from all contribution fixtures and constructors; compiler reports any semantic use left.
for root in [ROOT / "crates/fly-ui", ROOT / "crates/fly-leptos", ROOT / "crates/rustok-page-builder", ROOT / "crates/rustok-page-builder-storefront", ROOT / "crates/rustok-pages"]:
    if not root.exists():
        continue
    for path in root.rglob("*.rs"):
        text = clean_initializers(read(path))
        write(path, text)
