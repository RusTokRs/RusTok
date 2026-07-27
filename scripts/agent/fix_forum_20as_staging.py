from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(path: str, old: str, new: str) -> None:
    target = ROOT / path
    text = target.read_text()
    count = text.count(old)
    if count != 1:
        raise RuntimeError(f"{path}: expected one occurrence, found {count}: {old!r}")
    target.write_text(text.replace(old, new, 1))


replace_once(
    "scripts/agent/apply_forum_20as.py",
    '    "- provide Forum trust and Channel membership facts adapters, then add reply and moderation\\n  audience policies plus owner write commands;",',
    '    "- add reply and moderation audience policies plus owner write commands;",',
)

replace_once(
    "crates/rustok-forum/contracts/forum-topic-create-audience-transport-composition.json",
    '  "graphql_legacy_mutation_file": "crates/rustok-forum/src/graphql/mutation.rs",\n',
    '  "graphql_legacy_mutation_file": "crates/rustok-forum/src/graphql/mutation.rs",\n'
    '  "graphql_types_file": "crates/rustok-forum/src/graphql/types.rs",\n',
)

replace_once(
    "scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs",
    'const graphqlLegacy = read(contract.graphql_legacy_mutation_file ?? "");\n',
    'const graphqlLegacy = read(contract.graphql_legacy_mutation_file ?? "");\n'
    'const graphqlTypes = read(contract.graphql_types_file ?? "");\n',
)
replace_once(
    "scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs",
    '  graphqlLegacy,\n  "pub struct CreateForumTopicInput",',
    '  graphqlTypes,\n  "pub struct CreateForumTopicInput",',
)

(ROOT / "scripts/agent/fix_forum_20as_staging.py").unlink()
