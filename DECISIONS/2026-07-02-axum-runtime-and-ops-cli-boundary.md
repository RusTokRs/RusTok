# Граница Axum runtime и ops CLI

- Date: 2026-07-02
- Status: Accepted

## Context

RusToK уходит от Loco RS как application/runtime owner. При этом часть Loco
conventions полезна как operator/dev workflow: migrate, seed, install,
maintenance tasks и будущие distribution-aware builds.

Если оставить CLI и maintenance code внутри production server runtime, серверный
binary будет тащить исполняемую требуху, которая нужна не всем дистрибутивам.
Если вынести все module-specific команды в один центральный CLI crate, этот crate
станет свалкой команд всех core, optional и external модулей. Если поместить CLI
в доменное ядро модуля, модуль начнёт зависеть от `clap`, stdout/stderr, exit
codes и operator UX, что ломает hexagonal boundary.

## Decision

1. `apps/server` является чистым Axum runtime entrypoint: HTTP startup/shutdown,
   router composition, runtime context, workers и lifecycle.
2. Production server binary не зависит от ops CLI crate и не содержит
   maintenance command code.
3. Operator/dev CLI принадлежит отдельному ops layer: `rustok-ops` runner,
   parser, registry, settings loading и exit-code/output policy.
4. Доменное ядро модуля не зависит от ops CLI contracts.
5. Module-specific commands живут рядом с модулем как отдельный `cli/` adapter
   package, например `crates/rustok-index/cli`, и вызывают public typed API
   своего модуля.
6. `rustok-ops` агрегирует command providers через явный module/distribution
   manifest или generated registry, а не через hardcoded список всех модулей.
7. External modules могут поставлять свой `cli/` adapter package; если они этого
   не делают, host/distribution может держать adapter в integration layer.
8. Distribution-aware builds являются допустимым follow-up: `rustok-ops` может
   генерировать runtime/ops registries, собирать server binary без ops layer и
   ops binary только с providers выбранного дистрибутива.

## Consequences

- Удаление Loco CLI/tasks не требует переносить maintenance code в
  `apps/server`.
- Module ownership сохраняется: команды, scripts и maintenance adapters лежат
  рядом с модулем, но не внутри domain core.
- Центральный ops runner остаётся инфраструктурным orchestration layer, а не
  каталогом всех команд всех модулей.
- Дистрибутивы могут собирать разные наборы runtime modules и ops providers без
  ручного редактирования server crate.
- Любой следующий cutover от Loco tasks должен переводить use case в typed Rust
  API и вызывать его из module-local `cli/` adapter через `rustok-ops`.
