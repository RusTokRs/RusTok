# Channel resolution pipeline и typed policy trajectory

- Date: 2026-03-27
- Status: Accepted

## Context

`rustok-channel` стартовал как v0 baseline с простым runtime order `header -> query -> host -> default`.

Этого достаточно для pilot-стадии, но недостаточно как финальная архитектура платформы:

- resolution logic жила в server middleware, а не в domain-модуле;
- explicit default channel уже появился, но следующий шаг не должен превращаться в ad-hoc `tenant-level default rules`;
- при дальнейшем росте платформы понадобятся richer resolution predicates (`host`, `oauth_app`, `surface`, `locale`), но ввод scripting/generic rule engine заранее создаст долг и размоет инварианты.

Нужно зафиксировать конечную траекторию сейчас, пока кодовая база ещё допускает архитектурный сдвиг без дорогого перелома.

## Decision

Принимается следующая финальная модель channel resolution:

1. explicit selectors;
2. built-in typed target-resolution slices;
3. tenant-scoped typed resolution policies;
4. explicit default channel;
5. unresolved request.

Ключевые решения:

- `tenant-level default rules` как отдельная архитектурная концепция не вводятся;
- terminal fallback остаётся только один: explicit default channel tenant'а;
- runtime resolution выносится из server middleware в domain-layer `rustok-channel`;
- shared runtime contract оформляется как typed pipeline:
  - `RequestFacts`
  - `ResolutionDecision`
  - `ResolutionTraceStep`
- текущий host-based lookup по `web_domain` трактуется как built-in typed resolution slice, а не как основа для generic rule engine;
- будущая конфигурация richer matching будет называться `tenant-scoped typed resolution policies` и встраиваться перед explicit default channel;
- policy layer не должен быть Turing-complete:
  только typed predicates/action model, без scripting и без произвольного eval.

`rustok-api` остаётся владельцем host-facing `ChannelResolutionSource`, а domain resolver в `rustok-channel` держит собственный resolution contract и маппится в shared host contract на server boundary.

## Consequences

Плюсы:

- precedence order теперь становится domain invariant, а не middleware detail;
- появляется typed seam для будущих policy sets без немедленного ввода rule engine;
- debug/observability можно строить на `ResolutionTraceStep`, а не на неявных ветках middleware;
- explicit default channel остаётся детерминированным terminal fallback.

Минусы и follow-up:

- в коде появляется дополнительный resolver layer и временный mapping между domain origin и host-facing source contract;
- следующий этап уже не про “ещё один fallback”, а про storage/model/admin/runtime rollout для typed policies;
- текущий built-in host slice позже нужно будет либо встроить в общий policy engine, либо явно оставить как fast-path policy family, не дублируя semantics.
