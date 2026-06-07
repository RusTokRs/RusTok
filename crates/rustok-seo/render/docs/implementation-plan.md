# План реализации `rustok-seo-render`

Статус: renderer crate стабилен как canonical Rust-side SSR adapter для `SeoPageContext`. Следующий wave — parity hardening с host-интеграциями в рамках SEO Phase D (`D7`, `D8`, `D9`).

## Execution checkpoint

- Current phase: `phase_d7_renderer_parity_alignment`
- Last checkpoint: Добавлен snapshot batch D7.1: deterministic primary snapshot assertion + nondeterministic-token normalization comparison test для parity tooling.
- Next step: Закрыть D7.2 — расширить cross-host fixture matrix Rust renderer vs Next metadata adapter.
- Open blockers:
  - В этой VM отсутствует `cargo` в `PATH`, локальные проверки не запускались.
  - Для cross-host parity нужен стабильный REST/GraphQL `SeoPageContext` contract после SEO Batch D4.
- Hand-off notes for next agent:
  - Не переносить SEO business logic в renderer crate.
  - Любые изменения renderer должны оставаться pure serialization поверх backend-provided `SeoPageContext`.
  - Поддерживать parity evidence между Rust storefront renderer и Next metadata adapter.
- Last updated at (UTC): 2026-06-07T17:45:00Z

## Область работы

- держать единый Rust-side renderer поверх canonical `rustok-seo::SeoPageContext`;
- не позволять host-приложениям дублировать robots/meta/link/JSON-LD serialization;
- оставлять всю SEO business logic в `rustok-seo`, а не переносить её в adapter crate.

## Текущее состояние

- crate уже публикует `render_head_html` и `robots_directives`;
- `apps/storefront` использует этот crate вместо локального `build_seo_head`;
- renderer покрывает canonical, hreflang, typed robots, Open Graph, Twitter, verification, pagination, generic meta/link tags и JSON-LD blocks.

## Phase D backlog (renderer-side)

- [x] **D7.1 — Parity snapshots**
  - [ ] Добавить snapshot/unit tests на combinations: canonical + alternates + noindex + verification tags + multi-block JSON-LD.
  - [ ] Зафиксировать deterministic ordering для meta/link/script tags.

- [ ] **D7.2 — Cross-host contract parity**
  - [ ] Добавить contract tests, сравнивающие Rust renderer output и Next metadata adapter behavior на одном `SeoPageContext` fixture set.
  - [ ] Зафиксировать допустимые расхождения (например, unsupported long-tail tags в Next API).

- [ ] **D8 — Verification matrix**
  - [ ] Integration smoke с `apps/storefront` SSR path и `storefront/seo-page-context` server function.
  - [ ] Regression tests на `SeoStructuredDataBlock` serialization (`schema_kind`, `schema_type`, `source`, payload).

- [ ] **D9 — Docs/DoD sync**
  - [ ] Обновить README/docs по parity rules и renderer/non-renderer boundary.
  - [ ] Добавить mini-runbook для drift между Rust renderer и Next metadata adapter.

## Правила обновления

1. Изменения canonical SEO contract сначала фиксируются в `rustok-seo`.
2. Затем синхронизируется renderer crate и Rust-host потребители.
3. Если меняется ownership или public API renderer-а, обновляются `README.md`, `docs/README.md` и центральные registry docs.

## Проверка

- `cargo check -p rustok-seo-render --tests --config profile.dev.debug=0`
- `cargo check -p rustok-storefront --config profile.dev.debug=0`
- `npm --prefix apps/next-frontend run lint && npm --prefix apps/next-frontend run typecheck`

## Quality backlog

- [ ] Добавить snapshot coverage для parity-critical tag combinations.
- [ ] Поддерживать contract fixtures для Rust/Next parity.
- [ ] Обновлять execution checkpoint после каждого D7/D8 инкремента.
