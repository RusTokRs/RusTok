#!/usr/bin/env python3
import json
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    target = Path(path)
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{path}: expected one marker, found {count}: {old[:120]!r}")
    target.write_text(text.replace(old, new, 1))


verifier = "scripts/verify/verify-blog-admin-boundary.mjs"
replace_once(
    verifier,
    'const uiPath = "crates/rustok-blog/admin/src/ui/leptos.rs";\n',
    'const uiPath = "crates/rustok-blog/admin/src/ui/leptos.rs";\n'
    'const richtextAdapterPath = "crates/rustok-blog/admin/src/ui/richtext.rs";\n',
)
replace_once(
    verifier,
    '  uiPath,\n  moderationPath,\n',
    '  uiPath,\n  richtextAdapterPath,\n  moderationPath,\n',
)
replace_once(
    verifier,
    'const ui = readRepo(uiPath);\nconst moderation = readRepo(moderationPath);\n',
    'const ui = readRepo(uiPath);\nconst richtextAdapter = readRepo(richtextAdapterPath);\nconst moderation = readRepo(moderationPath);\n',
)
replace_once(
    verifier,
    'assertContains(ui, "set_document=set_content", `${uiPath}: UI must receive canonical document updates from the editor`);\n',
    'assertContains(ui, "set_document=set_content", `${uiPath}: UI must receive canonical document updates from the editor`);\n'
    'for (const [marker, description] of [\n'
    '  ["pub fn BlogRichTextEditor(", "owner editor component"],\n'
    '  ["ReadSignal<RichTextDocument>", "typed controlled input"],\n'
    '  ["WriteSignal<RichTextDocument>", "typed controlled output"],\n'
    '  ["serde_json::from_str::<RichTextDocument>", "typed RichTextDocument deserialization"],\n'
    '  ["set_document.set(document)", "typed document state update"],\n'
    '  [\'sandbox="allow-scripts"\', "isolated script-only iframe sandbox"],\n'
    '  [\'referrerpolicy="no-referrer"\', "no-referrer iframe policy"],\n'
    '  ["on_cleanup", "frame cleanup hook"],\n'
    '  ["dispose_richtext_frame", "frame disposal"],\n'
    ']) {\n'
    '  assertContains(richtextAdapter, marker, `${richtextAdapterPath}: missing ${description}`);\n'
    '}\n'
    'assertContains(\n'
    '  richtextAdapter,\n'
    '  /mount_richtext_frame\\([\\s\\S]*?"\\/richtext\\/frame",\\s*"article",/,\n'
    '  `${richtextAdapterPath}: owner adapter must mount the fixed Article profile through the canonical frame`,\n'
    ');\n'
    'assertNotContains(\n'
    '  richtextAdapter,\n'
    '  /sandbox="[^"]*allow-same-origin/,\n'
    '  `${richtextAdapterPath}: owner iframe must not grant allow-same-origin`,\n'
    ');\n'
    'for (const marker of [\'"discussion"\', "serde_json::from_str::<serde_json::Value>"]) {\n'
    '  assertNotContains(richtextAdapter, marker, `${richtextAdapterPath}: owner Article adapter contains forbidden ${marker}`);\n'
    '}\n',
)
replace_once(
    verifier,
    '  adminRichtextEvidence.schema_version !== 2 ||\n',
    '  adminRichtextEvidence.schema_version !== 3 ||\n',
)
replace_once(
    verifier,
    '  adminRichtextEvidence.sources?.ui !== uiPath ||\n  adminRichtextEvidence.sources?.locales?.en !== adminEnLocalePath ||\n',
    '  adminRichtextEvidence.sources?.ui !== uiPath ||\n  adminRichtextEvidence.sources?.adapter !== richtextAdapterPath ||\n  adminRichtextEvidence.sources?.locales?.en !== adminEnLocalePath ||\n',
)
replace_once(
    verifier,
    'for (const marker of adminRichtextEvidence.required_markers?.ui ?? []) {\n  assertContains(ui, marker, `${uiPath}: evidence-required canonical UI marker ${marker}`);\n}\n',
    'for (const marker of adminRichtextEvidence.required_markers?.ui ?? []) {\n  assertContains(ui, marker, `${uiPath}: evidence-required canonical UI marker ${marker}`);\n}\n'
    'for (const marker of adminRichtextEvidence.required_markers?.adapter ?? []) {\n'
    '  assertContains(richtextAdapter, marker, `${richtextAdapterPath}: evidence-required owner adapter marker ${marker}`);\n'
    '}\n'
    'for (const marker of adminRichtextEvidence.forbidden_adapter_markers ?? []) {\n'
    '  assertNotContains(richtextAdapter, marker, `${richtextAdapterPath}: evidence-forbidden owner adapter marker ${marker}`);\n'
    '}\n',
)


evidence_path = Path("crates/rustok-blog/contracts/evidence/blog-admin-richtext-boundary.json")
evidence = json.loads(evidence_path.read_text())
evidence["schema_version"] = 3
evidence["sources"]["adapter"] = "crates/rustok-blog/admin/src/ui/richtext.rs"
evidence["required_markers"]["adapter"] = [
    "pub fn BlogRichTextEditor(",
    "ReadSignal<RichTextDocument>",
    "WriteSignal<RichTextDocument>",
    "mount_richtext_frame",
    '"/richtext/frame"',
    '"article"',
    "serde_json::from_str::<RichTextDocument>",
    "set_document.set(document)",
    'sandbox="allow-scripts"',
    'referrerpolicy="no-referrer"',
    "on_cleanup",
    "dispose_richtext_frame",
]
evidence["forbidden_adapter_markers"] = [
    '"discussion"',
    "allow-same-origin",
    "serde_json::from_str::<serde_json::Value>",
]
evidence_path.write_text(json.dumps(evidence, indent=2) + "\n")


test_path = "scripts/verify/verify-blog-admin-boundary.test.mjs"
replace_once(
    test_path,
    'function moderationSource({ rawServiceCall = false, omitModeration = false } = {}) {\n',
    '''function richtextAdapterSource({
  wrongProfile = false,
  unsafeSandbox = false,
  untypedPayload = false,
  missingCleanup = false,
} = {}) {
  return `
use rustok_api::RichTextDocument;
pub fn BlogRichTextEditor(
    document: ReadSignal<RichTextDocument>,
    set_document: WriteSignal<RichTextDocument>,
) {
    let document_json = "{}";
    let mounted_handle = mount_richtext_frame(
        &iframe,
        "/richtext/frame",
        "${wrongProfile ? "discussion" : "article"}",
        document_json,
        messages_json,
        true,
        &on_document_change,
        &on_error,
    );
    ${untypedPayload ? "serde_json::from_str::<serde_json::Value>(document_json);" : "serde_json::from_str::<RichTextDocument>(document_json);"}
    set_document.set(document);
    sandbox="${unsafeSandbox ? "allow-scripts allow-same-origin" : "allow-scripts"}";
    referrerpolicy="no-referrer";
    ${missingCleanup ? "" : "on_cleanup(move || { dispose_richtext_frame(&mounted_handle); });"}
}
`;
}

function moderationSource({ rawServiceCall = false, omitModeration = false } = {}) {
''',
)
replace_once(
    test_path,
    '  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/leptos.rs", uiSource(options));\n',
    '  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/leptos.rs", uiSource(options));\n'
    '  writeFixtureFile(root, "crates/rustok-blog/admin/src/ui/richtext.rs", richtextAdapterSource(options));\n',
)
replace_once(
    test_path,
    '    schema_version: 2,\n',
    '    schema_version: 3,\n',
)
replace_once(
    test_path,
    '      ui: "crates/rustok-blog/admin/src/ui/leptos.rs",\n      locales: {\n',
    '      ui: "crates/rustok-blog/admin/src/ui/leptos.rs",\n'
    '      adapter: "crates/rustok-blog/admin/src/ui/richtext.rs",\n'
    '      locales: {\n',
)
replace_once(
    test_path,
    '      ui: ["use super::richtext::BlogRichTextEditor;", "let (content, set_content) = signal(RichTextDocument::empty());", "<BlogRichTextEditor", "document=content", "set_document=set_content"]\n    },\n    forbidden_markers:',
    '      ui: ["use super::richtext::BlogRichTextEditor;", "let (content, set_content) = signal(RichTextDocument::empty());", "<BlogRichTextEditor", "document=content", "set_document=set_content"],\n'
    '      adapter: ["pub fn BlogRichTextEditor(", "ReadSignal<RichTextDocument>", "WriteSignal<RichTextDocument>", "mount_richtext_frame", "\\\"/richtext/frame\\\"", "\\\"article\\\"", "serde_json::from_str::<RichTextDocument>", "set_document.set(document)", "sandbox=\\\"allow-scripts\\\"", "referrerpolicy=\\\"no-referrer\\\"", "on_cleanup", "dispose_richtext_frame"]\n'
    '    },\n'
    '    forbidden_adapter_markers: ["\\\"discussion\\\"", "allow-same-origin", "serde_json::from_str::<serde_json::Value>"],\n'
    '    forbidden_markers:',
)
replace_once(
    test_path,
    'test("blog admin boundary verifier rejects Leptos-specific core", () => {\n',
    '''test("blog admin boundary verifier rejects a non-Article owner editor profile", () => {
  withRoot({ wrongProfile: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /fixed Article profile|forbidden.*discussion/);
  });
});

test("blog admin boundary verifier rejects allow-same-origin on the editor iframe", () => {
  withRoot({ unsafeSandbox: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /must not grant allow-same-origin|evidence-forbidden owner adapter marker/);
  });
});

test("blog admin boundary verifier rejects untyped editor payload deserialization", () => {
  withRoot({ untypedPayload: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /typed RichTextDocument deserialization|evidence-required owner adapter marker/);
  });
});

test("blog admin boundary verifier rejects missing frame cleanup", () => {
  withRoot({ missingCleanup: true }, (result) => {
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /frame cleanup hook|frame disposal|evidence-required owner adapter marker/);
  });
});

test("blog admin boundary verifier rejects Leptos-specific core", () => {
''',
)


plan = "crates/rustok-blog/docs/implementation-plan.md"
replace_once(
    plan,
    'machine evidence plus self-regression fixtures. The guardrail is part of the\nBlog FBA command chain.\n',
    'machine evidence plus self-regression fixtures. The guardrail is part of the\n'
    'Blog FBA command chain. The owner adapter itself is now evidence-bound: it must\n'
    'mount the canonical frame with the fixed Article profile, round-trip only typed\n'
    '`RichTextDocument` payloads, keep an `allow-scripts`-only/no-referrer iframe, and\n'
    'dispose the mounted frame during Leptos cleanup.\n',
)
replace_once(
    plan,
    '- Blog admin canonical richtext guardrail: `source_verified_no_compile`; the\n  FFA verifier requires typed document/editor state, rejects removed selector,\n  raw-body helper, and locale-key contracts, validates machine evidence, and has\n  negative fixtures.\n',
    '- Blog admin canonical richtext guardrail: `source_verified_no_compile`; the\n'
    '  FFA verifier requires typed document/editor state, a fixed Article frame profile,\n'
    '  typed payload deserialization, an isolated no-referrer iframe, and cleanup/dispose;\n'
    '  it also rejects removed selector, raw-body helper, and locale-key contracts with\n'
    '  machine evidence and focused negative fixtures.\n',
)
replace_once(
    plan,
    '31. Locked the complete Blog FBA self-test chain in registry schema v5 and added\n    focused negative fixtures for offline-backfill safety and Forum Next admin\n    ownership, including exact leaf-test and consumer-runtime bindings.\n',
    '31. Locked the complete Blog FBA self-test chain in registry schema v5 and added\n'
    '    focused negative fixtures for offline-backfill safety and Forum Next admin\n'
    '    ownership, including exact leaf-test and consumer-runtime bindings.\n'
    '32. Extended the Blog admin canonical-richtext guardrail through the owner adapter\n'
    '    itself: fixed Article frame profile, typed document round-trip, isolated\n'
    '    no-referrer iframe, cleanup/dispose, evidence schema v3, and negative fixtures.\n',
)

print("Blog admin richtext owner-adapter guard staged")
