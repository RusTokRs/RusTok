# `commerce` как root-модуль семейства ecommerce и матрёшечная модель подмодулей

- Date: 2026-03-25
- Status: Accepted

## Context

После распила `rustok-commerce` на `product`, `pricing` и `inventory` возник второй архитектурный вопрос:

- как сохранить старый `commerce` главным модулем направления;
- как при этом не вернуть жирный монолит;
- как в будущем позволить заменять дефолтные подмодули своими реализациями по аналогии с Medusa-подходом
  и provider-подмодулями вроде `payment -> stripe`.

Нужно было развести три уровня:

- root family module;
- domain submodules;
- provider/submodule hierarchy внутри отдельных domain modules.

## Decision

Зафиксировать модель `matryoshka` для ecommerce-направления:

- `rustok-commerce` остается корневым platform module семейства `ecommerce`;
- `rustok-product`, `rustok-pricing`, `rustok-inventory` являются дефолтными подмодулями этого семейства;
- следующие domain modules (`cart`, `order`, `customer`, `payment`, `fulfillment`, ...) строятся по той же схеме;
- provider-подмодули живут уровнем ниже, например `payment -> stripe`, `payment -> custom-psp`.

Роль `rustok-commerce`:

- umbrella/root module семейства;
- orchestration и compatibility facade;
- верхняя документационная и runtime entry point для ecommerce family.

Роль дочерних модулей:

- владеть своим доменом и storage;
- выступать дефолтной реализацией capability-слота внутри ecommerce family.

Важное ограничение:

- `rustok-commerce` является корнем семейства в архитектурном и runtime-смысле;
- но он не должен быть нижней shared dependency для своих же дочерних модулей;
- shared DTO/contracts/helpers остаются в отдельном support crate (`rustok-commerce-foundation`), чтобы
  не создавать циклы зависимостей.

## Consequences

Положительные:

- появляется понятная иерархия `family -> submodules -> provider submodules`;
- старый `commerce` сохраняет роль главного модуля направления;
- сохраняется путь к заменяемым подмодулям и provider-модели.

Отрицательные:

- root-модуль нельзя путать с нижним shared base crate;
- для настоящей заменяемости в runtime позже придется ввести capability/provider selection в manifest/runtime.

Follow-up:

- описать capability slots семейства `commerce`;
- определить, как manifest будет выбирать дефолтный provider подмодуля;
- не допускать прямого схлопывания `rustok-commerce-foundation` обратно в umbrella crate.
