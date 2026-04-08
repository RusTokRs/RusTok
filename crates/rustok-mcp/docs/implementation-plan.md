# План реализации `rustok-mcp`

Статус: governed MCP tool adapter уже работает поверх `rmcp`; следующая работа
не про переписывание протокола, а про доведение RusToK-specific runtime,
identity/audit и Alloy-related control plane до platform-grade уровня.

## Область работ

- удерживать `rustok-mcp` как thin MCP adapter crate поверх `rmcp`;
- синхронизировать tool surface, runtime binding, access policy и local docs;
- не допускать смешивания MCP protocol boundary с AI provider orchestration.

## Текущее состояние

- crate уже интегрирован с `rmcp` и поставляется как library + binary;
- module discovery tools, health/introspection, Alloy-related tools и scaffold review/apply boundary уже подняты;
- persisted server-side scaffold drafts и runtime draft-store bridge уже связаны с MCP flow;
- identity/policy foundation, session-start runtime binding и allow/deny audit уже являются частью live contract.

## Этапы

### 1. Contract stability

- [x] зафиксировать `rustok-mcp` как thin adapter поверх `rmcp`;
- [x] поднять typed tool surface, response envelope и access-policy baseline;
- [x] встроить Alloy-related scaffold/review/apply vertical и runtime draft-store binding;
- [ ] удерживать sync между runtime contracts, management/control plane и local docs.

### 2. Platform hardening

- [ ] довести server-owned remote MCP transport/session bootstrap beyond текущий stdio path;
- [ ] расширить audit trail от allow/deny к richer execution telemetry;
- [ ] удерживать identity/policy layer совместимым с official MCP authorization guidance.

### 3. Product surface

- [ ] добавить UI-слой для MCP access management и Alloy draft review;
- [ ] расширять Alloy/codegen vertical без автоматического размывания review/apply boundary;
- [ ] добавлять новые MCP capabilities (`resources`, `prompts`, `sampling` и др.) только как explicit staged rollout.

## Проверка

- structural verification для RusToK-specific MCP docs и boundary;
- targeted compile/tests при изменении tool surface, access policy, runtime binding или draft-store integration;
- обязательная сверка с official MCP/rmcp docs при изменении protocol/security assumptions.

## Правила обновления

1. При изменении RusToK-specific MCP contract сначала обновлять этот файл.
2. Сначала сверять изменения с official MCP/rmcp источниками, потом обновлять local docs.
3. При изменении public crate behavior синхронизировать `README.md` и `docs/README.md`.
4. При изменении reference-map обновлять `docs/references/mcp/README.md` и при необходимости `docs/index.md`.
