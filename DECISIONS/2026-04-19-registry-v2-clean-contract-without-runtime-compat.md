# Registry V2 clean contract без runtime-compat слоя

- Date: 2026-04-19
- Status: Accepted

## Context

Registry/governance surface до clean-cutover содержал несколько классов проблем:

- header-based actor model для live authority;
- string-based error classification;
- смешивание public contract и internal audit payload;
- filesystem-oriented artifact contract;
- legacy naming и runtime fallback, которые размывали канонический principal-based read/write contract.

Параллельно платформа всё ещё находится на ранней стадии, поэтому сохранение runtime backward compatibility для старого registry payload shape не даёт ценности, но увеличивает сложность live code, UI и agent context.

## Decision

1. Для `Registry V2` принимается **big-bang cleanup**:
   - live authority строится только от session-backed user bearer auth;
   - legacy actor/publisher headers не поддерживаются;
   - controller маппит public ошибки только через typed `RegistryGovernanceError`;
   - runtime/admin не держат fallback на legacy `*_actor` и `stage/gate` keys.
2. Исторические registry audit rows нормализуются **миграцией**, а не runtime compatibility-слоем.
3. Principal-based naming (`owner`, `owner_principal`, `publisher`) считается каноническим для live code, read-side и docs.
4. Registry artifacts живут на storage-backed contract (`artifact_storage_key`, `artifact_download_url`) без выдачи local filesystem path клиентам.
5. Оставшиеся `artifact_url` / `artifact_path` вне registry governance допускаются только как часть build/release subsystem и не считаются registry compatibility obligations.

## Consequences

### Плюсы

- Уменьшается объём live code и agent context: нет второго старого registry contract.
- Public/admin/runtime читают один и тот же typed payload shape.
- Ошибки и права предсказуемо маппятся по типу, а не по строковым эвристикам.
- Registry reset можно считать закрытым по факту кода, миграций и docs, а не как perpetual transition.

### Компромиссы

- Pre-migration registry audit payload shape больше не поддерживается runtime.
- Любые старые данные должны быть приведены миграцией до запуска нового runtime.
- Если в будущем понадобится historical replay старого payload shape, это должен быть отдельный offline/import path, а не возврат legacy fallback в live code.

## Closeout

Registry reset считается закрытым для registry surface в следующем объёме:

- RTK-001..RTK-010 закрыты кодом, миграциями и updated docs;
- runtime compatibility для legacy registry payload сознательно не сохраняется;
- канонические правила для дальнейшей разработки живут в `docs/modules/module-authoring.md`, `docs/modules/manifest.md` и связанных ADR.
