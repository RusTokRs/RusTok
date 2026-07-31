from pathlib import Path

script = Path("scripts/agent/apply_forum_search_durable_ingest_sequence.py")
content = script.read_text()
old = '''replace_once(
    path,
    ") -> Result<Option<(DateTime<Utc>, Uuid)>> {",
    ") -> Result<Option<i64>> {",
)
replace_once(
    path,
    ") -> Result<Option<(DateTime<Utc>, Uuid)>> {\\n    let row = transaction",
    ") -> Result<Option<i64>> {\\n    let row = transaction",
)
'''
new = '''content = read(path)
signature = ") -> Result<Option<(DateTime<Utc>, Uuid)>> {"
if content.count(signature) != 2:
    raise SystemExit(f"{path}: expected two watermark signatures")
write(path, content.replace(signature, ") -> Result<Option<i64>> {", 2))
'''
if content.count(old) != 1:
    raise SystemExit("B2G1 watermark staging anchor drift")
content = content.replace(old, new, 1)
exec(compile(content, str(script), "exec"))
