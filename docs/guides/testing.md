# Testing guidelines

These guidelines capture what we should take from the shared rules to keep tests reliable and fast.

## Risks tests can introduce
- **Flaky failures**: async races, timeouts/sleeps, test ordering, external dependencies (DB/ports). Prefer deterministic synchronization over `sleep`.  
- **Test-only design debt**: avoid `#[cfg(test)]` hacks that leak into production design (e.g., making fields public or adding globals just to test).  
- **Build/feature breakage**: test-only dependencies must align with workspace features and avoid system-only libs when possible.  
- **False confidence**: tests that mock away core behavior can pass while real flows break.  
- **Slow suites**: slow integration tests discourage execution and degrade CI feedback loops.  

## Layered approach
1. **Unit**: fast, deterministic, no DB/network/time dependencies. Focus on pure logic and DTO mappings.  
2. **Integration**: uses DB/services, but no external network. Validate migrations, repositories, and service wiring.  
3. **Contract/Golden**: a small set of end-to-end checks for the most critical business flows and API compatibility.  

## Async tests
- Avoid `sleep()` as a synchronization mechanism.  
- Prefer polling with timeouts (retry until state/event observed).  
- Assert on outcomes from event handlers and queues instead of timing assumptions.  

## Database isolation
- Use a transaction per test with rollback, or  
- Use unique tenant identifiers + cleanup strategy.  

## Mocking boundaries
- Mock **ports** (e.g., `PricingPort`, `InventoryPort`, `TaxPort`) when unit testing services.  
- Avoid mocking internal persistence layers (e.g., SeaORM models) unless the test explicitly targets that integration boundary.  

> **Статус документа:** Актуальный. Расширенные примеры — в [`docs/guides/testing-integration.md`](./testing-integration.md) и [`docs/guides/testing-property.md`](./testing-property.md).

## Local quality gates (architecture boundaries)
Run architecture checks locally before commit when changing crate/app dependencies:

```bash
# Full local gate set (includes architecture suite)
./scripts/verify/verify-all.sh

# Only architecture suite
./scripts/verify/verify-all.sh architecture

# Direct boundary guard (cargo metadata + import rules)
python3 scripts/architecture_dependency_guard.py
```

The dependency guard enforces (backend scope):
- target backend apps (currently `rustok-server`) may depend only on workspace crates `rustok-*`, except explicit infrastructure exceptions in `scripts/architecture_rules.toml`;
- no new bypass dependencies between domain crates unless the edge is in `scripts/architecture_rules.toml` allow-list;
- no nested imports of internal crate modules from target backend apps, except explicit allow-list exceptions in `scripts/architecture_rules.toml`;
- frontend workspace libraries remain allowed for frontend apps and are not blocked by this backend-oriented guard.


## Docs PR verification contract

For documentation-only or documentation-heavy PRs, reporting must follow the
repository PR template and the tracker policy in
`docs/research/fix docs.md`.

Minimum requirement:

- include exact commands and statuses in **Testing**;
- mirror the same command list/statuses in **Verification Evidence**;
- include date in `YYYY-MM-DD` for every verification row;
- for `fail`/`blocked`, include `reason: ...` with exact stderr or limitation.

When checks are skipped by policy for text-only edits, use only:

- `text-only: checks skipped by policy`.

Reference template: `.github/pull_request_template.md`.

## Ownership-review path для docs/testing изменений

Для изменений, затрагивающих тестовые контракты, quality gates, ownership или
review policy, применяйте обязательный маршрут согласования:

1. Зафиксировать owner затронутой зоны (module owner или platform/DevEx owner).
2. В PR явно перечислить scope файлов без «и др.».
3. Добавить **Testing** и зеркальный **Verification Evidence** с фактическими
   статусами `pass`/`fail`/`blocked`.
4. Для `fail`/`blocked` указать точную причину и последующий шаг.
5. Обновить связанные central docs (`docs/index.md`, `docs/modules/registry.md`
   или `docs/verification/*`), если контракт изменился.

Это обязательный baseline для DOC-10/DOC-11 и всех последующих docs PR.
