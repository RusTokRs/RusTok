# Matryoshka / модель композиции

Этот документ сохраняет Matryoshka как концептуальную модель слоёв RusToK, но
фиксирует её в терминах текущего состояния, без отрыва от текущего кода.

## Зачем нужна модель

Matryoshka помогает не смешивать разные уровни платформы:

- foundation и runtime host
- платформенные модули
- shared/support/capability crate-ы
- UI и interaction-поверхности
- будущие federation/network layers

Это не отдельный runtime-контракт и не замена `modules.toml`. Это архитектурная
рамка, которая помогает объяснять composition.

## Слои текущего состояния

### Слой 1. Foundation-платформа

Foundation-слой включает:

- `apps/server` как composition root
- `rustok-core`
- `rustok-api`
- `rustok-events`
- `rustok-storage`
- `rustok-test-utils`

Этот слой даёт базовые контракты, runtime-wiring и host-level policy.

### Слой 2. Платформенные модули

Платформенные модули объявляются в `modules.toml` и делятся на:

- `Core`
- `Optional`

Именно этот слой описывает domain/runtime modules, а не любые crates в `crates/`.

### Слой 3. Shared domain families и support-slices

Этот слой покрывает shared family crates и module-adjacent support pieces,
например:

- `rustok-commerce-foundation`
- shared read/index/event helpers
- module-specific support-поверхности, которые не являются самостоятельными платформенными модулями

Смысл слоя — дать reuse без размывания ownership у платформенных модулей.

### Слой 4. Capability crate-ы

Capability crate-ы добавляют отдельные возможности платформы:

- `rustok-mcp`
- `rustok-ai`
- `alloy`
- `flex`
- `rustok-telemetry`
- `rustok-iggy`
- `rustok-iggy-connector`

Они участвуют в composition, но не входят в taxonomy `Core/Optional`, пока не
объявлены как платформенные модули.

### Слой 5. Unified UI

UI layer объединяет:

- Leptos hosts
- Next.js hosts
- module-owned UI packages
- общий UI/runtime-контракт

Здесь важно правило: UI остаётся module-owned, а хосты только монтируют и
композируют surfaces.

### Слой 6. Interaction / read-layer

Этот слой описывает:

- denormalized read models
- index/search layer
- event-driven projections
- live transport surfaces, если они нужны для interaction

Это не отдельная доменная taxonomy, а слой агрегирования и interaction-потоков.

### Слой 7. Federation / future network-layer

Этот слой остаётся vision-level направлением:

- межинстансовое взаимодействие
- federation protocols
- mesh/network scenarios

Он не считается текущим runtime baseline и не должен смешиваться с живым
контрактным слоем текущей платформы.

## Что важно в текущем состоянии

- `modules.toml` важнее conceptual-layer names, когда речь идёт об источнике истины для runtime
- платформенный модуль определяется через manifest и registry, а не через абстрактный
  слой модели
- capability crate не становится платформенным модулем только потому, что он важен
- central docs должны описывать текущий код, а не только vision

## Как использовать эту модель

Используйте Matryoshka, когда нужно:

- объяснить место нового компонента в общей архитектуре
- не перепутать host, module, support и capability roles
- понять, на каком уровне должен жить новый контракт

Не используйте Matryoshka как замену:

- `modules.toml`
- `rustok-module.toml`
- local docs компонентов
- контракт верификации

## Связанные документы

- [Обзор архитектуры платформы](./overview.md)
- [Архитектура модулей](./modules.md)
- [Диаграммы платформы](./diagram.md)
- [Обзор модульной платформы](../modules/overview.md)
- [Реестр модулей и приложений](../modules/registry.md)
