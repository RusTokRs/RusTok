from pathlib import Path

script = Path("scripts/agent/apply_forum_search_kind_filter.py")
content = script.read_text()
old = '''    if count != 1:
        raise SystemExit(f"{path}: expected one replacement, found {count}")
    write(path, content.replace(old, new, 1))
'''
new = '''    if count != 1:
        repeated_native_helper_signature = (
            path == "crates/rustok-search/storefront/src/transport/forum_native_server_adapter.rs"
            and old == "    published_from: Option<String>,\\n    published_to: Option<String>,\\n) -> Result<SearchPreviewPayload, ServerFnError> {"
            and count == 2
        )
        if repeated_native_helper_signature:
            index = content.rindex(old)
            write(path, content[:index] + new + content[index + len(old):])
            return
        raise SystemExit(f"{path}: expected one replacement, found {count}")
    write(path, content.replace(old, new, 1))
'''
if content.count(old) != 1:
    raise SystemExit("B2F4 staging helper definition drift")
content = content.replace(old, new, 1)
exec(compile(content, str(script), "exec"))
