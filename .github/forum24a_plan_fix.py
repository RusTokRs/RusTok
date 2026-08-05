from pathlib import Path

path = Path('crates/rustok-forum/docs/implementation-plan.md')
text = path.read_text()
old = '''match.

### Delivered in FORUM-24A
'''
new = '''match.

ID routes remain internal compatibility paths, not the primary storefront UX.

### Delivered in FORUM-24A
'''
if text.count(old) != 1:
    raise SystemExit('FORUM-24 scope insertion point changed')
text = text.replace(old, new, 1)
trailing = '''
ID routes remain internal compatibility paths, not the primary storefront UX.

## `FORUM-25`
'''
replacement = '''
## `FORUM-25`
'''
if text.count(trailing) != 1:
    raise SystemExit('FORUM-24 trailing ID-route sentence changed')
path.write_text(text.replace(trailing, replacement, 1))
