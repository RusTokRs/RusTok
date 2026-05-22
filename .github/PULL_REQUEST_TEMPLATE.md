# Pull Request

## 📋 Summary

<!-- Кратко: что изменено и зачем. -->

## 🎯 Scope

- DOC / code scope:
- Affected modules/apps:
- Out of scope:

## 🔍 Type of Change

- [ ] 🐛 Bug fix
- [ ] ✨ New feature
- [ ] 💥 Breaking change
- [ ] 📝 Documentation update
- [ ] 🔧 Configuration change
- [ ] ♻️ Refactor
- [ ] ⚡ Performance improvement
- [ ] 🧪 Test updates

## ✅ Required Checklist

### Universal
- [ ] Self-review completed
- [ ] No debug-only leftovers
- [ ] Changed docs are aligned with actual code/runtime behavior

### Docs policy (required for docs or contract changes)
- [ ] `docs/index.md` updated when navigation changed
- [ ] `docs/modules/registry.md` updated if module/app composition changed
- [ ] Existing docs were extended instead of creating duplicate documents
- [ ] One file — one language policy respected

### Security / Contract integrity
- [ ] Tenant boundaries preserved
- [ ] Permission/authorization contracts preserved
- [ ] Validation/error contracts updated where needed

## 🧪 Verification

<!-- Обязательно: фактические результаты. Не писать "checks passed" без evidence. -->

### Verification Evidence

- YYYY-MM-DD — `<exact command>` — `pass|fail|blocked`
  - output/reason: `<1-3 lines>`

<!-- Для text-only docs PR допустимо: -->
<!-- text-only: checks skipped by policy (see docs/research/fix docs.md) -->

## 📝 Docs Reviewer Checklist (required for docs PR)

- [ ] Scope and intent are clear and bounded
- [ ] Claims match current code/config/runtime
- [ ] Links/anchors are valid in changed sections
- [ ] Examples/commands are runnable or explicitly marked as blocked
- [ ] Ownership and follow-ups are stated when work is partial

## 🔗 Additional Context

- Related issue(s):
- Follow-up task(s):
- Migration/deployment notes (if any):
