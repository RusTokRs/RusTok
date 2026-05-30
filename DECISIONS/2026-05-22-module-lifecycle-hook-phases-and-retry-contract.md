# 2026-05-22 — Module lifecycle hook phases and retry contract

## Статус

Accepted.

## Контекст

Lifecycle control-plane/module lifecycle должен исключать частичный rollback
(когда `enabled` флаг уже меняли, а hook падал) и иметь единый recovery contract.
Без явного phase-model admin/runtime surfaces по-разному интерпретируют ошибки lifecycle hooks,
а журнал `module_operations` нельзя надёжно использовать для retry/compensation.

## Решение

- Lifecycle toggle фиксируется как state machine с фазами `validated -> running -> committed -> failed`.
- `pre_enable` / `pre_disable` выполняются до tenant state commit. Ошибка pre-hook:
  - не меняет effective module state;
  - завершает operation как `failed` с диагностикой.
- `post_enable` / `post_disable` выполняются после успешного commit tenant state и считаются
  idempotent side-effects.
- Ошибка post-hook не откатывает committed state. Вместо rollback создаётся retryable issue в
  `module_operations` (через status/details), чтобы recovery шёл через повтор/compensation,
  а не через неявный state rewind.
- Legacy `on_enable` / `on_disable` трактуются как compat pre-hook слой до полного cutover на
  explicit pre/post API.

## Последствия

- GraphQL/SSR surfaces обязаны показывать единую taxonomy ошибок: pre-hook failures относятся к
  user-facing lifecycle validation failures, post-hook failures отражаются как retryable operations.
- Операционный recovery опирается на journal (`correlation_id`, status/details) и становится
  воспроизводимым без скрытых побочных откатов.
- Module owners, добавляющие side-effects в hooks, должны обеспечить идемпотентность post-фазы
  и задокументировать retry-safe поведение.
