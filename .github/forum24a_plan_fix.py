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
sentence = 'ID routes remain internal compatibility paths, not the primary storefront UX.\n'
delivered = text.index('### Delivered in FORUM-24A')
trailing = text.find(sentence, delivered)
if trailing < 0:
    raise SystemExit('FORUM-24 trailing ID-route sentence changed')
text = text[:trailing] + text[trailing + len(sentence):]
path.write_text(text)
