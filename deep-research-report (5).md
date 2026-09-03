# Аудит RusTok: работоспособность коннектора, код-ревью и готовность к production

## Executive summary

**Вердикт: `NO-GO` для production в текущем состоянии.** Это не означает, что архитектурная база RusTok плохая: напротив, в проекте уже есть сильные механизмы изоляции Alloy/Rhai/WASM, default-deny capability broker, лимиты конкурентного исполнения, `cargo audit`, `cargo-deny`, SBOM, provenance, миграционные smoke-тесты и достаточно развитая CI-инфраструктура. Однако на проверенном снимке репозитория есть как минимум **один подтверждённый P0 security blocker**, несколько high-risk дефектов/пробелов и один прямо падающий governance gate на `main`. fileciteturn47file0L3-L7 fileciteturn60file0L1-L5

Аудит привязан к снимку `main` с SHA **`b9356dbed59425468ac147cdd4147cf6932293ad`**: GitHub Actions run `Repository Ruleset Contract` для этого SHA завершился `failure` 3 сентября 2026 года. fileciteturn60file0L1-L5

Под словом **«коннектор»** я рассматриваю фактический интеграционный тракт, который сам RusTok документирует как Alloy HTTP adapters:

`apps/server → Alloy → rustok-sandbox → CapabilityBroker → platform.http → 1C/SAP/CRM/Telegram/legacy API`.

В репозитории не обнаружен отдельный production-модуль, буквально называющийся `connector`. README прямо позиционирует Alloy как механизм runtime-интеграций и sandboxed HTTP adapters для 1C, SAP, CRM, Telegram и legacy backends; `alloy` использует общий `rustok-sandbox`, а внешние `http_*` функции регистрируются на конкретный sandbox request и проходят через `platform.http`. citeturn27view0 citeturn28view0

**Главный production blocker вне самого connector path — открытая P0-проблема авторизации.** Issue #2680 подтверждает, что tenant-scoped permissions способны читать или изменять host-global operational state: в частности, глобальный event delivery profile и глобальную диагностическую информацию. В issue прямо указано, что в `AuthContext` нет отдельной типизированной platform/root authority, а обычного tenant equality check недостаточно. Issue остаётся открытым. До устранения этого дефекта либо отключения соответствующих transport surfaces fail-closed production-релиз считать безопасным нельзя. fileciteturn47file0L3-L7

**Самая серьёзная найденная мной ошибка в sandbox execution path** находится в `crates/rustok-sandbox/src/runtime.rs`: результат выполнения уже может быть успешным, после чего `ExecutionObserver` вызывается с `.await?`. Ошибка observer-а превращает уже выполненную операцию в `Err` для вызывающей стороны. Если внешний caller считает `Err` основанием для retry, side effect может выполниться повторно. В ветке ошибки observer, в свою очередь, способен замаскировать первоначальную ошибку executor-а. Это не классическая data race, но это опасная **semantic race между фактом выполнения side effect и фиксацией его результата**. fileciteturn50file0L1-L5

**Второй серьёзный blocker — governance.** Единственный активный repository ruleset, возвращённый GitHub API, защищает default branch от deletion/non-fast-forward и включает Copilot review, но **не содержит `pull_request` и `required_status_checks` rules**. Одновременно issue #1837 по активации полноценной защиты `main` остаётся открытым и явно требует `Migration harness approval` и `Repository ruleset contract`. Сам `Repository Ruleset Contract` на проверенном SHA сейчас красный. Следовательно, собственная модель репозитория говорит: production branch governance ещё не доведён до требуемого состояния. fileciteturn58file0L1-L5 fileciteturn59file0L1-L5 fileciteturn48file0L3-L7 fileciteturn60file0L1-L5

**Третий блок — CI supply chain.** В основном `ci.yml` глобально выдаются `issues: write`, `security-events: write`, `attestations: write`, `id-token: write`, а множество actions используются по mutable tag/ref: `actions/checkout@v7`, `dtolnay/rust-toolchain@stable`, `Swatinem/rust-cache@v2`, `crate-ci/typos@master`, `EmbarkStudios/cargo-deny-action@v2`, `anchore/sbom-action@v0` и другие. Это особенно нежелательная комбинация: mutable third-party code + write/OIDC token. GitHub рекомендует минимальные `GITHUB_TOKEN` permissions и pin third-party actions на полный commit SHA; GitHub также поддерживает policy, принудительно требующую full-length SHA. fileciteturn53file0L1-L5 citeturn23search4turn23search0

**Outbound HTTP security сделана хорошо на одном уровне, но не полностью.** `HttpCapabilityConstraints` требует непустые host/method/path-prefix allowlists и сверяет host, method и URL path; это сильнее типичного unrestricted webhook connector. Однако на этом policy layer отсутствуют явная проверка `http/https` scheme, проверка resolved IP против loopback/private/link-local/metadata ranges и контроль redirect destination. Поэтому безопасность от SSRF зависит от реализации downstream HTTP broker/client — а её runtime-поведение с реальным DNS/redirect transport я подтвердить не смог. Это следует рассматривать как **HIGH / условную уязвимость**, пока отдельные SSRF regression tests не докажут обратное. OWASP прямо рекомендует disable automatic redirects для SSRF-sensitive client, проверять IP/domain и дополнять application allowlist сетевым egress control. fileciteturn52file0L1-L2 citeturn24search3

**Dependency security сейчас существенно лучше, чем можно было бы предположить по старым issues.** На проверенном SHA в `deny.toml` advisory-ignore пуст, а `.cargo/audit.toml` содержит только две временные lockfile-only exceptions: `RUSTSEC-2023-0071` (`rsa`) и `RUSTSEC-2026-0235` (`rkyv`), обе с expiry **13 сентября 2026 года**. Реестр утверждает, что обе зависимости отсутствуют из locked all-feature/all-target inverse tree; это нужно повторно доказать перед production и не продлевать автоматически. Ранее актуальные `rustls-webpki` 0098/0099/0104 уже записаны как закрытые с resolved `rustls-webpki 0.103.13`, что соответствует patched thresholds RustSec. fileciteturn54file0L1-L5 fileciteturn55file0L1-L5 fileciteturn56file0L1-L5 citeturn32view0turn32view1turn32view3

**Фактический локальный Linux/Docker запуск в моей среде — `непроверяемо`.** Исходный checkout через доступное локальное execution environment не удалось получить из-за сетевого ограничения среды, поэтому я не буду выдавать статический анализ за выполненный `cargo test`. Вместо этого я сверил код напрямую через GitHub, workflow configuration, текущие issues и GitHub Actions evidence и ниже даю детерминированный набор Linux x86_64/Docker-команд для повторной проверки. Проверка реальных 1C/SAP/CRM/Telegram endpoints также **непроверяема без credentials, allowlist configuration и доступности upstream systems**.

Итоговая оценка по направлениям:

| Направление | Оценка | Вывод |
|---|---:|---|
| Архитектурная база | **7.5/10** | Хорошая модульность, sandbox/capabilities/outbox и возможность selective extraction. citeturn27view0turn28view0turn28view1 |
| Connector isolation | **7/10** | Хороший default-deny host/method/path contract; SSRF/egress transport надо доказать тестами. fileciteturn52file0L1-L2 |
| Error semantics | **5/10** | Observer может изменить business result после выполнения side effect. fileciteturn50file0L1-L5 |
| CI/testing | **7/10** | Хороший охват Rust/migrations/audit/SBOM; clippy feature-gap и supply-chain hardening требуют исправлений. fileciteturn53file0L1-L5 |
| Security governance | **4/10** | Открытый P0, production ruleset incomplete, governance workflow красный. fileciteturn47file0L3-L7 fileciteturn60file0L1-L5 |
| Production readiness | **NO-GO** | Нужны P0/P1 remediation, полноценный integration/load/security gate и restore drill. |

## Объект аудита, методика и воспроизводимость

RusTok — большой Rust monorepo: корень содержит `apps`, `crates`, `tests/e2e-rust`, `.athanor/adapters`, `.github`, `ops`, `RUNBOOKS`, `SPECS` и другие инфраструктурные элементы. README описывает default deployment как modular monolith с возможностью selective extraction тяжёлых доменов через gRPC/event streams. `apps/server` является composition root для Axum, GraphQL, auth/RBAC, outbox и module manifests. citeturn27view0

Alloy имеет достаточно ясную ownership boundary: он владеет scripts/source storage, scheduling, hooks, execution history и transport surfaces, но production sandbox должен брать из `rustok-sandbox`, а не создавать второй execution/security runtime. В README `alloy` перечислены `AlloyHttpRuntime`, `ScriptExecutor`, `ScriptOrchestrator`, `PhaseCapabilities`, `HttpCapabilityBridge` и `create_sandbox_rhai_executor`; external `http_*` calls передаются общему host broker под grant `platform.http`. citeturn28view0

`rustok-sandbox` документирует default-deny capability checks по execution/subject/tenant/actor/phase/trace context, typed HTTP host/method/path grants, bounds на capability calls/input size/rate, redacted evidence, cancellation и global/executor/tenant/artifact admission gates. Важный для масштабирования факт: synchronous Rhai/WIT broker bridging документирован как **один native thread на execution**, поэтому concurrency limits здесь являются не просто защитой от tenant abuse, а capacity-control mechanism. citeturn28view1

Текущий admission controller по умолчанию разрешает одновременно `global=64`, `per_executor=32`, `per_tenant=16`, `per_artifact=8`; `admit()` не ждёт semaphore/queue, а синхронно проверяет счётчики под `Mutex` и немедленно возвращает `LimitExceeded`. `AdmissionPermit::drop` освобождает счётчики, что является хорошим RAII-подходом. fileciteturn51file0L1-L5

**Что проверено непосредственно:**

| Объект | Статус |
|---|---|
| README, структура репозитория, Alloy/sandbox contracts | Проверено по source snapshot. citeturn27view0turn28view0turn28view1 |
| `runtime.rs`, `admission.rs`, `capability.rs`, phase bridge | Проверено по source snapshot `b9356d…`. fileciteturn50file0L1-L5 fileciteturn51file0L1-L5 fileciteturn52file0L1-L2 fileciteturn57file0L1-L5 |
| GitHub Actions CI | Проверено по workflow source. fileciteturn53file0L1-L5 |
| Repository rulesets | Проверено live через GitHub API. fileciteturn58file0L1-L5 fileciteturn59file0L1-L5 |
| Текущий governance run | Проверено: `failure` на audited SHA. fileciteturn60file0L1-L5 |
| Security issues | Проверено; P0 #2680 открыт. fileciteturn47file0L3-L7 |
| RustSec exception policy | Проверено по текущим configs/register. fileciteturn54file0L1-L5 fileciteturn56file0L1-L5 |
| Компиляция Linux x86_64 в моей execution environment | **Непроверяемо** из-за невозможности получить checkout в локальный runner |
| `docker compose up` в моей execution environment | **Непроверяемо** по той же причине |
| Реальные 1C/SAP/CRM/Telegram calls | **Непроверяемо** без credentials/upstream network |
| DNS rebinding/redirect behaviour фактического HTTP transport | **Непроверяемо**, необходим dynamic integration test |

Для воспроизводимости проверку следует выполнять именно с audited SHA, а потом повторить на предполагаемом release SHA:

```bash
git clone https://github.com/RusTokRs/RusTok.git
cd RusTok

git checkout b9356dbed59425468ac147cdd4147cf6932293ad

git status --short
git rev-parse HEAD
rustc --version
cargo --version
docker version
docker compose version
```

Базовая compile/lint/test последовательность для Linux x86_64 должна максимально повторять CI. Основной CI уже запускает `cargo check --workspace --all-targets --all-features`, Nextest с all-features, PostgreSQL migration smoke, docs, `cargo audit`, cargo-deny и coverage. При этом отдельный Clippy job сейчас запускается **без `--all-features`**, что оставляет lint gap для optional paths. fileciteturn53file0L1-L5

```bash
# Проверка форматирования и full-feature compilation
cargo fmt --all -- --check

cargo check \
  --locked \
  --workspace \
  --all-targets \
  --all-features

# Рекомендованный вариант сильнее текущего CI:
cargo clippy \
  --locked \
  --workspace \
  --all-targets \
  --all-features \
  --no-deps \
  -- -D warnings

# Точечно наиболее критичные для connector path crates
cargo test --locked -p rustok-sandbox
cargo test --locked -p alloy

# После установки cargo-nextest
cargo nextest run \
  --locked \
  --workspace \
  --all-targets \
  --all-features \
  --no-fail-fast
```

Dependency/security gate стоит запускать в точности вместе с repository-specific verifier. Текущий реестр прямо требует проверки inverse dependency trees и запрещает просто удалять lockfile blocks для подавления advisories. fileciteturn56file0L1-L5

```bash
node scripts/verify/verify-advisory-exceptions.mjs
node scripts/verify/verify-dependency-feature-hygiene.mjs

cargo tree \
  --locked \
  --workspace \
  --all-features \
  --target all \
  -i rsa

cargo tree \
  --locked \
  --workspace \
  --all-features \
  --target all \
  -i rkyv

cargo audit
cargo deny check advisories --all-features
cargo deny check --all-features
```

Для `RUSTSEC-2023-0071` важно не путать наличие package entry в `Cargo.lock` с runtime reachability. RusTok register утверждает, что `rsa` находится только в lockfile-only optional SQLx MySQL path, который не выбирается поддерживаемой feature graph; RustSec при этом подтверждает, что сама vulnerability — timing side-channel при private RSA operation, CVSS 5.9, и patched release пока нет. Поэтому корректный production gate — не «игнорировать навсегда», а заново доказать пустой inverse tree и дать waiver истечь 13 сентября 2026 года, если upstream path уже можно удалить. fileciteturn54file0L1-L5 fileciteturn56file0L1-L5 citeturn32view2

## Работоспособность коннектора: запуск, конфигурация и интеграционные сценарии

Текущий интеграционный поток выглядит следующим образом:

```mermaid
flowchart LR
    Client[REST / GraphQL / Scheduler / Workflow]
    Server[apps/server]
    Alloy[Alloy runtime]
    Sandbox[SandboxRuntime]
    Admission[AdmissionController]
    Executor[Rhai / WASM executor]
    Broker[CapabilityBroker]
    Policy[platform.http constraints]
    Host[Host HTTP implementation]
    External[1C / SAP / CRM / Telegram / Legacy API]
    Observer[ExecutionObserver]
    DB[(PostgreSQL)]
    Outbox[Transactional Outbox]
    Iggy[Iggy / async consumers]

    Client --> Server
    Server --> Alloy
    Alloy --> Sandbox
    Sandbox --> Admission
    Sandbox --> Executor
    Executor --> Broker
    Broker --> Policy
    Policy --> Host
    Host --> External
    Sandbox --> Observer
    Server --> DB
    Server --> Outbox
    Outbox --> Iggy
```

Это соответствует documented architecture: Alloy используется сервером, MCP и workflow integrations, production execution идёт через `SandboxRuntime`, а external HTTP bridge делегируется shared sandbox host under `platform.http`. Основной README также документирует transactional outbox и selective gRPC/event-stream extraction. citeturn28view0turn27view0

**Phase isolation выглядит разумно.** В текущем `PhaseCapabilities`:

| Phase | Validation | DB | External |
|---|---:|---:|---:|
| `Before` | ✓ | — | — |
| `After` | — | ✓ | — |
| `OnCommit` | — | — | ✓ |
| `Manual` | ✓ | ✓ | ✓ |
| `Scheduled` | ✓ | ✓ | ✓ |

Это хороший базовый дизайн: `Before` не может случайно делать внешние side effects, а `OnCommit` специально получает external service capability. Однако unit test в том же файле фиксирует только `Before` и `OnCommit`; `After`, `Manual`, `Scheduled` не покрыты явными assertions. Для security-sensitive capability matrix этого недостаточно. fileciteturn57file0L1-L5

**Development configuration работает как dev-конфигурация, но не должна попадать в production.** `.env.dev.example` сам предлагает запуск через Compose, использует PostgreSQL user/password `rustok/rustok`, `RUST_LOG=debug`, development JWT/refresh placeholders и включает debug panel. Это нормально для dev, но production bootstrap должен fail-fast обнаруживать эти известные значения. citeturn28view2

Стандартный dev bootstrap из самого репозитория:

```bash
cp .env.dev.example .env.dev

docker compose \
  --env-file .env.dev \
  -f docker-compose.yml \
  -f docker-compose.full-dev.yml \
  config

docker compose \
  --env-file .env.dev \
  -f docker-compose.yml \
  -f docker-compose.full-dev.yml \
  up -d

docker compose \
  --env-file .env.dev \
  -f docker-compose.yml \
  -f docker-compose.full-dev.yml \
  ps

curl -fsS http://127.0.0.1:5150/api/openapi.json

docker compose \
  --env-file .env.dev \
  -f docker-compose.yml \
  -f docker-compose.full-dev.yml \
  logs --no-color --tail=200
```

`.env.dev.example` действительно документирует Compose invocation и server port `5150`; основной CI также считает `/api/openapi.json` readiness endpoint и ждёт его после запуска `rustok-server`, что даёт полезный подтверждённый smoke contract. citeturn28view2 fileciteturn53file0L1-L5

Для чистого Linux x86_64 smoke test без frontend-части я бы применил тот же pattern, что уже используется в CI:

```bash
export DATABASE_URL='postgres://postgres:postgres@127.0.0.1:5432/rustok_test'

docker run --rm -d \
  --name rustok-postgres-test \
  -e POSTGRES_USER=postgres \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=rustok_test \
  -p 127.0.0.1:5432:5432 \
  postgres:16

until docker exec rustok-postgres-test pg_isready -U postgres; do
  sleep 1
done

cargo build --locked -p rustok-server
./target/debug/rustok-server > /tmp/rustok-server.log 2>&1 &
SERVER_PID=$!

for i in $(seq 1 60); do
  curl -fsS http://127.0.0.1:5150/api/openapi.json >/dev/null && break
  sleep 2
done

curl -fsS http://127.0.0.1:5150/api/openapi.json >/dev/null

kill "$SERVER_PID"
docker rm -f rustok-postgres-test
```

Именно PostgreSQL 16 service, server build и readiness probe через `/api/openapi.json` уже применяются в CI, поэтому этот сценарий не является выдуманным новым launch contract. fileciteturn53file0L1-L5

**Connector-specific integration suite должна быть значительно жёстче обычного happy path.** Минимальный acceptance matrix:

| Сценарий | Ожидаемый результат |
|---|---|
| Allowed host + allowed method + allowed path | Вызов проходит |
| Allowed host + forbidden method | `CapabilityConstraintDenied` до HTTP dispatch |
| Allowed host + forbidden path | Denied до dispatch |
| Forbidden host | Denied до DNS/connect |
| `Before` hook пытается сделать HTTP | Denied |
| `OnCommit` делает разрешённый HTTP call | Разрешено |
| HTTP 500/429 | Typed retryable/non-retryable result; не panic |
| Connect timeout | Controlled error + metric |
| Response > configured maximum | Resource-limit error |
| Redirect allowed.example → `127.0.0.1` | Должен быть заблокирован |
| Redirect → metadata address | Должен быть заблокирован |
| Allowlisted DNS name резолвится в RFC1918/loopback | Должен быть заблокирован либо разрешён только explicit private-service policy |
| IPv6 `::1`, link-local | Denied |
| Script cancelled во время outbound call | Call прекращается/результат помечается cancelled |
| Observer storage unavailable после successful side effect | Внешний side effect **не повторяется** |
| Duplicate idempotency key | Внешний side effect выполняется максимум один раз |
| 65-й execution при default global limit 64 | Предсказуемый overload response, не hang |

Сейчас `HttpCapabilityConstraints` проверяет URL parsing, exact host allowlist, method и path prefix. Но security policy на этом уровне не проверяет scheme/resolved destination. Это как раз тот класс кейсов, который OWASP SSRF guidance рекомендует закрывать application-level allowlisting плюс network-level restrictions и запретом redirect-following. fileciteturn52file0L1-L2 citeturn24search3

**Что именно остаётся `непроверяемо`:** точный E2E route создания/запуска Alloy script с production auth context; реальные credentials; upstream TLS/client-certificate setup; реальная redirect policy downstream HTTP client; DNS resolver policy; egress firewall; SAP/1C-specific auth refresh/retry semantics. Для production acceptance эти проверки нужно выполнять в isolated staging network с mock server первым этапом и реальным non-production upstream вторым этапом.

## Детальный код-ревью и реестр проблем

Ниже severity означает не «CVSS», а production priority: **Критично** — блокирует релиз; **Высоко** — потенциальная security/data-integrity/availability авария; **Средне** — значимый reliability/operability gap; **Низко** — hardening/testability/developer-quality issue.

| Приоритет | Проблема | Местоположение | Влияние | Рекомендация |
|---|---|---|---|---|
| **Критично** | Tenant authority используется для host-global operations | `crates/rustok-events-module/admin/...`, `apps/server/src/graphql/system.rs`, issue #2680 fileciteturn47file0L3-L7 | Tenant admin может получить host-global diagnostics или менять global event delivery profile; собственный issue маркирует это P0 | Ввести отдельный typed platform principal/authority; до этого отключить эти transports fail-closed |
| **Высоко** | Observer failure меняет результат уже выполненного execution | `crates/rustok-sandbox/src/runtime.rs` fileciteturn50file0L1-L5 | Успешный side effect может быть возвращён как `Err`, а retry способен повторить действие; observer также маскирует исходную execution error | Разделить business result и telemetry/audit persistence; durable audit сделать через transactional/outbox contract |
| **Высоко** | `main` не требует PR/status checks через активный ruleset | Repository Rules API, issue #1837 fileciteturn58file0L1-L5 fileciteturn48file0L3-L7 | Изменение может попасть на production branch без требуемого verified gate | Required PR + exact required checks + no permanent bypass |
| **Высоко** | Governance gate сейчас красный | `.github/workflows/repository-ruleset-audit.yml` run на `b9356d…` fileciteturn60file0L1-L5 | Собственный production governance contract не выполняется | Сделать этот workflow зелёным до release freeze |
| **Высоко** | Mutable Actions refs + глобальные write/OIDC permissions | `.github/workflows/ci.yml` fileciteturn53file0L1-L5 | Compromised/moved action ref получает существенно больший blast radius | Full-SHA pinning; global `contents: read`; elevated permissions только в attestation/SARIF jobs. GitHub рекомендует именно это. citeturn23search4turn23search0 |
| **Высоко, условно** | Неполный SSRF boundary | `rustok-sandbox/src/capability.rs` fileciteturn52file0L1-L2 | Redirect/DNS rebinding/private network access потенциально могут обойти исходный host check | `https` policy, resolved-IP validation, redirect disabled/revalidated, egress proxy/firewall. citeturn24search3 |
| **Средне** | `queue_time_ms` фактически не queue time | `runtime.rs` + `admission.rs` fileciteturn50file0L1-L5 fileciteturn51file0L1-L5 | Метрика включает validate/get/admit/Started observer, но admission вообще не ждёт очередь; autoscaling/latency diagnostics будут вводить в заблуждение | Переименовать либо измерять `validation`, `admission_wait`, `observer_latency`, `execution` отдельно |
| **Средне** | Overload = немедленный reject, нет управляемой очереди/backpressure | `admission.rs` fileciteturn51file0L1-L5 | При burst нагрузке резкий рост user-visible errors; особенно заметно при 1 native thread/execution | Ввести bounded queue либо явно закрепить fail-fast semantics + Retry-After + circuit breaker |
| **Средне** | Clippy не проверяет optional features | `.github/workflows/ci.yml` fileciteturn53file0L1-L5 | WASM/other optional code может пройти compile/test, но не пройти одинаковый lint quality gate | Добавить `--all-features` |
| **Средне** | Две security exceptions истекают 13.09.2026 | `.cargo/audit.toml`, advisory register fileciteturn54file0L1-L5 fileciteturn56file0L1-L5 | После даты либо gate должен падать, либо waiver будет продлён без fresh evidence | До release повторно доказать inverse tree; предпочтительно убрать exception, а не продлевать |
| **Средне** | Dev secrets/debug config опасны при accidental promotion | `.env.dev.example`, Compose citeturn28view2turn28view3 | Предсказуемые credentials, debug logging/panel, data ports могут оказаться публичными | Prod profile отдельно; startup rejection known defaults; secret manager/Compose secrets; private networks |
| **Средне** | Нет доказанного обязательного connector-specific SSRF/failure E2E gate | Основной CI запускает broad workspace tests, но отдельный outbound connector security suite в просмотренном `ci.yml` не выделен. fileciteturn53file0L1-L5 | Regression в redirect/DNS/timeout policy может попасть в release | Добавить отдельный `connector-contract` required job |
| **Средне** | One-thread-per-execution требует capacity proof | `rustok-sandbox` runtime contract citeturn28view1 | При слишком либеральных лимитах возможны thread burst, context switching и memory pressure | Load test и лимиты на основе CPU/RSS, не «на глаз» |
| **Низко** | Capability phase test неполон | `crates/alloy/src/bridge/mod.rs` fileciteturn57file0L1-L5 | Ошибка в будущей phase mapping может расширить privilege | Table-driven test всех phases + negative assertions |

Особого внимания заслуживает **observer bug**. Текущий pattern эквивалентен:

```rust
let result = executor.execute(&request, host.clone()).await;

match result {
    Ok(outcome) => {
        // side effect уже мог произойти
        self.observe(success_record).await?;
        Ok(outcome)
    }
    Err(error) => {
        self.observe(failed_record).await?;
        Err(error)
    }
}
```

То есть storage/telemetry failure после execution смешивается с business execution failure. Это видно непосредственно в `SandboxRuntime::execute_with_cancellation`. fileciteturn50file0L1-L5

Для обычной observability правильнее сделать observer best-effort и сохранить первоначальный outcome:

```rust
async fn observe_best_effort(&self, record: ExecutionRecord) {
    for observer in &self.observers {
        if let Err(error) = observer.observe(&record).await {
            tracing::error!(
                execution_id = %record.execution_id,
                error = ?error,
                "sandbox execution observer failed"
            );
        }
    }
}
```

А terminal branches приблизительно так:

```rust
match result {
    Ok(mut outcome) => {
        // Заполнить metrics...
        let record = ExecutionRecord {
            status: ExecutionStatus::Succeeded,
            // ...
        };

        self.observe_best_effort(record).await;

        // Business execution остаётся успешным.
        Ok(outcome)
    }

    Err(execution_error) => {
        let record = ExecutionRecord {
            status: ExecutionStatus::Failed,
            error_code: Some(execution_error.code().to_string()),
            // ...
        };

        self.observe_best_effort(record).await;

        // Сохраняется именно первоначальная execution error.
        Err(execution_error)
    }
}
```

Но если `ExecutionObserver` — не telemetry, а **обязательный юридический/audit ledger**, делать его просто best-effort тоже неправильно. Тогда audit intent должен durable записываться **до либо атомарно вместе с business state**, после чего delivery в log/analytics выполняется через outbox. Для внешнего HTTP side effect обязателен idempotency/receipt contract. RusTok уже документирует transactional outbox и idempotency mechanisms в других частях платформы, поэтому логично применять тот же принцип и здесь. citeturn27view0

Нужны три regression test именно для этого дефекта:

```rust
#[tokio::test]
async fn successful_execution_is_not_reclassified_when_observer_fails() {
    // executor => Ok
    // terminal observer => Err
    // expected: business outcome remains Ok
}

#[tokio::test]
async fn execution_error_is_not_masked_when_observer_fails() {
    // executor => SomeDomainError
    // observer => StorageUnavailable
    // expected: returned error == SomeDomainError
}

#[tokio::test]
async fn retry_after_observer_failure_does_not_repeat_external_effect() {
    // First call produces remote receipt.
    // Observer fails.
    // Retry with same idempotency key.
    // Assert remote call count == 1.
}
```

Для **SSRF** я бы разделил policy на два слоя. Первый — capability-level syntax/authority, второй — transport-level network authority. Capability code уже хорошо делает exact host/method/path checks. fileciteturn52file0L1-L2

Минимальный transport hardening:

```rust
let client = reqwest::Client::builder()
    // Любой redirect должен пройти policy заново;
    // безопасный минимум — вообще не follow автоматически.
    .redirect(reqwest::redirect::Policy::none())
    .connect_timeout(std::time::Duration::from_secs(3))
    .timeout(std::time::Duration::from_secs(10))
    .build()?;
```

Перед dispatch следует дополнительно требовать разрешённый scheme:

```rust
match url.scheme() {
    "https" => {}
    "http" if policy.allow_plain_http => {}
    other => {
        return Err(SandboxError::CapabilityConstraintDenied {
            capability: call.capability.clone(),
            reason: format!("URL scheme `{other}` is not allowed"),
        });
    }
}
```

Но одного этого недостаточно. Нужна проверка **каждого resolved A/AAAA address** и запрет loopback/link-local/metadata/private networks, если private destination не указан явным service policy. OWASP отдельно предупреждает про DNS pinning/rebinding и рекомендует network-layer restriction как defense in depth. citeturn24search3

Для production ещё предпочтительнее не разрешать arbitrary host networking самому server process, а построить:

```mermaid
flowchart LR
    Sandbox[Sandbox / Alloy]
    Broker[Capability Broker]
    Validate[Host + Method + Path + Scheme]
    Egress[Egress Proxy / Firewall]
    DNS[Controlled DNS]
    Public[Approved External Systems]
    Private[Private / Metadata / Loopback]

    Sandbox --> Broker
    Broker --> Validate
    Validate --> Egress
    Egress --> DNS
    DNS --> Public
    Egress -. deny .-> Private
```

Тогда ошибка application validator не превращается автоматически в доступ ко всей VPC/host network. Такой application + network defense-in-depth соответствует OWASP SSRF guidance. citeturn24search3

**Логирование и privacy.** Положительный момент: `rustok-sandbox` документирует redacted capability evidence и stable error code вместо untrusted error text. Это правильное направление. Однако production regression должен автоматически проверять, что в logs/traces не оказываются `Authorization`, cookies, access/refresh tokens, DB URLs, secret values, request bodies с PII и query parameters с credentials. OWASP рекомендует не записывать напрямую access tokens, session identifiers, passwords, DB connection strings, encryption keys и sensitive PII. citeturn28view1 citeturn24search2

**Race conditions и memory safety.** В просмотренном admission code обычной data race вокруг counters нет: state защищён `Mutex`, permit release сделан через RAII. Подтверждённой memory leak в connector path я не нашёл. Тем не менее это не равно доказательству «во всём monorepo нет unsafe/race/leak»: для 120+ crates такой вывод без sanitizer/Miri/load pass был бы необоснован. fileciteturn51file0L1-L5

Дополнительный production security pass:

```bash
# Инвентаризация unsafe
rg -n '\bunsafe\b' \
  --glob '*.rs' \
  apps crates tests

# Паники/необработанные assumptions в runtime path
rg -n '\b(unwrap|expect|panic!|unreachable!)\b' \
  crates/alloy \
  crates/rustok-sandbox \
  apps/server

# Потенциальные blocking operations в async code
rg -n 'std::thread|std::fs|std::process|block_on' \
  crates/alloy \
  crates/rustok-sandbox \
  apps/server
```

После этого для поддерживаемых crates стоит добавить Miri для bounded unit-test subset и длительный RSS/thread-count soak test. При этом documented per-execution native thread делает именно thread-count/RSS soak обязательнее, чем формальный поиск `unsafe`. citeturn28view1

## Архитектура, масштабирование и эксплуатационная готовность

Сильная сторона RusTok — **не преждевременно разбитая микросервисная архитектура**. README описывает modular monolith по умолчанию с domain boundaries и возможностью selective gRPC extraction. Это хороший production default: меньше network failure modes и distributed transaction complexity, пока конкретный module не докажет необходимость независимого масштабирования. citeturn27view0

Для connector subsystem естественная единица масштабирования — не весь RusTok сразу, а **execution workers / outbound integrations**. Сейчас Alloy и sandbox удобно встроены в composition root, но нагрузочный профиль connector-а принципиально отличается от CRUD/API: scripts могут занимать CPU, создавать native threads, ждать slow upstream, получать 429 и обрабатывать большие ответы. Поэтому следующим architectural breakpoint должен быть не «вынести всё в микросервисы», а отделить bounded execution capacity.

Предлагаемая production topology:

```mermaid
flowchart TB
    LB[Load Balancer]
    API1[RusTok API replica]
    API2[RusTok API replica]
    DB[(PostgreSQL HA)]
    Pool[DB Pool / PgBouncer]
    Outbox[Transactional Outbox]
    Stream[Iggy / Event Stream]
    Worker1[Sandbox Worker]
    Worker2[Sandbox Worker]
    Broker[Capability Broker]
    Egress[Egress Proxy]
    Upstream[1C / SAP / CRM / Telegram]
    Audit[Durable Audit / Execution History]
    OTel[OpenTelemetry Collector]
    Metrics[Prometheus-compatible metrics]
    Logs[Central logs]
    Secrets[Secret Manager]

    LB --> API1
    LB --> API2

    API1 --> Pool
    API2 --> Pool
    Pool --> DB

    API1 --> Outbox
    API2 --> Outbox
    Outbox --> Stream

    Stream --> Worker1
    Stream --> Worker2

    Worker1 --> Broker
    Worker2 --> Broker
    Broker --> Egress
    Egress --> Upstream

    Worker1 --> Audit
    Worker2 --> Audit

    API1 --> OTel
    API2 --> OTel
    Worker1 --> OTel
    Worker2 --> OTel
    OTel --> Metrics
    OTel --> Logs

    API1 --> Secrets
    Worker1 --> Secrets
    Worker2 --> Secrets
```

Не обязательно реализовывать всё до первого production release. На старте sandbox worker может оставаться in-process, если load test доказывает изоляцию и admission limits. Но контракт должен позволять позднее вынести executor pool, не переписывая Alloy API. Это согласуется с уже заявленной архитектурой RusTok «embedded by default → selectively remote». citeturn27view0

**Admission/backpressure.** Текущий controller — это concurrency breaker, а не queue: при достижении лимита caller немедленно получает `LimitExceeded`. Для synchronous manual calls это может быть правильным поведением; для scheduled/workflow jobs часто лучше bounded durable queue. Поэтому я не считаю отсутствие очереди само по себе bug, но считаю ошибкой использовать `queue_time_ms` как будто очередь есть. fileciteturn50file0L1-L5 fileciteturn51file0L1-L5

Метрики нужно разделить минимум так:

```text
sandbox_validation_duration_seconds
sandbox_admission_duration_seconds
sandbox_admission_rejected_total{scope}
sandbox_active_executions{executor}
sandbox_execution_duration_seconds{executor,phase,outcome}
sandbox_observer_duration_seconds
sandbox_observer_failures_total
sandbox_capability_calls_total{capability,operation,outcome}
connector_http_duration_seconds{upstream,method,outcome}
connector_http_timeouts_total{upstream}
connector_http_retries_total{upstream,reason}
connector_http_response_bytes
connector_egress_denied_total{reason}
outbox_oldest_pending_age_seconds
outbox_pending_events
db_pool_in_use
db_pool_wait_duration_seconds
process_resident_memory_bytes
process_threads
```

В Prometheus labels **не следует** помещать unrestricted `tenant_id`, full URL, script ID или error text: это создаёт high cardinality и потенциальную privacy leak. Tenant/script correlation лучше хранить в structured logs/traces с controlled retention.

Рекомендованный initial SLO/alert matrix — это **целевые значения для обсуждения, а не существующие гарантии проекта**:

| Сигнал | Начальная цель | Alert |
|---|---:|---|
| Core API availability | ≥ 99.9%/месяц | burn-rate alerts |
| Core API p95 | < 300 ms для локальных операций без external upstream | p95 > 500 ms, 10 min |
| Connector success rate | ≥ 99.5% за вычетом классифицированных upstream business errors | < 99%, 10 min |
| Admission rejection | < 0.1% steady state | > 1%, 5–10 min |
| Observer failure | 0 в normal operation | **любое** устойчивое >0 |
| Connector timeout ratio | < 1% baseline | > 5%, 5 min |
| Outbox oldest pending age | < 5 s normal | > 30 s |
| DB pool utilization | < 80–85% sustained | > 85%, 10 min |
| Sandbox RSS | empirically bounded | > tested safe envelope |
| Thread count | bounded admission-derived envelope | > expected limit + overhead |
| Last successful backup | < 24 h | > 24 h |
| Restore drill | минимум ежеквартально | missed drill = operational incident |

Для external upstream latency не стоит обещать один глобальный `p95 < 300ms`: SAP и Telegram имеют совершенно разные latency/error domains. У каждого connector profile должны быть собственные timeout/retry/circuit-breaker budgets.

**Retries должны быть только bounded + idempotent.** Повторять GET обычно проще; POST/PUT к внешней системе можно повторять только при наличии idempotency key или remote receipt semantics. Нельзя слепо retry-ить `4xx`, certificate errors, capability denial и malformed responses. `429`, selected `5xx`, connect reset и timeout требуют отдельно классифицированной policy с exponential backoff + jitter.

**Secrets.** Development env намеренно содержит placeholders, но в production secrets не должны жить в image, Git, Compose YAML или отладочных логах. Docker рекомендует Compose secrets вместо environment variables для passwords/API keys, поскольку env values проще случайно раскрыть и они могут попадать в debugging/logging. Compose secret даётся только явно указанному service и монтируется в `/run/secrets/...`. citeturn24search0turn24search6

Следовательно, prod config должен быть примерно таким по семантике:

```text
RUST_LOG=info
ENABLE_DEBUG_PANEL=false

DATABASE_URL=<получать из secret provider>
JWT_SECRET=<secret provider>
REFRESH_TOKEN_SECRET=<secret provider>

# Не использовать:
# dev_secret_change_in_production...
# dev_refresh_secret_change_in_production
# rustok/rustok
```

И желательно добавить hard startup validation:

```rust
fn validate_production_secrets(cfg: &Config) -> anyhow::Result<()> {
    const FORBIDDEN: &[&str] = &[
        "dev_secret_change_in_production_please_use_strong_random_string",
        "dev_refresh_secret_change_in_production",
        "change-me-in-production",
        "admin12345",
    ];

    for secret in [&cfg.jwt_secret, &cfg.refresh_token_secret] {
        if secret.len() < 32 || FORBIDDEN.iter().any(|known| secret == known) {
            anyhow::bail!("unsafe production secret configuration");
        }
    }

    Ok(())
}
```

Это особенно важно, потому что `.env.dev.example` явно содержит development JWT values, `RUST_LOG=debug` и enabled debug panel. citeturn28view2

**Containers.** Production image должен быть immutable и pinned digest в deployment manifests. Docker объясняет, что tag может измениться, тогда как digest является immutable content identifier. citeturn24search4

Не:

```yaml
image: postgres:16
```

а для production-controlled release:

```yaml
image: postgres@sha256:<reviewed-and-tested-digest>
```

То же относится к RusTok application image и infrastructure dependencies после testing в staging.

**Backups.** Для PostgreSQL одного nightly `pg_dump` недостаточно, если заявляется маленький RPO. PostgreSQL официально поддерживает base backup + continuous WAL archiving для Point-in-Time Recovery; `pg_basebackup` является стандартной отправной точкой для base backups. citeturn23search17turn23search3

Практический initial target:

```text
RPO: <= 5 минут
RTO: <= 30 минут
```

но эти значения становятся реальными только после timed restore drill. Production checklist должен требовать не «backup job green», а:

```text
backup создан
WAL архивируется
backup зашифрован
копия находится вне failure domain production DB
restore выполнен в clean environment
application smoke test прошёл на restored DB
измерены фактические RPO/RTO
```

Для Iggy/libSQL/Turso backup semantics в рамках этого аудита **непроверяемы достаточно глубоко**; до release нужно явно определить, какой datastore является source of truth, какие данные можно реконструировать из PostgreSQL/outbox и какие требуют собственного backup/restore procedure.

**CI/CD hardening.** Хорошая сторона текущего workflow — уже есть format, Rust 1.96/stable checks, cargo audit, cargo-deny, `udeps`, docs, Nextest+coverage, PostgreSQL migration smoke, Next.js builds, SBOM и provenance attestation. Это серьёзная база. fileciteturn53file0L1-L5

Но permissions следует реструктурировать:

```yaml
# По умолчанию для всего workflow:
permissions:
  contents: read

jobs:
  fmt:
    permissions:
      contents: read

  clippy:
    permissions:
      contents: read

  audit:
    permissions:
      contents: read

  sbom:
    # Только job, которому это действительно нужно:
    permissions:
      contents: read
      id-token: write
      attestations: write

  # SARIF job, если есть:
  security-scan:
    permissions:
      contents: read
      security-events: write
```

GitHub рекомендует явно задавать минимальные permissions, поскольку compromised workflow/action с write permissions может эксфильтрировать secrets или изменить repository state. citeturn23search4

Action refs необходимо перевести с:

```yaml
uses: actions/checkout@v7
uses: Swatinem/rust-cache@v2
uses: crate-ci/typos@master
```

на:

```yaml
uses: actions/checkout@<FULL_REVIEWED_COMMIT_SHA>  # v7.x.y
uses: Swatinem/rust-cache@<FULL_REVIEWED_COMMIT_SHA>
uses: crate-ci/typos@<FULL_REVIEWED_COMMIT_SHA>
```

и включить repository/organization policy **Require actions to be pinned to a full-length commit SHA**. GitHub предоставляет такую enforcement setting напрямую. citeturn23search0turn23search4

Dependabot можно продолжать использовать для PR-обновления pinned SHA — SHA pinning не означает отказ от обновлений; оно означает, что обновление становится reviewable diff.

Отдельный connector gate:

```yaml
connector-contract:
  name: Connector Contract
  runs-on: ubuntu-24.04
  permissions:
    contents: read

  services:
    postgres:
      image: postgres:16
      env:
        POSTGRES_USER: postgres
        POSTGRES_PASSWORD: postgres
        POSTGRES_DB: rustok_test
      ports:
        - 5432:5432

  steps:
    - uses: actions/checkout@<FULL_SHA>
    - uses: dtolnay/rust-toolchain@<FULL_SHA>

    - name: Sandbox policy tests
      run: cargo test --locked -p rustok-sandbox

    - name: Alloy connector tests
      run: cargo test --locked -p alloy

    - name: Advisory exception contract
      run: node scripts/verify/verify-advisory-exceptions.mjs

    - name: Connector SSRF / retry / idempotency regression suite
      run: ./scripts/verify/verify-connector-contract.sh
```

Этот job должен стать required status check наряду с `CI Success`, migration governance и repository ruleset contract.

## Roadmap до production-ready

Основной принцип roadmap: сначала закрыть **security/governance/data-integrity blockers**, затем доказать connector behavior под отказами, и только потом оптимизировать масштабирование.

| Этап | Работа | Приоритет | Усилие | Критерий готовности |
|---|---|---:|---:|---|
| Немедленно | Закрыть P0 host-global authority #2680 либо отключить surfaces | **P0 / критично** | 3–7 чел.-дн. + архитектурное решение | Tenant admin получает deny; только explicit platform principal имеет host authority. fileciteturn47file0L3-L7 |
| Немедленно | Исправить observer/business-result semantics | **P0** | 1–3 чел.-дн. | Observer failure не меняет execution outcome; original errors не маскируются; idempotency test green. fileciteturn50file0L1-L5 |
| Немедленно | Активировать полноценный branch ruleset | **P0** | 0.5–1 день | Required PR + required status checks; ruleset contract green; issue #1837 закрыт. fileciteturn48file0L3-L7 |
| Немедленно | Устранить красный `Repository Ruleset Contract` | **P0** | 0.5–1 день | Audited release SHA имеет green governance run. fileciteturn60file0L1-L5 |
| Первая неделя | Least-privilege GitHub token + SHA-pin Actions | **P1 / высоко** | 1–2 чел.-дн. | Global permissions read-only; actions full-SHA pinned; enforcement enabled. citeturn23search4turn23search0 |
| Первая неделя | SSRF/egress hardening | **P1 / высоко** | 3–5 чел.-дн. | Redirect/DNS/private-IP/metadata tests green; egress network policy active. citeturn24search3 |
| До 13 сентября | Решить две текущие RustSec exceptions | **P1** | 0.5–2 чел.-дн. | Waiver removed либо fresh approved evidence; expiry не продлевается автоматически. fileciteturn54file0L1-L5 fileciteturn56file0L1-L5 |
| Первая неделя | Сделать Clippy all-features и connector contract required | **P1** | 1–2 чел.-дн. | Optional features linted; dedicated security/failure suite mandatory |
| Первая–вторая неделя | Production secret/config profile | **P1** | 2–4 чел.-дн. | Known dev secrets/debug settings rejected; external secret store; no public DB/broker ports. citeturn24search0turn28view2 |
| Вторая неделя | Load + soak + chaos tests sandbox | **P1** | 3–7 чел.-дн. | CPU/RSS/thread envelope documented; no permit drift; overload predictable |
| Вторая неделя | Metrics/dashboards/alerts | **P1** | 2–4 чел.-дн. | RED metrics + connector/admission/outbox/observer dashboards and alerts |
| Вторая–третья неделя | PostgreSQL PITR + restore drill | **P1** | 2–4 чел.-дн. | Restore in clean environment demonstrated; measured RPO/RTO. citeturn23search17turn23search3 |
| Третья неделя | Staging real-upstream validation | **P1** | 2–5 чел.-дн. | 1C/SAP/CRM/Telegram non-prod credentials, TLS, rate limit, timeout/retry test |
| После launch proof | Separate sandbox workers при необходимости | **P2** | 5–15 чел.-дн. | Решение основано на profiling, а не на заранее выбранных microservices |

**Минимальный security patch gate для P0 #2680.** Сам issue формулирует правильную модель: host-global resource нельзя защищать tenant permission вроде `SETTINGS_MANAGE` или `LOGS_READ`. Нужен отдельный principal/scope, который обычная tenant role получить не может. fileciteturn47file0L3-L7

Концептуально:

```rust
pub enum Authority {
    Tenant {
        tenant_id: Uuid,
        permissions: PermissionSnapshot,
    },
    Platform {
        principal_id: Uuid,
        scopes: PlatformScopeSet,
    },
}

pub fn require_platform_read(auth: &AuthContext) -> Result<(), AuthError> {
    match &auth.authority {
        Authority::Platform { scopes, .. }
            if scopes.contains(PlatformScope::OperationalRead) =>
        {
            Ok(())
        }
        _ => Err(AuthError::Forbidden),
    }
}
```

И regression:

```rust
#[tokio::test]
async fn tenant_admin_cannot_read_host_global_event_configuration() {
    let tenant_admin = tenant_admin_with(SETTINGS_READ | SETTINGS_MANAGE);

    let result = event_delivery_configuration_native(tenant_admin, ...).await;

    assert_forbidden(result);
}

#[tokio::test]
async fn explicit_platform_operator_can_read_host_global_configuration() {
    let operator = platform_operator(PlatformScope::OperationalRead);

    let result = event_delivery_configuration_native(operator, ...).await;

    assert!(result.is_ok());
}
```

До появления такого authority contract безопасный временный вариант из самого issue — выключить host-global admin transports fail-closed. fileciteturn47file0L3-L7

**Regression suite перед каждым production release** должна иметь следующие обязательные группы:

| Группа | Обязательные проверки |
|---|---|
| Authorization | Cross-tenant deny; platform-global deny для tenant admins; token expiry; revoked session; forged identity headers |
| Connector capabilities | host/method/path/phase positive + negative matrix; unknown fields; malformed URLs |
| SSRF | redirect, DNS rebinding simulation, IPv4/IPv6 loopback, RFC1918, link-local, metadata, forbidden scheme |
| Error semantics | executor error preserved; observer failure isolated; timeout/cancel; broker unavailable |
| Idempotency | retry POST после timeout; duplicate event; duplicate scheduler run; process restart between side effect/receipt |
| Resource limits | max input/output, Rhai operation limits, recursion/depth, concurrency by global/tenant/artifact |
| Load | steady, burst, slow upstream, 429 storm, 5xx storm; thread/RSS/CPU under saturation |
| Persistence | migration zero→head, incremental migration, DB restart, outbox replay |
| Privacy | no access token/password/DB URL/secret/request PII in logs/traces. OWASP рекомендует маскировать или исключать эти данные. citeturn24search2 |
| Recovery | PostgreSQL PITR restore; corrupted/missing upstream response; restart during in-flight connector operation |
| Supply chain | `cargo audit`, cargo-deny, waiver verifier, SBOM, action-SHA policy |

**Release checklist, который я бы сделал обязательным gate:**

- P0 #2680 закрыт либо affected transports физически отключены fail-closed. fileciteturn47file0L3-L7
- `Repository Ruleset Contract`, `Migration harness approval`, `CI Success`, `Connector Contract` зелёные на **том же SHA**, который превращается в release image.
- `main` требует PR + reviews + required checks; прямой push не является обычным delivery path. Issue #1837 прямо задаёт это как желаемый процесс. fileciteturn48file0L3-L7
- GitHub Actions SHA-pinned, workflow permissions least-privilege. citeturn23search4turn23search0
- Security exceptions не expired; `cargo audit`, `cargo deny` и repository verifier green. fileciteturn54file0L1-L5 fileciteturn56file0L1-L5
- Connector SSRF suite доказала redirect/DNS/private-address isolation. citeturn24search3
- Dev defaults невозможно запустить в production mode. citeturn28view2
- Secrets находятся вне Git/image/plain Compose env; доступ минимизирован. citeturn24search0
- Production container/dependency images закреплены digest, а release artifact связан с SBOM/provenance. Immutable digest semantics документированы Docker. citeturn24search4
- SLO dashboards и alerts проверены искусственным failure injection.
- PostgreSQL backup не просто существует: выполнен реальный restore/PITR drill. PostgreSQL документирует base backup + WAL archive/PITR как штатный recovery mechanism. citeturn23search17turn23search3
- Проведён минимум один 60–120 минутный saturation/soak тест sandbox concurrency с фиксацией peak RSS, CPU, thread count, admission rejects и external latency.
- Для каждого production upstream задокументированы owner, credentials rotation, allowed hosts, allowed paths/methods, timeout, retry policy, idempotency semantics, rate limits и emergency disable switch.

После устранения P0 authority flaw, observer/result bug, branch governance gap и доказанного SSRF/egress contract я бы сменил оценку с **NO-GO** на **conditional GO for staging/canary**. Полный production `GO` должен зависеть уже не от дополнительного статического review, а от четырёх доказательств на конкретном release SHA: **green required CI, connector failure/SSRF suite, sustained load test и successful backup restore drill**. На проверенном SHA как минимум первое условие сейчас объективно не выполнено: `Repository Ruleset Contract` завершился `failure`. fileciteturn60file0L1-L5