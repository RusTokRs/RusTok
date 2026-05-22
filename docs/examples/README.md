# Каталог исполняемых примеров

Этот раздел — единая точка discoverability для примеров, smoke-сценариев и
команд воспроизведения, которые используются в документации платформы.

## Цель

- убрать разрозненные «пример-команды» из случайных документов;
- дать единый вход для операторов, DevEx и module owners;
- сделать примеры пригодными для постепенного подключения в DOC-07 quality gates.

## Формат записи примера

Каждый пример в дочерних документах должен содержать:

1. **Контекст** — где используется (module/app/guide).
2. **Команду(ы)** — минимальный runnable набор.
3. **Ожидаемый результат** — что считается успешным выполнением.
4. **Ограничения окружения** — что может блокировать запуск.
5. **Владельца** — кто отвечает за актуальность примера.

## Базовые smoke-сценарии (первый слой)

### 1) Full local stack (dev-start)

```bash
./scripts/dev-start.sh
```

Ожидаемый результат:

- backend доступен на `http://localhost:5150`;
- admin/storefront host-поверхности поднимаются в dev-profile.

Source: `docs/guides/quickstart.md`, `scripts/dev-start.sh`.

### 2) Installer preflight (без миграций)

```bash
cargo run -p rustok-server --bin rustok-server -- install preflight \
  --environment local \
  --profile dev-local \
  --database-engine postgres \
  --database-url postgres://rustok:rustok@localhost:5432/rustok_dev \
  --admin-email admin@local \
  --admin-password admin12345 \
  --tenant-slug demo \
  --tenant-name "Demo Workspace" \
  --seed-profile dev \
  --secrets-mode dotenv-file
```

Ожидаемый результат:

- preflight возвращает отчёт;
- не запускаются миграции и side-effect bootstrap шаги.

Source: `docs/guides/quickstart.md`.

### 3) Docs lint baseline

```bash
npx --yes markdownlint-cli <changed-files>
```

Ожидаемый результат:

- корректный `exit code` (`0` для pass, иначе fail).

Source: `docs/research/fix docs.md`.

## Связанные документы

- [Быстрый старт](../guides/quickstart.md)
- [План исправления документации](../research/fix%20docs.md)
- [Главный план верификации платформы](../verification/PLATFORM_VERIFICATION_PLAN.md)
