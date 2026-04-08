# План верификации платформы: события, домены и интеграции

- **Статус:** актуальный детальный чеклист
- **Контур:** domain events, outbox/runtime transport, межмодульные связи, integration boundaries
- **Примечание:** API и UI surfaces проверяются в отдельных verification-планах, здесь остаётся event/runtime contract

---

## Актуальный scoped contract

Event/runtime слой должен оставаться согласованным с current-state моделью:

- canonical event contracts живут в `rustok-events`
- transactional delivery contract живёт в `rustok-outbox`
- publishers владеют semantic meaning своих событий
- consumers остаются идемпотентными и replay-safe

## Фаза 1. Event runtime

### 1.1 Runtime bootstrap

**Файлы:**
- `apps/server/src/services/event_transport_factory.rs`
- `apps/server/src/services/event_bus.rs`
- `crates/rustok-outbox/`
- `crates/rustok-iggy/`

- [ ] Server bootstrap поднимает актуальный event runtime.
- [ ] Transport mode согласован с текущими settings и runtime wiring.
- [ ] `rustok-outbox` остаётся production-first delivery path там, где нужна транзакционная согласованность.
- [ ] Дополнительные transport layers не подменяют canonical outbox contract.

### 1.2 Transactional publish path

- [ ] Domain write path и запись в outbox происходят в одной транзакции там, где это требуется контрактом.
- [ ] Межмодульные события не публикуются мимо canonical transactional path без явной причины.
- [ ] Runtime docs и local docs publisher-а совпадают с фактическим publish path.

## Фаза 2. Domain event ownership

### 2.1 Publishers

- [ ] Ownership event family совпадает с owning module/service layer.
- [ ] Host layer не становится скрытым publisher-ом module-owned event family.
- [ ] Shared helper events не разрастаются в универсальный substitute для typed domain events.

### 2.2 Consumers

- [ ] Consumers обновляют projections и downstream state идемпотентно.
- [ ] Replay и recovery path остаются допустимыми.
- [ ] Consumer path не ломает module boundaries.

## Фаза 3. Межмодульные связи

### 3.1 Dependency discipline

- [ ] Межмодульные зависимости совпадают с `modules.toml`, local docs и runtime wiring.
- [ ] Новый integration path не создаёт скрытую прямую связность между модулями там, где нужен event-driven contract.
- [ ] Capability/support crate-ы не выдаются за платформенные модули в integration graph.

## Фаза 4. Read-side и интеграции

### 4.1 Index/read consumers

- [ ] Read-side consumers согласованы с `rustok-index` и event-flow contract.
- [ ] External integration path не подменяет canonical internal event flow.
- [ ] Routing/cache/index updates, завязанные на события, описаны в owning component docs.

## Фаза 5. Точечные локальные проверки

### 5.1 Минимум

- [ ] targeted `cargo check` / `cargo test` для затронутых publishers/consumers
- [ ] targeted `xtask module test <slug>`, если меняется module-owned event contract
- [ ] targeted runtime smoke, если меняется transport wiring

## Open blockers

- [ ] Runtime-only blockers фиксировать кратко, отдельно от самого checklist.
- [ ] Не превращать этот документ в список исторических инцидентов.

## Связанные документы

- [Контракт потока доменных событий](../architecture/event-flow-contract.md)
- [Каналы и real-time surfaces](../architecture/channels.md)
- [Архитектура модулей](../architecture/modules.md)
- [Реестр crate-ов модульной платформы](../modules/crates-registry.md)
