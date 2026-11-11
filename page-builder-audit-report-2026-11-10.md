# Аудит логики Page Builder RusToK — ошибки, косяки и рекомендации

**Дата:** 2026-11-10
**Ветка:** `arena/c37b5f4e-rustok` (base `dd9b1d4`)
**Объём:** `crates/modules/rustok-page-builder` (123 файла, ~38.5k LOC), движок `crates/ui/fly` + `fly/browser` (~45k LOC), хост-интеграция `crates/modules/rustok-pages` (`services/page/*`, `admin/*`), `apps/admin/src/main.rs` (intent endpoint), Next.js поверхности `apps/next-admin/packages/pages` и `apps/next-frontend/packages/rustok-pages`, ADR 2026-07-13 / 2026-10-09 / 2026-10-14 / 2026-10-16, backlog `docs/modules/page-builder-backlog.md`.
**Метод:** статический проход по исполняемым путям (publish-пайплайн, санитизация, intent-протокол, draft-сессии, авторизация, undo/history, кодек, рантайм действий, scheduled publish), проверка 6 Universal Logic Dimensions (AGENTS.md §2.1). Изменений в коде не делал; тесты и компиляцию не запускал — это чисто ревью, все ссылки на файлы/строки проверены по текущему HEAD.

> Отчёт — не фикс. Каждый пункт снабжён путём к файлу и строкой. Приоритеты: **P1** — функциональный баг / безопасность / конкурентность, **P2** — качество, производительность, целостность контрактов, **P3** — заметки.

---

## 1. Executive Summary

Page Builder — самая зрелая подсистема из аудированных мной в этом репозитории. Пайплайн публикации образцовый: fail-closed политика, канонические SHA-256 хэши, верификация целостности артефактов, повторная валидация после runtime-материализации, идемпотентные receipts, CAS по ревизиям, транзакционный outbox. Экранирование HTML/CSS/JSON-LD корректно во всех проверенных местах.

Тем не менее найдено **7 дефектов уровня P1** и ~16 позиций P2/P3. Главные темы:

* **Авторизация построена на строках ролей, а не на каноническом RBAC** — прямое нарушение инварианта `rustok-rbac` из AGENTS.md. Кастомные роли с реальными грантами `pages:update` получают read-only, а роль `manager` может редактировать, но **не может сохранить ни одного изменения** (каждая правка падает с Forbidden на автосейве).
* **Синхронная расходимость двух сторфронтов**: рантайм действий на Next.js — подмножество рантайма на Rust; табы/аккордеоны/слайдеры, собранные в билдере, на Next.js не работают.
* **`srcset` валидируется ~150 строками грамматики, но рендерер его выбрасывает** — валидация мертва относительно выхода, README утверждает обратное.
* **Конкурентность draft-сессий атомарна только в памяти**: файловый и KV бэкенды — классический check-then-act, CAS по generation проигран на мультиподной сборке.
* **Scheduled publish может навсегда застрять в `executing`** после крэша воркера — рипера нет.

Ниже — находки, сгруппированные по доменам.

---

## 2. Авторизация и безопасность (P1)

### 2.1. Авторизация по имени роли вместо канонического RBAC [P1]

**Где:** `crates/modules/rustok-pages/admin/src/access.rs:30-46` (`pages_editor_permissions_for_role`: `Some("super_admin") | Some("admin") => full`, `manager => без publish`, прочее — read-only), `crates/modules/rustok-pages/admin/src/builder.rs:419-426` (`page_builder_permissions_for_role` синтезирует `Vec<Permission>` из строки роли), `apps/admin/src/main.rs:96-99`.

**Проблема:** инвариант AGENTS.md (Layer 1, `rustok-rbac`): *«MUST NOT hardcode role checks (`if role == "admin"`) or local permission tables»* — здесь это сделано дословно. Реальный набор разрешений пользователя (`rustok-rbac`) никогда не запрашивается: `fetch_current_user` возвращает только `role`, из неё синтезируются `Permission::new(Resource::Pages, …)`.

Последствия:
- кастомная роль `editor` с настоящим грантом `pages:update` получает read-only в билдере — функционально сломано и молча;
- переименование ролей ломает редактор незаметно (unknown → read-only);
- постраничная гранулярность (owner-scope) на этом уровне отсутствует — её спасает только `enforce_owned_scope` в самом `PageService` (data-слой проверяет корректно, см. `services/page/document.rs:52-57`);
- fail-closed для unknown ролей — единственное, что сделано правильно.

**Рекомендация:** в intent-endpoint'е и фасаде запрашивать эффективные разрешения пользователя через канонический RBAC-порт и маппить `CapabilityState` из них (`pages:read` → чтение, `pages:update` → edit/properties/assets, `pages:publish` → publish). Ролевой маппинг оставить только как явно задокументированный fallback для хостов без RBAC-порта.

### 2.2. Роль `manager`: редактировать можно, сохранить нельзя [P1]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:155-173` (синтезированный `save`), `crates/modules/rustok-page-builder/admin/src/capability_access.rs:135` (`BrowserIntentKind::Save => vec![EditorCapability::Publish]`), `builder.rs:419-426` (manager → только `pages:read`).

**Проблема:** цепочка. (1) Preflight пропускает мутирующий интент — у manager `edit = true`. (2) После мутации `result.dirty == true` → хост **синтезирует** intent `save` (автосейв каждой правки). (3) Синтезированный конверт диспатчится через `dispatch_browser_intent` **минуя** `preflight_pages_intent` (нет ни capability-проверки, ни palette-проверки — см. §2.3). (4) Фасад доходит до `AuthorizedPageBuilderHandlers` → publish требует `pages:publish` → **Forbidden**. Итог: каждая правка менеджера возвращается ошибкой; полезной работы роль выполнить не может, а ошибка приходит поздно и не объясняет причину.

**Рекомендация:** развести семантику: сохранение черновика — это `pages:update` (draft-save), реальная публикация — `pages:publish`. В автосейве пропускать синтез `save`, если у пользователя нет соответствующей capability, оставляя канвас dirty с индикатором «не сохранено». И прогонять синтезированный конверт через тот же preflight.

### 2.3. Синтезированный save обходит capability/palette preflight [P1→P2 после фикса прав]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:155-173`; серверный контроллер канваса создаётся с полными правами: `crates/modules/rustok-page-builder/admin/src/model.rs:38` (`FlyUiStateMachine::new(Presentation::Full)`).

**Проблема:** единственное место, где `CapabilityState` пользователя проверяется — preflight **оригинального** конверта (`contribution_browser_intent.rs:84-95`). Серверный `AdminCanvasController` всегда Full. Любой новый путь, диспатчащий интенты в обход preflight (сегодня — синтезированный save; завтра — любой новый хост, вызывающий публичный `dispatch_pages_browser_intent` с `CapabilityState::full()` из `contribution_browser_intent.rs:38-41`), исполняется без capability-гейта. Защита остаётся только на фасаде (permission) — но она (а) не знает про palette-доступ, (б) падает поздно и без типизированного denial-ответа.

**Рекомендация:** сузить `Presentation`/capability у серверного контроллера до запрошенных; единая точка входа `dispatch(intent)` должна валидировать capability для **каждого** конверта, включая синтезированные.

### 2.4. `page_builder_permissions_for_role` никогда не выдаёт `pages:update` [P2, латентно]

**Где:** `crates/modules/rustok-pages/admin/src/builder.rs:419-426`; контракт возможностей: `crates/modules/rustok-page-builder/src/service.rs:23-46` (`Properties` требует `pages:update`); семантика импликации: `crates/libs/rustok-api/src/permissions.rs:284-287` (`Manage` имплицирует всё, `Publish` **не** имплицирует `Update`).

**Проблема:** список синтезированных прав содержит только `Read` и (для admin) `Publish`. Capability `Properties` (`pages:update`) недостижима ни для одной роли через `AuthorizedPageBuilderHandlers`. Сегодня латентно (фасад Pages маршрутизирует только Preview/Publish), но первый же хост, отправляющий Properties-запрос через `compose_fly_page_builder_handlers`, получит вечный Forbidden.

### 2.5. Draft-сессии не привязаны к пользователю; создание сессий read-only пользователями [P1]

**Где:** `crates/modules/rustok-page-builder/admin/src/draft_session.rs:15-24` (запись не содержит user/tenant), `:293-296` (load проверяет только `page_id`); `crates/modules/rustok-pages/admin/src/browser_intent.rs:189-193` (безусловный `commit_with_context` в конце **каждого** успешного интента, включая read-only `select` пользователя с ролью `customer`).

**Проблема (две):
1. **Ресурсная амплификация:** каждый `select` от аутентифицированного читателя создаёт новую draft-сессию с полным контроллером (весь проект). Лимит стора — только количество записей (10 000) без ограничения размера записи; проект до 16 МБ (`max_project_bytes`) → теоретически ~160 ГБ в памяти/на диске от одного низкопривилегированного пользователя.
2. **Отсутствие владельца:** токен — единственный «credential» сессии. Два редактора одного тенанта, обменявшиеся токеном (логи, referrer, скриншот), молча перехватывают чужой несохранённый черновик; аудит-атtribution отсутствует. Cross-tenant недоступен (страница fetched токеном вызывающего) — поэтому P1, а не P0.

**Рекомендация:** привязать сессию к владельцу: добавить в запись `tenant_id` + `user_id`, проверять при load/commit; не создавать сессию для пользователей без edit-capability; ввести лимиты на размер записи и на количество сессий на пользователя.

### 2.6. Файловый draft-стор пишет контент тенанта в OS temp dir [P2]

**Где:** `crates/modules/rustok-page-builder/admin/src/draft_session.rs:270-273` (`default_dir() = /tmp/rustok_draft_sessions`), `:1049-1071` (`atomic_write_file` — remove-then-rename, окно без файла при крэше; `fs::write` с дефолтными правами 0644).

**Проблема:** несохранённые черновики страниц (в т.ч. непубличный контент) лежат 8 часов в world-readable `/tmp` без tenant-скопинга; любой локальный процесс может их читать/подменять/удалять. На Unix `fs::rename` перезаписывает атомарно — предварительный `remove_file` не нужен и снижает durability.

**Рекомендация:** каталог хранилища — конфигурируемый, с правами 0700/0600 и per-tenant подкаталогами; убрать pre-remove на Unix (оставить только в Windows-ветке); добавить `fsync` перед rename.

---

## 3. Функциональные баги ядра (P1)

### 3.1. `srcset` валидируется, но никогда не рендерится [P1]

**Где:** валидация — `crates/modules/rustok-page-builder/src/static_publish_policy.rs:553-570` + `validate_srcset` (~100 строк грамматики, дескрипторы `w`/`x`, запрет `data:`, уникальность); рендер — `crates/ui/fly/src/render.rs:503`: атрибут `srcset` безусловно выбрасывается из выхода вместе с `style`/`srcdoc`/`xlink:href`/`ping`/`background`. При этом `sizes` **не** выбрасывается — в опубликованный HTML попадает осиротевший `sizes` без `srcset`.

**Проблема:** адаптивные изображения проходят publish-политику (README модуля прямо обещает «default policy permits validated `srcset`»), но в артефакте `srcset` исчезает молча. Это (а) потеря функциональности, (б) мёртвый код валидации, (в) рассинхрон документации и поведения, (г) непоследовательность `sizes`/`srcset`.

**Рекомендация:** добавить srcset в рендерер с той же per-URL проверкой (`UrlAttributeKind::ResourceImage`), либо до тех пор — выбрасывать и `sizes`, и убрать обещание из README. Покрыть end-to-end тестом «srcset в проекте → srcset в артефакте».

### 3.2. Next.js storefront: рантайм действий — подмножество Rust-рантайма [P1]

**Где:** Rust (полный): `crates/ui/fly/browser/assets/fly-actions.js` — 7 видов действий (`emit_event`, `provider_action`, `toggle_visibility`, `switch_tab`, `toggle_accordion`, `switch_slide`, `scroll_to`, `animate`) + touch/swipe + fallback `panel_id` в `findTarget`. Next.js (подмножество): `apps/next-frontend/packages/rustok-pages/src/components/fly-actions-runtime.ts` — только `emit_event`/`provider_action`/`toggle_visibility`/`scroll_to`/`animate`; нет `switch_tab`/`toggle_accordion`/`switch_slide`, нет touch-жестов, нет `panel_id`-fallback'а.

**Проблема:** компоненты «табы», «аккордеон», «слайдер», собранные в билдере, на Rust-сторфронте работают, на Next.js — мёртвые (клик ничего не делает). Это прямое нарушение Dual-Implementation Parity (AGENTS.md §5.6) и дублирование source of truth: два рантайма уже разошлись и продолжат расходиться.

**Рекомендация:** сделать `fly-actions.js` единым каноническим артефактом, который Next-пакет потребляет как статический ассет (копия в сборке с хэш-сверкой против Rust-версии), а не переписывает на TS. Добавить parity-тест, сравнивающий множество поддерживаемых `kind` в обоих рантаймах.

### 3.3. Scheduled publish: задача может навсегда застрять в `executing` [P1]

**Где:** `crates/modules/rustok-pages/src/services/page/schedule.rs:229-260` — claim это оптимистичный `scheduled → executing`; выборка задач — только `state = "scheduled"` (`:236`); никакого рипера/lease/timeout для `executing` по всему модулю нет (grep подтверждает).

**Проблема:** крэш пода после claim'а и до `published/failed` оставляет job в `executing` навсегда — публикация страницы не происходит и не повторяется. Нарушение State Transition Integrity («no entity can become permanently stranded in processing upon mid-chain failure»).

**Рекомендация:** lease-поле (`claimed_at`/`lease_until`), рипер, возвращающий просроченные `executing` в `scheduled` (с учётом `attempts < MAX`), и метрика на stranded-состояние.

---

## 4. Конкурентность и целостность (P1/P2)

### 4.1. CAS по generation в draft-сторе атомарен только в памяти [P1]

**Где:** in-memory — `draft_session.rs:169-224` (единый `RwLock`, корректно); файл — `draft_session.rs:521-577` (`commit_with_context`: read-file → check → write-file, без блокировок); KV — `draft_session.rs:737-752` (`save_record` — безусловный `set`, без compare-and-set); дефолт хоста `DurableSsrDraftSessionStore` (`:896-901`) = memory + file.

**Проблема:** классический TOCTOU. Два пода читают generation N, оба проходят проверку `expected_generation`, оба пишут N+1 — lost update, защита от конфликта поколений молча обходится. На single-pod инсталляции работает (память), на мультиподной — нет.

**Рекомендация:** для KV — `SETNX`-подобный условный запись/Lua-CAS по (token, generation); для файла — `flock` или полный переход на KV-бэкенд в мультиподной сборке; тест на гонку двух «подов» (в тестах стор уже эмулируется двумя инстансами — `pod1_store`/`pod2_store`, добавить сценарий одновременного commit).

### 4.2. publish_reviewed держит DB-локи во время CPU-тяжёлой компиляции [P2]

**Где:** `crates/modules/rustok-pages/src/services/page/reviewed_publish.rs:68-95` — `find_page_for_update` (эксклюзивный лок строки страницы), затем внутри транзакции: parse JSON → sanitize → materialize → hash по каждой локали.

**Проблема:** по бенчмаркам backlog'а validate ~300 мс на 10k узлов; на большой мультиязычной странице транзакция с эксклюзивным локом живёт секунды. Конкуррентные редакторы/публикаторы одной страницы сериализуются; растёт риск lock-таймаутов и очередей на «горячих» страницах.

**Рекомендация:** компилировать из reviewed-снапшота под shared-локом вне эксклюзивной транзакции, затем короткая эксклюзивная фаза: перепроверить `expected_body_revisions`/version и выполнить bind/switch. Альтернатива — pre-compile на save и публикация уже staging-артефакта.

### 4.3. `RevisionState::mark_changed` сбрасывает `save_failed` [P3]

**Где:** `crates/ui/fly/src/command/model.rs:331-338`.

**Проблема:** после неудачного сохранения первая же новая правка стирает индикатор отказа (`save_failed = false`), хотя изменения всё ещё не на сервере. `dirty` остаётся — но UI-сигнал «последний сейв упал» теряется.

---

## 5. Производительность (P2)

### 5.1. Каждый intent отвечает полным JSON проекта + page-fetch на каждый intent [P2]

**Где:** `crates/modules/rustok-page-builder/admin/src/browser_intent.rs:502-516` (`result()` — `GrapesJsCodec::encode_value` всего документа в каждом ответе, включая `select`/`hover`); `crates/modules/rustok-pages/admin/src/browser_intent.rs:73-77` (`transport::fetch_page` — HTTP round-trip на каждый intent, включая не-мутирующие).

**Проблема:** на странице в 1 МБ каждый клик в редакторе = fetch страницы + полная сериализация проекта + полный project_data в ответе (сеть). Автосейв каждой правки (dirty → save) добавляет ещё и полный save round-trip. Для интерактивного редактирования это главный масштабный барьер.

**Рекомендация:** для не-мутирующих интентов — lightweight-ответ (selection, hash, sequence) без project_data; мутирующие — дифф или клиентская реконструкция из effects; кэшировать fetch_page в пределах generation сессии.

### 5.2. Двойной clone+apply на slow-path `FlyEditor::apply` [P3]

**Где:** `crates/ui/fly/src/command/editor.rs:103-150` — speculative clone+apply в fast-path выполняется, а при `validate_patch_incremental → None` выбрасывается и повторяется заново на полном пути. Корректно (клоны независимы), но это лишняя полная копия+применение команды на каждый не-Patch fallback.

### 5.3. `InMemorySsrDraftSessionStore::load` берёт write-lock ради TTL-refresh [P3]

**Где:** `draft_session.rs:293-296`; `DurableSsrDraftSessionStore::commit_with_context` (`:996-1001`) использует `memory.load` как staleness-пробу — с побочным эффектом продления TTL. Мелочь, но на горячем пути чтения — лишняя эксклюзивность.

---

## 6. Целостность контрактов и поведение (P2)

### 6.1. «Настраиваемая» publish-политика не существует в рантайме [P2]

**Где:** README модуля: «operators can explicitly ban it through `forbidden_attributes`»; код: `static_publish_policy.rs:281-285` — `validate_static_publish_document` всегда `PageBuilderStaticPublishPolicy::default()`; `verify_integrity` evidence (`:217-231`) прибит к хэшу дефолта. Не-дефолтные конструкции политики встречаются только в тестах (`:1705`, `:1735`).

**Проблема:** поля `forbidden_attributes`/`forbidden_css_tokens` сериализуемы и задокументированы как операторские ручки, но ни один путь не загружает настроенную политику — и evidence-верификация её отвергла бы. Документация обещает несуществующее.

**Рекомендация:** либо реализовать загрузку политики из tenant settings (с включением хэша политики в evidence, как уже спроектировано), либо убрать заявления про операторскую настройку из README/комментариев.

### 6.2. Preview допускает то, что publish отвергает [P2]

**Где:** `crates/modules/rustok-pages/admin/src/builder.rs:569-573` — `PagesPageBuilderRenderer::render_preview` использует `RenderPolicy::default()` (`allow_http: true`), тогда как статическая публикация отвергает `http://` (`static_landing.rs:186-189`). Аналогично `load_project` (`:484-505`): ошибка инъекции шаблона глотается `if let Ok(...)`, превью молча деградирует к нетемплированному проекту.

**Проблема:** автор видит HTTP-картинку в превью и поздний сбой публикации — ровно тот «late, confusing publish failure», против которого написаны комментарии в самой политике (про `url(` и CSS-когерентность). Молчаливая деградация шаблона в превью скрывает поломку композиции.

**Рекомендация:** единая политика превью/публикации (`allow_http: false`) + диагностика вместо молчаливого fallback'а шаблона.

### 6.3. Выполняется только первый Request-effect [P2]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:174` — `requests.into_iter().next()`; остальные запросы молча отбрасываются. Сегодня контракт «≤1 запроса на диспатч» соблюдается эмпирически, но это не asserted — добавление интента, порождающего 2+ Request-effect, даст тихую потерю второго.

**Рекомендация:** либо явно `debug_assert`/error на >1 запроса, либо исполнить все (последовательно, с обработкой ошибок).

### 6.4. Ошибка сохранения оставляет клиент расходившимся с сервером [P2]

**Где:** `crates/modules/rustok-pages/admin/src/browser_intent.rs:174-188` — при ошибке фасада весь обработчик возвращает Err: draft не коммитится, `reload` не возвращается. Мутирующий интент уже применён в браузерном канвасе (оптимистично), серверная сессия откатилась к прошлому состоянию — клиент и сервер расходятся до перезагрузки страницы пользователем.

**Рекомендация:** на ошибку save возвращать структурированный ответ (не HTTP-ошибку) с `reload: true`/`revert_to` + revision, чтобы рантайм канваса восстановил серверное состояние.

### 6.5. Trait-метод `commit()` стирает runtime_context в File/Durable сторах [P2, латентно]

**Где:** `draft_session.rs:515-519` (File) и `:975-982` (Durable) — `commit` делегирует `commit_with_context(..., Value::Object(Map::new()))`, что **перезаписывает** сохранённый контекст пустым объектом; in-memory `commit` (`:112-118`) контекст **сохраняет**. Продакшн-вызовы используют `commit_with_context`, но trait-контракт модульно-зависим — мина для следующего хоста.

**Рекомендация:** в `commit` передавать `None`-семантику (сохранить прежний контекст) во всех бэкендах; зафиксировать тестом на всех трёх сторах.

### 6.6. `DurableSsrDraftSessionStore::prune` возвращает `max` вместо суммы [P3]

**Где:** `draft_session.rs:1053-1060` — `Ok(mem.max(persistent))` — статистика занижает число удалённых записей.

### 6.7. `FileSsrDraftSessionStore::load` глотает сбой продления TTL [P3]

**Где:** `draft_session.rs:505` — `let _ = self.save_snapshot(&snapshot)`: сессия не продлилась, но клиент об этом не узнает и через TTL молча потеряет черновик.

---

## 7. Next.js-сторфронт: детали (P2)

### 7.1. `scopeLandingCss` — рукописный CSS-токенайзер и `@scope`-зависимость [P2]

**Где:** `apps/next-frontend/packages/rustok-pages/src/components/css-scope.ts`.

**Проблема:**
- `@scope` поддерживается только современными браузерами (Chrome 118+/Safari 17.4+/Firefox 128+); на старых — CSS лендинга выйдет за контейнер и поломает shell хоста (деградация, не безопасность);
- hand-rolled парсер: `@keyframes` не входит в `isCssGroupRule`, но выживает за счёт балансировки стека; экранирование кавычек `trimmed[i-1] !== "\\"` некорректно для `\\\\"`; `passthrough` при уже-`@scope` префиксе — без проверки корректности;
- `contain: style layout` не содержит CSS-стили (`style` — только counters/quotes) — комментарий в коде обещает больше, чем делает.

**Рекомендация:** fallback для старых браузеров (postcss-префиксация селекторов) или явный QGIS-порог поддержки; заменить ручной парсер на постпроцесс селекторов через проверенный парсер (у Next-стека есть готовые), либо минимизировать грамматику (сегодня Rust-политика уже ограничивает CSS — можно транслировать селекторы механически).

### 7.2. `parseArtifactHtml` — regex-парсинг HTML [P3]

**Где:** `apps/next-frontend/packages/rustok-pages/src/api/artifact.ts:20-35` — regex для `<style>`/`<body>`. Работает на артефактах, которые генерирует Rust-рендерер (предсказуемая форма), но хрупок к любому изменению огибающей артефакта. Лучше передавать body/css отдельными полями из GraphQL (они уже есть в `StaticLandingPage`), а не вырезать regex'ом.

---

## 8. Замечания к структуре (P2/P3)

1. **FFA-расхождение:** 34 файла в `admin/src/editor/` смешивают Leptos `view!` с доменной логикой на верхнем уровне адаптера, тогда как канонический layout — `core/` (нейтрально) + `ui/<framework>`; нейтральное ядро есть (`model.rs`, `core/session.rs`, `draft_session.rs`), но бо́льшая часть поверхности — фреймворк-связанная. Dioxus запаркован (задокументировано) — ок, но Leptos-слой стоит продолжать мигрировать в workspaces (`fly-leptos FlyEditorWorkspace`), как планировал backlog.
2. **Семантика «Publish = save»:** capability `Publish` в сервисном слое фактически выполняет **сохранение черновика** (`PagesPageBuilderProjectStore::save_project` → `save_page_document`), а реальная публикация — отдельный `publish_reviewed` через Pages GraphQL. Именование сбивает с толку и уже привело к багу §2.2. Рекомендуется переименовать capability в `Save`/`Persist` на следующем major-этапе (zero-legacy позволяет).
3. **`PortContext.tenant_id` несёт slug:** `ensure_port_context` сравнивает `context.tenant_id.as_str() != tenant_slug` — поле называется `tenant_id`, содержит slug. Работает, но противоречит канонической типизации тенанта.
4. **`next-admin/packages/pages`:** hardcoded Tailwind-палитра (`bg-emerald-100` и т.п.) вместо общих design-token классов — мелочь против токен-контракта.
5. **`PagesBuilderFacade::with_provider_flags`** помечен `#[expect(dead_code)]` с честным комментарием — ок по §14, но «retires the attribute»-семантика сработает только при появлении вызова; следить.

---

## 9. Что сделано хорошо (сохранить как эталон)

- **Санитизация/публикация:** fail-closed политика с полным покрытием (теги, атрибуты, URL-виды, CSS-токены, media-запросы, srcset/sizes грамматика), канонические хэши, `verify_integrity` на каждом уровне, повторная проверка **после** runtime-материализации (`static_landing.rs:130-140`), ресурсные лимиты до рекурсивного прохода.
- **Экранирование:** `push_escaped_html`/`push_escaped_attribute` без исключений; `escape_style_element_text` (raw-text breakout через `\00003c`); JSON-LD `</script>`-эскейпинг; `escape_css_attribute` allow-list; `strip_validated_css_urls` с корректным порядком (data-URL с `;` до структурной проверки) — редкая аккуратность.
- **Транзакция публикации:** idempotency receipts с exact-replay, CAS по body-ревизиям и версии страницы, `FOR UPDATE`, транзакционный outbox, immutable staging артефактов, квоты размеров.
- **Undo/history:** Arc-sharing before/after (память ~½), инвертируемые команды, байтовый бюджет истории, `VecDeque`-эвикция, iterative depth-check в кодеке (стек-оверфлоу исключён), сохранение GrapesJS frames без второго дерева-авторитета.
- **CSRF:** деривация SHA-256 от session-id, constant-time compare, fail-closed на пустом секрете (`verify_csrf_token` отвергает `""`).
- **Canvas iframe:** `sandbox` без `allow-same-origin`, подписка прибита к expected origin, protocol+instance+sequence валидация сообщений.
- **Каталог символов:** CAS через revision-токен на save + `assert_reviewed_symbol_snapshot` на publish —lost-update между редакторами пойман.
- **Scheduler-claim:** оптимистичный `scheduled→executing` с фильтром по состоянию — конкурентные воркеры безопасны (не хватает только рипера, §3.3).

---

## 10. Приоритетный план исправлений

| # | Что | Приоритет | Оценка |
|---|-----|-----------|--------|
| 1 | RBAC-маппинг вместо строк ролей (§2.1) + выдать `pages:update` (§2.4) | P1 | средняя |
| 2 | Менеджер/автосейв: развести save-draft и publish (§2.2), preflight для синтезированного save (§2.3) | P1 | малая |
| 3 | Единый action-runtime для обоих storefront'ов (§3.2) | P1 | малая-средняя |
| 4 | Рендер `srcset` (§3.1) + e2e-тест | P1 | малая |
| 5 | Рипер `executing`-задач публикации (§3.3) | P1 | малая |
| 6 | CAS для durable draft-стора (§4.1) + binding user/tenant (§2.5) + лимиты сессий | P1 | средняя |
| 7 | Lightweight-ответы интентов + кэш fetch_page (§5.1) | P2 | средняя |
| 8 | Компиляция вне эксклюзивного лока (§4.2) | P2 | средняя |
| 9 | Согласовать preview/publish политику (§6.2) и убрать/реализовать «настраиваемую» политику (§6.1) | P2 | малая |
| 10 | Обработка ошибки save без расхождения клиент/сервер (§6.4) | P2 | малая |
| 11 | Файловый стор: права, атомарность, per-tenant (§2.6) | P2 | малая |
| 12 | Parity-тест рантаймов, srcset, stranded-job, draft-race — добавить в scoped-тесты соответствующих крейтов | — | — |

**Верификация:** аудит статический; код не изменялся, `cargo check/test` и Playwright-наборы не запускались. Все ссылки на файлы:строки сверены с HEAD `dd9b1d4`. Перед фиксами из таблицы — соответствующие scoped-проверки: `cargo test -p rustok-page-builder -p rustok-page-builder-admin -p rustok-pages`, `npm run verify:fly-interaction-capabilities`, parity-скрипты из `.github/workflows/pages-page-builder-parity.yml`.

## 11. Журнал реализации фиксов

### Фикс 1 (2026-11-10): RBAC-маппинг вместо строк ролей (§2.1) + выдача `pages:update` (§2.4) — РЕАЛИЗОВАН

Root cause: UI-тип `rustok_ui_auth::AuthUser` не нёс permissions, и все потребители (pages access/builder, intent-эндпоинт, CSR-фолбэки) угадывали капабилити по role-строке; порт Page Builder получал `[pages:read]` без `pages:update`, поэтому Properties-капабилита (save) всегда падала в Forbidden у не-админов, а менеджеры не видели вкладки авторинга.

Реализация (21 файл, единая сквозная семантика «эффективные permissions»):

- **Контракт UI**: `rustok_ui_auth::AuthUser` + `permissions: Vec<String>` (`#[serde(default)]` — старые пейлоады десериализуются в пустой снапшот = fail-closed). Туда же контракт-тест обратной совместимости.
- **Транспорты leptos-auth**: `permissions` добавлены в GraphQL `CurrentUserQuery`/`AuthUserGraphql`, REST `RestUserInfo`/`RestUserResponse`, cookie `CookieUserInfo` + все маппинги в `AuthUser`. Покрыты все пути получения юзера: GraphQL `me`, REST `/api/auth/me`, REST login/register/refresh, cookie session login/register/refresh, SSR `exchange_refresh_token_ssr`.
- **Сервер**: `UserInfo`/`UserResponse` (rustok-auth rest.rs) + `permissions`; резолв внутри транзакции выпуска сессии (`AuthLifecycleService::resolve_effective_permission_strings` через `get_user_permissions_authoritative`, как и `resolve_effective_role`) → поле в `AuthTokens`; GraphQL-провайдер переиспользует `tokens.permissions` (удалены дублирующий резолв и post-issuance compensation — они стали структурно не нужны); `me`/`update_profile` отдают снапшот из `CurrentUser` (авторитетный + отфильтрованный по доступности модулей).
- **pages-admin**: `access.rs` — `pages_editor_permissions_from_effective(&[String])` (manage→всё; update→авторинг; publish→только publish; иначе read-only) + `effective_permissions_grant` (зеркало `has_effective_permission`: точное совпадение или resource-manage); builder.rs — порт получает `[read] + update? + publish?` из единого маппинга (§2.4 закрыт: Properties теперь авторизован `pages:update`); composition.rs / inline_edit_launch.rs / lib.rs / contract_tests.rs / CRATE_API.md переведены на новый API; ролевые функции удалены (zero-legacy).
- **apps/admin**: intent-эндпоинт (main.rs) — политика из `verified_user.permissions`; CSR-фолбэк contribution host — вычисляет granted-подмножество как SSR server fn (убран role-гейт `super_admin|admin`).

Верификация: песочница без Rust-тулчейна, а `static.rust-lang.org`/`crates.io` вне allowlist хостов — компиляция здесь невозможна. Выполнено строгое статическое ревью: полный проход по `git diff` всех 21 хунков; grep-полнота всех конструкторов `AuthUser`/`AuthTokens`/`UserInfo`/`UserResponse` (включая тесты); подтверждение, что `create_user_in_tx` создаёт role-биндинги в той же транзакции (резолвер видит их); удалённые функции нигде не упоминаются. **`cargo check/test` НЕ запускались** — мейнтейнеру/CI: `cargo check -p rustok-ui-auth -p leptos-auth -p rustok-pages-admin -p rustok-admin -p rustok-server -p rustok-auth && cargo test -p rustok-ui-auth -p rustok-pages-admin`.

Follow-up (вне скоупа фикса, новое наблюдение): сайдбар админки (`apps/admin/src/widgets/app_shell/sidebar.rs:81`) гейтит группу «Management» по role-строке — нужен per-section permission map (users:read / modules:read / …), отдельный пункт плана.

Следующий по плану: фикс 2 — развести save-draft и publish (§2.2) + preflight синтезированного save (§2.3).

### Фикс 2 (2026-11-10): разводим save-draft и publish (§2.2) + preflight синтезированного save (§2.3) — РЕАЛИЗОВАН

Root cause: «save» в редакторе сквозь моделировался как publish (4 слоя: машина состояний `RequestSave` → require `publish`; кнопка Save в тулбаре; `BrowserIntentKind::Save`/`EditorShortcut::Save` → `EditorCapability::Publish`; порт `BuilderCapabilityKind::Publish` → `pages:publish`), при этом бэкенд-обработчик — `PageService::save_document` — на самом деле сохраняет working copy и сам требует `pages:update` (для опубликованной страницы публичный контент не меняется до отдельного publish_reviewed). И синтезированный автосейв диспатчился мимо capability-проверок.

Реализация (10 файлов, семантика «save = авторинг = pages:update; publish — отдельный флоу»):

- **fly_ui (machine.rs)**: `UiIntent::RequestSave` требует `edit` вместо `publish` (+ сообщение диагностики «save blocked…»). Тест переписан: автор (edit без publish) сохраняет; publish-only ревьюер — не мутирует и не сохраняет (publish-капабилитета остаётся независимой для настоящего publish-флоу — это прямо документировано в `CapabilityState::normalized`).
- **rustok-page-builder-admin (capability_access.rs)**: `BrowserIntentKind::Save` и `EditorShortcut::Save` → `vec![EditorCapability::Edit]`. Тесты переписаны: менеджер-профиль проходит save-preflight; restricted-профиль получает типизированный denial `edit`.
- **toolbar.rs**: кнопка Save гейтится по `capabilities.edit`, а не `publish`.
- **rustok-pages/admin (browser_intent.rs)**: `capabilities: CapabilityState` протаскивается через оба диспатча; SSR-контроллер сужается per-request через `UiIntent::SetEditableCapabilities` (контроллер создаётся с Full — теперь каждый конверт, идущий через его машину состояний, гейтится таблицей капабилитетов вызывающего); синтезированный автосейв валидируется `validate_browser_capability_access` и **пропускается** (не ошибка) при отказе — канвас остаётся dirty, клиент показывает индикатор «не сохранено».
- **contribution_browser_intent.rs**: обе with_capabilities-обёртки передают capabilities дальше; тест preflight переписан под новую семантику. browser_problem.rs: тест problem-контракта отражает реальную форму denial (`edit`).
- **builder.rs (фасад Pages)**: композиция порта переведена на `compose_configured_fly_page_builder_handlers` с кастомным авторизером: `publish: pages:update` (зеркалит собственный enforcement `save_document`), `properties: pages:update`, `preview/tree: pages:read`. Шаренный дефолт (publish→pages:publish) не тронут — хосты, чей сторе реально публикует, сохраняют его. Реальная публикация (`publish_reviewed`) по-прежнему требует `pages:publish` в модуле Pages.
- **TS parity**: моки denial в `apps/next-admin/tests/e2e/fly-browser-capability-problem.spec.ts` и `fly-browser-response-order.spec.ts` обновлены на `requires editor capability \`edit\`` / `capability: 'edit'`.

Сквозная цепочка после фикса (менеджер с `pages:update`): preflight пропускает мутирующий интент (edit) → автосейв валиден (edit) → SSR-машина пропускает (edit) → порт авторизован (pages:update) → бэкенд `save_document` (pages:update + owned scope). §2.2 закрыт полностью; §2.3 — синтезированный конверт проходит тот же capability-гейт, серверный контроллер сужен до запрошенных прав.

Верификация: как и в фиксе 1 — компиляция в песочнице невозможна (нет тулчейна, crates.io заблокирован). Статическое ревью: полный проход по `git diff` (10 файлов), проверка согласованности capability-таблиц (машина и preflight используют один `CommandCapabilityRequirement`), `validate_navigation_intent` не rejecting SetEditableCapabilities, idempotentность сужения между запросами draft-сессии, отсутствие остаточных упоминаний старой семантики (grep по `requires editor capability \`publish\`` и save/publish-линкам). **`cargo test -p fly-ui -p rustok-page-builder-admin -p rustok-pages-admin` и Playwright-наборы (`fly-browser-capability-problem`, `fly-browser-response-order`) НЕ запускались** — оставить CI. Путь синтеза автосейва (транспорт-зависимый) не покрыт unit-тестом — кандидат на integration-тест в CI.

Следующий по плану: фикс 3 — единый action-runtime (§3.2).

### Фикс 3 (2026-11-10): единый канонический action-runtime для обоих сторфронтов (§3.2) — РЕАЛИЗОВАН

Root cause: Next-пакет `@rustok/pages-frontend` не потреблял канонический рантайм действий, а содержал TS-переписывание (`fly-actions-runtime.ts`, 194 строки), которое поддерживало только 5 из 8 видов действий: отсутствовали `switch_tab`, `toggle_accordion`, `switch_slide` (+ touch/swipe-жесты, `panel_id`-fallback в `findTarget`, синхронизация aria-selected/is-active, exclusive-режим аккордеона, `globalThis.FlyActions`). Компоненты «табы/аккордеон/слайдер» на Rust-сторфронте работали, на Next.js — были мёртвыми; два рантайма продолжали расходиться.

Реализация (10 файлов: семантика «один канонический артефакт — `crates/ui/fly/browser/assets/fly-actions.js`; Next потребляет его байт-в-байт»):

- **`components/fly-actions.js`** (новый): byte-exact копия канонического ассета (sha256 `460bcd6e…`, `cmp` = 0). Ассет — самостартующий idempotent IIFE с `typeof document`/`typeof globalThis`-guard'ами → безопасен как side-effect import в SSR-контексте клиентского компонента; `ADAPTER_KEY = Symbol.for("fly.actions.adapter")` гарантирует один listener-set на документ.
- **`components/fly-actions.d.ts`** (новый): типы для side-effect модуля (`tsc` с `allowJs: false` + `moduleResolution: "bundler"` резолвит `./fly-actions.js` → `.d.ts`; сам `.js` в `include` не попадает и не компилируется) + типизация глобального API `FlyActions { start, handleAction, parseAction }` как `| undefined` (forced feature-detect).
- **`components/fly-action-bridge.tsx`**: удалён `initFlyActions`/импорт TS-рантайма; вместо него `import "./fly-actions.js"` в module scope. Остальная логика bridge (cart/navigate/islands/link-intercept) не менялась. В `packages/rustok-pages/package.json` нет `"sideEffects": false` → webpack не выкинет side-effect import.
- **`components/fly-actions-runtime.ts`**: УДАЛЁН (−194 строки; единственный импортёр — bridge). Grep: упоминания `initFlyActions`/`fly-actions-runtime` в коде отсутствуют, обновлены README.md и CRATE_API.md пакета.
- **`apps/next-frontend/scripts/verify-fly-actions-parity.mjs`** (новый): (1) byte-exact сравнение копии с crates-оригиналом (Buffer.equals + sha256 в сообщении об ошибке); (2) извлечение множества `case "kind":` из ассета и сверка с `ComponentAction::kind()` из `crates/ui/fly/src/action/model.rs` минус нативные `navigate_page`/`navigate_url`/`submit_form` (материализуются как href/form, до рантайма не доходят) — новый Rust-kind без runtime-поддержки валит проверку и наоборот; (3) bridge обязан side-effect-импортировать ассет и не ссылаться на удалённый рантайм; (4) `fly-actions-runtime.ts` не должен существовать (защита от resurrection); (5) `.d.ts` существует.
- **`package.json`**: скрипт `verify:fly-actions-parity` + включён в цепочку `test` (после `verify:seo-runtime-fixtures`).
- **`eslint.config.mjs`**: vendored-ассет в ignores — линт/autofix не должен менять байты (парity — byte-for-byte).
- **`.github/workflows/fly-page-builder.yml`**: шаг «Verify fly-actions asset parity with Next.js storefront» (`node apps/next-frontend/scripts/verify-fly-actions-parity.mjs`, скрипт путерезолится через `import.meta.url` — cwd-независим) + paths-триггеры на копию ассета и verifier в push/pull_request.
- **`pages-frontend.test.mjs`**: тест «fly-actions runtime is the canonical asset and handles every non-native action kind» — byte-parity + pinned-множество из 8 kinds на уровне `test:pages`.

Верификация (выполнено в песочнице, node доступен): `node scripts/verify-fly-actions-parity.mjs` → OK (8 runtime kinds = 11 ComponentAction − 3 native); `node --experimental-strip-types --test packages/rustok-pages/src/pages-frontend.test.mjs` → **9/9 pass** (было 8). Мутационное тестирование verifier'а: (а) однобуквенная правка копии → падение с sha256-диффом и инструкцией re-copy; (б) фейковый `Self::MysteryKind` в Rust enum → падение «missing case "mystery_kind"»; обе мутации откачены, `git status` канона/model.rs чист. Статическое ревью compile-рисков: tsconfig-резолв `.js`→`.d.ts`, отсутствие `sideEffects:false` в пакете, SSR-guard'ы ассета, BOM workflow сохранён. **`tsc --noEmit`, `next build`, `eslint`, Playwright НЕ запускались** (node_modules не устанавливается в песочнице) — CI: `npm --prefix apps/next-frontend run verify:fly-actions-parity && npm --prefix apps/next-frontend run test:pages && npm --prefix apps/next-frontend test && npm --prefix apps/next-frontend run typecheck`; Rust-сторона не менялась (`cargo test -p fly-browser` — существующий контракт `action_runtime_contract.rs`).

Замечание по дизайну: физически ассет дублируется (vendored copy), но verifier делает дрейф структурно невозможным для мержа — это ровно рекомендация §3.2 («копия в сборке с хэш-сверкой против Rust-версии»), альтернативы (public/ + next/script, webpack ?raw) отклонены ранее как ломающие пакетную инкапсуляцию/требующие next.config.

Следующий по плану: фикс 4 — рипер/lease для `executing`-задач публикации (§3.3; `schedule.rs:229-260` — claim scheduled→executing без таймаута, выборка рипера только `state='scheduled'`). srcset §3.1 (пункт 4 таблицы §10) остаётся в очереди.

### Фикс 4 (2026-11-10): lease + рипер для `executing`-задач публикации (§3.3) — РЕАЛИЗОВАН

Root cause: claim в `process_due_publish_jobs_as_of` — оптимистичный `scheduled → executing` без lease/таймаута, а выборка задач (и, значит, любая повторная обработка) фильтрует только `state='scheduled'`. Крэш пода между claim'ом и терминальным апдейтом оставлял job в `executing` навсегда: публикация не происходила и не повторялась — нарушение State Transition Integrity («no entity can become permanently stranded in processing upon mid-chain failure»).

Реализация (9 файлов: семантика «claim держит lease; свип сначала жнёт просроченные leases»):

- **Миграция** `m20261110_000001_add_page_publish_job_lease_until`: nullable `lease_until TIMESTAMPTZ` + partial index `idx_page_publish_jobs_lease ON page_publish_jobs (lease_until) WHERE state='executing'` (Postgres + SQLite, одинаковый синтаксис). Backfill-контракт `pages-page-publish-job-lease` (mode `none`) добавлен в `docs/migrations/backfill-contracts.json` — обязательный по CI-верификатору `verify-migration-backfill-contracts.mjs` (каждая appended-миграция требует контракт).
- **Entity**: `lease_until: Option<DateTimeWithTimeZone>` (NULL на `executing`-строке = claim до появления колонки; рипер трактует как просроченный).
- **schedule.rs**:
  - `PAGE_PUBLISH_JOB_LEASE_SECS = 600` — lease 10 минут ≫ худшего времени `publish_reviewed`; replay безопасен (захваченная команда несёт `idempotency_key`).
  - Claim (`scheduled → executing`) теперь пишет `lease_until = now + lease`; все терминальные/повторные переходы (published, failed, retry→scheduled) и requeue чистят lease.
  - `reap_expired_publish_jobs_as_of(now)`: просроченные `executing` (lease ≤ now ИЛИ NULL) → `scheduled` при `attempts < MAX`; терминальный `failed` при `attempts ≥ MAX`; терминальный `canceled`, если для страницы уже есть более новый `scheduled`-job — пока job «висел», автор мог создать новый schedule (pending-строки нет → `schedule_publish` вставляет новую), и requeue старого нарушил бы partial unique index `uq_page_publish_jobs_active`; новый schedule выигрывает. Каждый переход — оптимистичный (UPDATE re-check'ает state и просроченность lease; `rows_affected == 0` → пропустить), с маркером `PAGE_PUBLISH_JOB_LEASE_EXPIRED` + сообщением о восстановлении.
  - `sweep_due_publish_jobs_as_of(now, limit) → PagePublishSweepOutcome { published, reaped }` — канонический свип «reap → claim → execute»; рипер встроен в свип, поэтому любой хост свипа самовосстанавливается без отдельного процесса. Прежние `process_due_publish_jobs` / `process_due_publish_jobs_as_of` — обёртки с прежними сигнатурами (u64 published), существующие вызовы не тронуты.
  - **Метрики** (по образцу outbox `RelayMetrics`): `PagePublishSchedulerMetrics` (AtomicU64) + `PagePublishScheduler::metrics() → PagePublishSchedulerMetricsSnapshot { sweeps_total, published_total, requeued_total, reaped_failed_total, superseded_total }` — `requeued_total` детектирует crash-recovery, `reaped_failed_total` — stranded-публикации, требующие внимания оператора.
- **Реэкспорты** (services/page/mod.rs, services/mod.rs): outcome-типы, снапшот метрик, константы lease. API-ответы (`PagePublishScheduleResponse`) не менялись.
- **Доки**: README модуля (обязанности + behavior notes) и `DECISIONS/2026-10-09-scheduled-page-publishing.md` (Failure semantics: lease-семантика и три исхода рипера; Migration: вторая append-only миграция; Verification: покрытие рипера).

Тесты (`crates/modules/rustok-pages/tests/page_publish_scheduler_sqlite.rs`, +7 к 8 существующим = 15): один свип самовосстанавливает stranded-задачу и публикует её (requeued=1 → published=1, attempts=2, lease сброшен, success очищает маркер); requeue с маркером `PAGE_PUBLISH_JOB_LEASE_EXPIRED`; живой lease не трогается; NULL lease трактуется как просроченный; attempts=MAX → терминальный `failed`, позже не воскрешается; superseded → `canceled`, новый schedule исполняется; метрики шедулера считают свипы/восстановления и не растут при повторном свипе без strand'ов. Крэш симулируется helper'ом `strand_job` (прямая постановка `executing` + lease + attempts).

Верификация: компиляция в песочнице невозможна (тулчейна нет, crates.io вне allowlist). Статическое ревью: полный проход по диффу; оба литерала `page_publish_job::ActiveModel` в `schedule_publish` дополнены `lease_until: Set(None)` (иначе literal не компилируется); `Expr::value(Option<DateTimeWithTimeZone>)` подтверждён по документации sea-query 1.0.2 (`From<DateTime<FixedOffset>> for Value` + generic `From<Option<T>> where T: Nullable`); `Condition::any().add(...)` сверен с локальной идиомой (не `.filter`); `ConnectionTrait` импортирован в миграцию (по образцу соседней); rustfmt-моделирование (max_width 100 / fn_call_width 60 / chain_width 60) сверено с эмпирикой соседнего кода, строк > 100 нет. Прогнаны доступные node-гейты: `node scripts/verify/verify-adrs.mjs` (178 решений, ок), `node scripts/verify/verify-migration-backfill-contracts.mjs --self-test` (ок), `node scripts/verify/verify-migration-compatibility-contract.mjs` (ок). **`cargo fmt/clippy/test` НЕ запускались** — CI: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p rustok-pages --test page_publish_scheduler_sqlite` (workspace-покрытие — через `cargo llvm-cov nextest --workspace --all-targets`).

Замечания по дизайну: (1) рипер встроен в свип, а не отдельным процессом — единая точка восстановления, pub-метод `reap_expired_publish_jobs_as_of` доступен для ops; (2) гонка «старый воркер дописывает после requeue+re-claim» закрыта идемпотентностью `publish_reviewed` (idempotency_key) + lease ≫ худшего publish; (3) `latest_publish_job`/`page_publish_schedule` не exposes lease — поверхность API не расширена намеренно.

Следующий по плану: фикс 5 — CAS для durable draft-стора (§4.1) + binding user/tenant (§2.5) + лимиты сессий. srcset §3.1 (пункт 4 таблицы §10) остаётся в очереди.

### Фикс 5 (2026-11-10): CAS для durable draft-стора (§4.1) + owner-binding и лимиты сессий (§2.5) — РЕАЛИЗОВАН

Root cause (две независимые дыры одного стора): (1) TOCTOU — файловый `commit_with_context` делал read-file → check → write-file без блокировок, KV `save_record` — безусловный `set`; два пода читали generation N, оба проходили `expected_generation`, оба писали N+1 → lost update, оптимистичная блокировка молча обходилась на мультиподе; (2) запись сессии не содержала владельца — токен был единственным «credential» (обмен токеном = перехват чужого черновика), а безусловный commit в конце каждого интента (включая read-only `select` от `customer`) + отсутствие лимитов размера записи давали ресурсную амплификацию до ~160 ГБ на одного низкопривилегированного пользователя.

Реализация (12 файлов; семантика «CAS по числу generation + владелец в записи + лимиты; legacy-записи без owner остаются читаемыми»):

- **`rustok-page-builder-admin/draft_session.rs`** (ядро):
  - `DraftSessionOwner { tenant_id, user_id }` (Serialize/Deserialize, PartialEq) + `owner: Option<...>` в `SsrDraftSessionSnapshot`/`PersistentDraftSessionRecord` (`serde(default)`, skip-if-none — wire-совместимо со старыми записями). Хелпер `ensure_session_owner(record, caller)`: attributed-запись принимает только точно такого же caller'а → `OwnerMismatch`; `None`-запись (legacy/внутренние пути) пропускает всех; первый attributed commit **усыновляет** владельца (adoption) — после этого анонимные caller'ы блокируются.
  - Трейт `SsrDraftSessionStore`: `load/commit/commit_with_context/remove` получают `owner: Option<&DraftSessionOwner>` последним параметром; `OwnerMismatch` — opaque (не светит user_id/tenant_id).
  - **KV CAS**: `DraftSessionKvDriver::set(key, value, cas, ttl)` + новый обязательный `compare_and_set(key, expected_cas, value, cas, ttl) -> bool` — CAS по **числу generation** (byte-стабильность сериализации AdminCanvasController не гарантирована из-за HashMap-порядка в FlyEditor/FlyUiStateMachine), `expected_cas: None` = SETNX-семантика «ключ отсутствует». `MemoryKvDriver` хранит `(bytes, cas, expires)`; реальный Redis-драйвер реализуется WATCH/MULTI или Lua без изменения контракта. `ExternalKvDraftSessionBackend::compare_and_save_record(expected: Option<&record>, new)` прокидывает `record.generation` как expected-cas.
  - **Файловый CAS**: `with_token_lock(dir, token, action)` — lock-файл через `OpenOptions::create_new` (O_EXCL; только std, без новых зависимостей — crates.io недоступен), stale-break при mtime > 30 s (крэшнувший холдер), wait-deadline 2 s (→ `LockTimeout`), poll 10 ms; имя файла безопасно — токен уже валидируется `is_safe_token` (≤64, `[A-Za-z0-9_-]`). Под локом — семантическое сравнение (token, generation, page_id, owner). Обновление существующей записи (`commit_existing_under_lock`) выполняется только под локом; входы «отскакивают» обратно в caller'а через `TokenLockCommit::Missing`, если запись исчезла/истекла (ownership-раунд-трип без клонирования контроллера). `remove` — тоже под локом (не может удалить свежезаписанную запись или воскресить удалённую); TTL-refresh в `load` — через `compare_and_save_record` (load больше не может регрессировать чужой коммит старой генерацией).
  - **Durable CAS-флоу**: load_record → fail-fast (page/owner/generation) → seed memory → `memory.commit` → `backend.compare_and_save_record(expected = загруженная запись, new)`; при проигрыше — reseed memory из авторитетной записи бэкенда + `GenerationConflict{expected: актуальная, actual: caller'а}`. Новая сессия: `expected = None` → set-if-absent.
  - **Лимиты (§2.5c)**: `MAX_DRAFT_SESSION_RECORD_BYTES = 20 MiB` (против 16 MiB max_project_bytes; проверка `ensure_record_size` через serde_json::to_vec на File/KV `save_record` и KV CAS) → `RecordTooLarge{bytes, maximum}`; `MAX_DRAFT_SESSIONS_PER_OWNER = 8` — eviction старейших same-owner сессий в InMemory (insert + commit), File `save_record` для новых записей (scan), KV `prune_records` (group-by-owner).
  - InMemory: `DraftEntry.owner`, per-owner cap с eviction по `expires_at`, owner-чек в `load`/`commit_internal`/`remove`, adoption в commit.
- **`rustok-pages/admin`**: `PagesBuilderSaveSnapshot` + `owner: Option<DraftSessionOwner>` (5 конструкторов: endpoint — `Some`; builder/composition/lib/metadata — `None`); `browser_intent.rs`: owner в `load`/`remove`/`commit_with_context`; **read-only гейт** — при `capabilities.edit == false` commit не вызывается вовсе (ответ эхом возвращает загруженный `draft_token`/`draft_generation` либо `""`/`0`; пустой токен фильтруется envelope-нормализацией при следующем load) — read-only `select` больше не создаёт сессию; `composition.rs`: SSR-restore строит owner из `current_user`+`tenant` (без аутентификации — `None`, legacy-поведение); `browser_problem.rs`: `OwnerMismatch` → HTTP 403 (opaque detail), `RecordTooLarge` → 413, `LockTimeout` → 503; реэкспорт `DraftSessionOwner` из lib.rs.
- **`apps/admin/main.rs`**: intent-endpoint строит `Some(DraftSessionOwner { tenant_id: tenant_slug, user_id: verified_user.id })` — только verified user, никаких client-supplied идентификаторов.
- **Доки**: CRATE_API.md обоих крейтов (owner-binding семантика, CAS-контракты, лимиты, новые коды ошибок).

Тесты (draft_session.rs: +13 к существующим; browser_problem.rs: +1): owner mismatch на load/commit/remove + cross-tenant; legacy `None`-запись читаема attributed-caller'ом + adoption запирает анонимов; durable owner-чек по бэкенд-записи при пустом memory-кэше (перезапуск пода); per-owner cap InMemory (10 коммитов → выживают последние 8) и File; `RecordTooLarge` на 20 MiB+ runtime context; атомарность `MemoryKvDriver::compare_and_set` (win/stale/SETNX); файловая гонка два-пода-один-каталог (один выигрывает gen 2, второй `GenerationConflict{2,1}`, lock-файлы убраны); KV-гонка pod1/pod2 (выигравший gen 2, проигравший конфликт + reseed памяти); **детерминированная интерливинг-гонка** `RacingKvDriver` (test-драйвер вклинивает чужой коммит прямо перед CAS) — CAS-проигрыш на пути, недостижимом последовательными вызовами, + reseed из winner-записи; fail-fast по перечитанному бэкенду после чужого коммита; прямые CAS-контракты обоих бэкендов (create/setnx-lose/update-win/stale-lose/owner-lose). Все 35 существующих вызовов стора (33 в тестах draft_session.rs, 1 в contract_tests.rs, 1 в тесте browser_intent.rs) обновлены на новую арность.

Верификация: компиляция в песочнице невозможна (тулчейна нет, crates.io вне allowlist — поэтому lock через std `OpenOptions::create_new`, а не fs2/flock). Статическое ревью: полный проход по диффу; баланс скобок/паренов проверен программно; ownership-анализ замыканий (`commit_existing_under_lock` возвращает входы через `TokenLockCommit::Missing` — use-after-move исключён); rustfmt-моделирование (max_width 100 / fn_call_width 60 / chain_width 60) — удлинившиеся после `+owner` цепочки разбиты вертикально; единственный внешние call-site'ы стора (pages/admin ×5, main.rs, contract_tests.rs) все обновлены, иных имплементоров трейтов в репо нет. **`cargo fmt/clippy/test -p rustok-page-builder-admin -p rustok-pages-admin` НЕ запускались** — CI: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test -p rustok-page-builder-admin -p rustok-pages-admin`.

Замечания по дизайну: (1) CAS-токен — число generation, а не хэш байтов: Redis-пути (WATCH/MULTI, Lua) не зависят от недетерминированной serde-сериализации контроллера; (2) legacy `None`-записи не инвалидируются массово — мягкая миграция через adoption при первом attributed commit; (3) read-only гейт на уровне dispatch, а не стора: внутренние пути с `CapabilityState::full()` сохраняют прежнее поведение, а `PagesBrowserIntentResponse` для read-only остаётся валидным (эхо токена); (4) lock-файл — advisory, защищает только кооперирующие процессы RusTok; stale-break 30 s > любых операций под локом; (5) per-owner cap File-стора сканирует каталог только при создании новой записи (update не меняет счётчик).

**Ревью-проход (2026-11-10, глубокий self-review фикса 5 до запуска CI):** перечитаны все диффы целиком; найдено и исправлено 5 дефектов:
1. *Критичный — слои*: `FileSsrDraftSessionStore::delete_record` делегировал в owner-проверяющий `remove(token, None)` → `Durable::remove` owned-сессии ВСЕГДА падал бы с `OwnerMismatch` даже для законного владельца. `delete_record` переписан как безусловный бэкенд-примитив под токен-локом (owner-проверка остаётся в слое durable-store, где ей место).
2. *Лимиты*: `ensure_record_size` мерил compact-сериализацию, а файловый бэкенд пишет pretty — файл мог превысить 20 MiB кап. Хелпер переведён на фактические байты: `ensure_record_size(json_bytes.len())` после соответствующей сериализации (pretty — File, compact — KV) в трёх точках записи.
3. *InMemory*: re-seed существующего токена через `insert_snapshot` (durable-синхронизация памяти) ошибочно эвиктил чужую same-owner сессию — теперь per-owner eviction только для genuinely новых токенов (`!entries.contains_key`).
4. *Учёт*: KV `prune_records` считал запись дважды (глобальный кап + per-owner кап могли удалить/посчитать один ключ) — удаления собираются в `BTreeSet`, применяется однократно, счёт точен; заодно File `prune` получил тот же per-owner кап, что KV (единая семантика бэкендов).
5. *Лок*: holder-identity в lock-файле (uuid записывается при acquire, сверка перед remove) — холдер больше не может удалить чужой свежий лок после stale-break; провал записи identity прибирает лок и возвращает Io вместо неатрибутированного захвата; в доке лока честно задокументирована остаточная гонка stale-break (µs-окно, требует крэша холдера + двух waiter'ов; устраняется flock/fcntl при появлении зависимости).
Верно подтверждены при ревью: NLL-паттерн `get`→`?`→`remove` (последнее использование ссылки до мутации), object-safety всех трёх трейтов (dyn-использование), transparent-Display цепочка для 403-теста, `path` srcset-диагностики (`{component}.attributes.srcset`), string-continuation в e2e-ассерте, `image → img` маппинг тега, отсутствие атрибутной пост-фильтрации в артефактном пайплайне (`body_html = rendered.html` напрямую).

Следующий по плану: srcset §3.1 (пункт 4 таблицы §10) — последний из очереди.

### Фикс 6 (2026-11-10): рендер `srcset` с per-URL-проверкой (§3.1) — РЕАЛИЗОВАН

Root cause: рендерер безусловно выбрасывал `srcset` из выхода вместе с `style`/`srcdoc`/`xlink:href`/`ping`/`background`, при этом `sizes` НЕ выбрасывался — в артефакт попадал осиротевший `sizes`. ~150 строк грамматической валидации (`validate_srcset`: дескрипторы `w`/`x`, запрет `data:`, уникальность, лимит кандидатов) были мертвы относительно выхода, а README модуля обещал «default policy permits validated `srcset`».

Реализация (4 файла; семантика «srcset рендерится целиком, если каждый URL кандидата проходит ту же политику, что `src`; один плохой кандидат роняет весь атрибут»):

- **`crates/ui/fly/src/render.rs`**: `srcset` убран из безусловного drop-списка; новый хелпер `srcset_allowed(value, policy)` — парсит структуру кандидатов (split по `,`, непустой URL + не более одного дескриптора, без лишних токенов), отвергает `data:` URL'ы явно (строже, чем `allow_data_images` для `src` — зеркалит publish-политику) и проверяет каждый URL через `url_allowed(url, UrlAttributeKind::Resource, policy)` — тот же ресурсный профиль, что у `src`/`poster`. All-or-nothing: частичный srcset молча менял бы, какое изображение выберет браузер. `sizes` остаётся как был — теперь с рендерящимся `srcset` он больше не осиротевший.
- **`crates/ui/fly/src/render.rs` (тесты)**: валидный srcset (`/img/hero.webp 800w, /img/hero@2x.webp 1600w`) + `sizes` доходят до HTML; drop-кейсы — `javascript:` кандидат, `data:` кандидат, лишний токен в кандидате, пустое значение.
- **`crates/modules/rustok-page-builder/src/static_landing.rs` (e2e)**: end-to-end «srcset в проекте → srcset в артефакте» — `compile_publish` с https-кандидатами: `body_html` содержит `srcset` и `sizes`, `verify_integrity` проходит; http-кандидат отклоняется ДО создания артефакта (`landing_srcset_rejected`, путь `attributes.srcset` — `require_secure_resource_urls` смотрит только `src`/`poster`, srcset-кандидаты доходят именно до srcset-валидации); `data:`-кандидат отклоняется аналогично. Валидация и рендер теперь когерентны: то, что пропускает политика, реально попадает в артефакт; то, что рендер может выбросить, политика не пропустит.
- **`README.md` модуля**: обещание «default policy permits validated `srcset`» теперь истинно end-to-end; добавлено описание рендер-поведения (per-candidate проверка, all-or-nothing drop).

Верификация: компиляция в песочнице невозможна. Статическое ревью: полный проход по диффу; баланс скобок проверен полноценным сканером (строки/комментарии/char-literals); строка-continuation в e2e-ассерте сверен с семантикой Rust `\`-newline (пробел перед `\` сохраняется, ведущие пробелы следующей строки съедаются); `UrlAttributeKind::Resource` в fly — семантический аналог `UrlKind::ResourceImage` publish-политики (relative || https [+ data-images, но srcset отвергает data: раньше); порядок проверок в рендере: Bool-атрибуты → scalar → srcset-ветка → generic URL-чек. **`cargo test -p fly -p rustok-page-builder` НЕ запускались** — CI.

Замечания по дизайну: (1) рендер не дублирует полную грамматику дескрипторов (уникальность/семейство w-vs-x) — это обязанность publish-гейта перед артефактом; рендер проверяет то, что важно для любого рендера (структура + URL-политика); (2) data: в srcset запрещён жёстче, чем data-images в `src` — намеренное зеркало publish-политики (`validate_srcset` отвергает `data:` до `validate_url`); (3) `sizes` при упавшем srcset остаётся в HTML превью-рендера — инертный атрибут без srcset, для артефактов исключено парой «политика пропускает только валидную пару».

Остались в очереди §10 (все P2): пункт 7 — Lightweight-ответы интентов + кэш fetch_page (§5.1); пункт 8 — компиляция вне эксклюзивного лока (§4.2); пункт 9 — согласование preview/publish политики (§6.2) + «настраиваемая» политика (§6.1); пункт 10 — обработка ошибки save без расхождения клиент/сервер (§6.4); пункт 11 — файловый стор: права, атомарность, per-tenant (§2.6). Пункт 12 (parity/stranded/draft-race/srcset тесты в scoped-наборах) закрыт фиксами 3–6.

### Фикс 7 (2026-11-10): Lightweight-ответы не-мутирующих интентов + кэш persisted-ревизии вместо fetch_page на каждый intent (§5.1) — РЕАЛИЗОВАН

Root cause: `result()` сериализовал весь документ (`GrapesJsCodec::encode_value`) в КАЖДОМ ответе интента — включая `select`/`hover`, где документ не меняется; pages-слой делал HTTP `fetch_page` на каждый intent, хотя страница нужна только для seed/reseed контроллера. На странице в 1 МБ каждый клик в редакторе = fetch + полная сериализация + мегабайт мёртвого JSON в ответе.

Предварительное исследование потребителей `result.project_data` (решало Option vs Null): TS-клиентов нет (grep apps/next-admin, apps/next-frontend пуст); `fly-browser.js` читает из ответа только `revision_id`, `project_hash`, `draft_token`, `draft_generation`, `reload` — project_data игнорирует; Dioxus/adapter-канвасы (`ui/dioxus.rs:194`, `editor/adapter_workspace.rs:51`) вызывают `dispatch_browser_intent` и результат НЕ читают (`Ok(_)`) — рендер напрямую из своего контроллера; pages-слой только пересериализует. Вывод: `Option<Value>` + `skip_serializing_if` безопасен, мутирующие пути оставлены с полным документом (консервативно, wire-совместимо).

Реализация (2 файла):

- **`crates/modules/rustok-page-builder/admin/src/browser_intent.rs`**: `BrowserIntentDispatchResult.project_data: Value` → `Option<Value>` (`#[serde(default, skip_serializing_if = "Option::is_none")]` — десериализация старых пейлоадов совместима); `dispatch_browser_intent` теперь передаёт в `result(controller, effects, is_mutating_intent(&envelope))` — тот же предикат, которым `validate_revision` решает, проверять ли revision/hash-конфликты (self-consistency: интенты без revision-чека не могут изменить документ, значит lightweight-ответ не может дезинформировать); не-мутирующие (select/hover/focus_requested/hover_requested и прочие вне `BrowserIntentKind::is_mutating` + вне расширенного списка) получают `project_data: None`, мутирующие — `Some(encode_value(document))` без изменений поведения.
- **`crates/modules/rustok-pages/admin/src/browser_intent.rs`**: (a) реструктуризация `dispatch_pages_browser_intent_with_store`: `draft_store.load` перенесён ДО fetch (было после), fetch стал ленивым — выполняется только когда нет сессии или сессия не подтверждена кэшем; (b) `PersistedRevisionCache` — статический `Mutex<HashMap<u64, (String, Instant)>>`, ключ = DefaultHasher(token, tenant_slug, page_id) (токен хэшируется — креды не дублируются в долгоживущих записях; разные токены не разделяют view ревизии), TTL 30с, лимит 512 записей (при переполнении после prune-expired карта очищается целиком — цена только лишние fetch, не ошибки); (c) fast-path: существующая сессия с `revision_id == cached` → fetch пропускается вовсе (страница нужна только для seed/reseed); (d) кэш обновляется после fetch (cache-miss/reseed) и после успешного publish — из `revision_id` ответа `PublishPageBuilderResult` (источник — только что сохранённая страница), без refetch; (e) `controller_snapshot` (context-update/post-publish пути) оставлен с `Some(...)` — консервативно, оба пути вызывают reload.
- Юнит-тесты кэша (4): roundtrip per key + победа последней записи; expiry по границе TTL (`now < expires_at`, просроченная запись удаляется); bounded (512 живых → 513-я сбрасывает карту, остаётся 1); раздельность ключей по token/tenant/page.

Корректность кэша (ключевой аргумент): save-путь авторитетен независимо от кэша — `save_project` (builder.rs:551+) сам делает `fetch_page` и возвращает `REVISION_CONFLICT: expected …, current …` при расхождении ДО записи. Stale-запись кэша лишь задерживает детектирование чужой правки в пределах TTL (30с): вместо тихого reseed-по-свежей-странице пользователь получит явный REVISION_CONFLICT на своём save. Аналогично удалённая страница: `PageNotFound` теперь может прийти на save-момент, а не на каждый клик. Сессия с MATCHING revision вообще не нуждается в странице — единственное назначение fetch было подтвердить это совпадение.

Верификация: компиляция в песочнице невозможна (без изменений). Статическое ревью: полный проход по диффу обоих файлов; баланс скобок state-machine сканером (строки/комментарии/char-literals/raw-strings) — OK; все 4 места конструирования `BrowserIntentDispatchResult` обновлены (page-builder result(), pages controller_snapshot, тест request_effects); grep-полнота: других `.project_data`-читателей результата интента нет (fly-browser.js, fly-actions.js, composition.rs, apps/admin, adapter/dioxus-канвасы); строки ≤100 символов; импорт `DefaultHasher` в стиле `std::collections::hash_map::DefaultHasher` — как в rustok-core/cache_atomic.rs. **`cargo test -p rustok-page-builder-admin -p rustok-pages-admin` НЕ запускались** — CI.

Замечания по дизайну: (1) мутирующие ответы по-прежнему несут полный документ — аудит предлагал «дифф или клиентскую реконструкцию из effects», но клиент всё равно делает full reload после мутации (`reload: is_mutating() || persistence`), поэтому дифф дал бы ноль выгоды при усложнении протокола; lightweight применён только к не-мутирующим, где выигрыш реален; (2) TTL=30с — окно позднего детектирования чужих правок; при необходимости тонкой настройки вынести в конфиг (сейчас const, как соседние MAX_*); (3) кэш per-process — многоинстансовые деплои просто не получают выигрыша, корректность не страдает (save-time check в фасаде).

Следующий по плану: фикс 8 — компиляция вне эксклюзивного лока (§4.2).

Остались в очереди §10 (все P2): пункт 8 — компиляция вне эксклюзивного лока (§4.2); пункт 9 — согласование preview/publish политики (§6.2) + «настраиваемая» политика (§6.1); пункт 10 — обработка ошибки save без расхождения клиент/сервер (§6.4); пункт 11 — файловый стор: права, атомарность, per-tenant (§2.6). Пункт 12 закрыт фиксами 3–6; пункт 7 закрыт этим фиксом.

### Среда проверки для фиксов 8–11 и последующих записей

Начиная с этой записи часть кода компилируется и запускается, а не только читается. Что это за проверка и чего она не покрывает:

- **Toolchain:** `rustc`/`cargo`/`clippy`/`rustfmt` 1.88.0 (Android-prebuilt, получен через git). Workspace требует `rust-version = "1.96"`; crates.io из песочницы недоступен. Зависимости `serde`, `serde_json`, `thiserror` и их деревья собраны из git-тегов вручную; `sha2`, `uuid`, `rustok-ui-i18n` заменены минимальными заглушками (SHA-256 сверен по тест-векторам FIPS).
- **Собирается и тестируется:** `fly`, `fly-ui`, `fly-browser`, `rustok-page-builder` (без feature `server`), а также модули `draft_session.rs`, `model.rs`, `asset_provider.rs` из `rustok-page-builder-admin`, подключённые через `#[path]` во вспомогательный крейт вне репозитория.
- **Не собирается:** `rustok-pages` (sea-orm), `rustok-pages-admin` и `rustok-page-builder-admin` целиком (leptos), `apps/admin`, feature `server`. Для них — только синтаксический разбор (`rustc -Z parse-crate-root-only`), `rustfmt --check` и чтение кода. Исключение: функции Pages browser-intent без зависимостей от фреймворка и их тесты дословно извлечены скриптом в тот же вспомогательный крейт и выполнены.
- **Не запускалось:** property-тесты `fly` (`proptest` недоступен, блоки вырезаны из копии); три теста, которым нужны CLDR-данные настоящего `rustok-ui-i18n`; все интеграционные тесты с Postgres/SQLite; Playwright.
- **Итог последнего полного прогона на финальном дереве:** `fly` 301, `fly-ui` 80, `fly-browser` 16 + 21 контрактный, `rustok-page-builder` 126 + 3, вспомогательный крейт 58 (42 `draft_session`, 4 `model`, 3 `asset_provider`, 9 извлечённых из Pages) — все зелёные. `clippy` 1.88 выдаёт предупреждения только на строках, не затронутых этими фиксами. `rustfmt --check` по затронутым файлам: новых расхождений нет (в `draft_session.rs`, `browser_intent.rs`, `builder.rs` расхождения были и на HEAD).

**Вывод для ревьюера:** зелёный прогон здесь не заменяет CI. Обязательны `cargo check --workspace`, `cargo clippy --workspace -- -D warnings` и тесты `rustok-pages`, `rustok-pages-admin`, `rustok-page-builder-admin` на штатном toolchain.

### Дефекты сборки на `main`, найденные при первой компиляции — ИСПРАВЛЕНЫ

Фиксы 1–7 проверялись только чтением. Первая реальная сборка показала четыре ошибки, которые не дали бы пройти CI:

1. `rustok-page-builder/admin/src/lib.rs` не разбирался: содержимое `model.rs` было вставлено в `mod model { … ; }` с лишней `;`. Блок заменён на `mod model;`. Перед удалением вставка сверена с файлом `src/model.rs` построчно (без учёта отступов) — совпадает полностью, код не потерян.
2. `draft_session.rs`, `DurableSsrDraftSessionStore`: использование значения после перемещения (E0382).
3. Тест `compiler_renders_validated_srcset_into_artifact` (фикс 6) падал: у изображения в фикстуре не было `alt`, который требует политика публикации. Исправлена фикстура.
4. Тест `file_store_caps_sessions_per_owner` (фикс 5) падал; проходит после переработки стора в фиксе 11.

Пункты 1–2 — ошибки компиляции, 3–4 — ошибки тестов. Все четыре относятся к коду, который в журнале выше помечен «РЕАЛИЗОВАН». Теперь они собираются и проходят в среде, описанной выше.

### Фикс 8 (2026-11-10): компиляция лендинга вне эксклюзивной блокировки (§4.2) — РЕАЛИЗОВАН, НЕ ЗАПУСКАЛСЯ

Root cause: `publish_reviewed` открывал транзакцию, брал `FOR UPDATE` на страницу и уже под блокировкой раскрывал символы, собирал layout, разбирал проект и компилировал статический лендинг. Всё это время другие операции над страницей ждали.

> Эта запись описывает первую версию фикса. Она нарушала контракт порядка операций публикации и была переработана — см. пункт 5 записи «Проход по AGENTS.md» ниже. Имена `ReviewedPublishInputs` и `evaluate_reviewed_publish`, а также замечания 1 и 3 в конце этой записи к текущему коду не относятся.

Реализация (один файл, `crates/modules/rustok-pages/src/services/page/reviewed_publish.rs`; публичная сигнатура `publish_reviewed` не изменилась):

- **Оптимистичная фаза до `self.db.begin()`.** `precompile_reviewed_publish` читает без блокировок страницу, тела, каталог символов, шаблоны и сценарный baseline, собирает их в `ReviewedPublishInputs`, считает отпечаток (`stable_hash` по всем входам) и выполняет `evaluate_reviewed_publish`. Проверка владения (`enforce_owned_scope`) стоит до любой CPU-работы. Любая ошибка этой фазы даёт `None`: фаза ничего не решает и ничего не отклоняет.
- **`evaluate_reviewed_publish` не делает ввода-вывода.** Результат зависит только от аргументов: проверка снимка символов, раскрытие символов, сборка layout, разбор, сценарный гейт, компиляция.
- **Фаза под блокировкой** сохраняет прежний порядок обращений к БД и блокировок. После повторного чтения тех же входов уже внутри транзакции отпечаток сравнивается с оптимистичным. Совпал — берётся готовый результат; не совпал или оптимистичной фазы не было — вычисление идёт под блокировкой, как раньше. Пока транзакция открыта, чтений через `self.db` нет.

Корректность: готовый результат используется только при побитово равных входах. Ложное несовпадение стоит времени, но не может дать неверный артефакт.

Тесты (5 новых юнит-тестов в том же файле): отпечаток стабилен и меняется при изменении источника, локали, ключа шаблона, символов, записи шаблона и baseline; ошибка при отсутствии шаблона layout для локали; сценарный гейт пропускается без baseline и отклоняет нечитаемый baseline; гейт настроек модуля; ключи шаблонов по умолчанию.

Верификация: **файл ни разу не компилировался и ни один его тест не запускался** — `rustok-pages` в песочнице не собирается. Сделано: синтаксический разбор, `rustfmt --check` (чисто), ручная проверка импортов, видимости вызываемых функций, `Send`-границ и ожиданий тестов по `symbols.rs`/`error.rs`. Поведение под нагрузкой и выигрыш во времени удержания блокировки не измерялись. Это самый рискованный фикс серии: до слияния нужны `cargo test -p rustok-pages` и интеграционные наборы `tests/artifact_loss_*_postgres.rs`, `storefront/tests/native_storefront_*_sqlite.rs`.

Изменения поведения и замечания:

1. Ошибки `feature_disabled` и ошибки загрузки baseline теперь возвращаются раньше ошибок раскрытия символов, layout и разбора. Набор ошибок прежний, меняется приоритет, когда их несколько.
2. Публикация делает примерно вдвое больше читающих запросов (входы читаются до транзакции и повторно в ней).
3. Ошибки оптимистичной фазы отбрасываются без записи в лог.
4. Компиляция по-прежнему выполняется на async-задаче, без `spawn_blocking`.
5. Строки символов и шаблонов внутри транзакции читаются без блокировок — как и до фикса.

### Фикс 9 (2026-11-10): единая политика preview и publish (§6.2), «настраиваемая» политика (§6.1) — РЕАЛИЗОВАН

Root cause: превью Pages строило свою политику рендера (допускала `http:`), а публикация — свою (`allow_http: false`). Страница выглядела в превью исправной и отклонялась при публикации. Отдельно документация обещала настройку политики оператором, которой в рантайме нет.

Реализация:

- **`rustok-page-builder/src/static_landing.rs`**: `static_publish_render_policy()` — единственный источник политики. Её используют и `StaticLandingCompiler::new`, и превью.
- **`rustok-pages/admin/src/builder.rs`**: `render_preview` берёт эту политику. `load_project` возвращает `PageBuilderServiceError::Runtime`, если не удалась сборка шаблона (раньше ошибка терялась).
- **§6.1**: выбран вариант «убрать обещание». Утверждения о настройке оператором удалены из `static_publish_policy.rs`, `README.md`, `CRATE_API.md` модуля. Настройка не реализована.

Тесты: два новых теста в `static_landing.rs` — компилятор по умолчанию использует общую политику; превью опускает ровно те `http:`-ресурсы, которые отклоняет публикация. Верификация: часть в `rustok-page-builder` собрана и выполнена (126 тестов). Часть в `rustok-pages-admin` — только разбор, формат и чтение.

Изменение поведения: превью больше не показывает `http:`-ресурсы. Страницы, которые раньше «работали» в превью с такими ресурсами, теперь выглядят в превью так же, как после публикации.

### Фикс 10 (2026-11-10): ошибка сохранения без расхождения клиента и сервера (§6.4) + несколько request-эффектов (§6.3) — РЕАЛИЗОВАН

Root cause §6.4: если автосохранение после правки отклонялось, сервер отвечал ошибкой. Правка при этом уже была в черновой сессии, а клиент оставался с оптимистичным канвасом и не знал, что на сервере она не сохранена. Root cause §6.3: из списка request-эффектов исполнялся первый, остальные молча отбрасывались.

Реализация, сервер (`rustok-pages/admin/src/browser_intent.rs`, `browser_problem.rs`):

- Сохранение отклонено фасадом → контроллер помечается (`REVISION_CONFLICT` → конфликт, иначе `mark_save_failed`), кэш persisted-ревизии сбрасывается, черновик фиксируется. Ответ — HTTP 200 с `reload: true` и новым полем `save_failure { status (409/422), error, code, intent: "save" }`.
- Синтезированное автосохранение, заблокированное до отправки (например, блокирующая диагностика), обрабатывается так же.
- Явный пользовательский `save`, упавший на этапе dispatch, остаётся HTTP-ошибкой — здесь расхождения нет.
- §6.3: `single_request` — более одного запроса даёт `UnsupportedRequestBatch { count }` (HTTP 422) до исполнения чего-либо. Сейчас ни один интент не порождает два запроса; это страховка от молчаливой потери при будущем изменении контракта, а не поддержка пакетов.

Реализация, клиент (`crates/ui/fly/browser/assets/fly-browser.js`): при `result.save_failure` проблема сообщается через существующий канал и перед перезагрузкой сохраняется в `sessionStorage` (`fly:ssr-save-failure:<page>`, 10 минут, сообщение до 1024 символов). После перезагрузки адаптер сообщает её снова. Запись снимается ответом, в котором рабочая копия уже не `dirty` (исправлено позже, см. «Ревью `draft_session.rs` и `crates/ui/fly`»; сначала её снимал любой следующий интент без `save_failure`). Next-админка использует тот же файл, отдельного порта не требуется.

Тесты и верификация:

- Сервер: пять новых тестов вспомогательных функций (и четыре прежних теста тех же функций) извлечены и **выполнены**. Сам поток `dispatch_pages_browser_intent_with_store` и тесты в `browser_problem.rs`, зависящие от типов фреймворка, — только чтение.
- Клиент: контрактный тест `public_bundle_surfaces_save_failures_across_the_reload` (проверяет наличие строк в бандле) выполнен. Поведение один раз проверено локальным скриптом на jsdom (в репозиторий не включён; вызов `location.reload` в нём подменён счётчиком). **Playwright-спека не добавлена и не запускалась:** браузеров в песочнице нет. В реальном браузере цепочка «ответ → sessionStorage → перезагрузка → повторное сообщение» не проверена.

Замечание: если `sessionStorage` недоступен, сообщение теряется при перезагрузке; сама перезагрузка и серверное состояние остаются корректными.

### Фикс 11 (2026-11-10): файловый стор черновиков — права, атомарность, раздел по тенантам (§2.6); попутно §5.3, §6.5, §6.6, §6.7 — РЕАЛИЗОВАН

Root cause: черновики всех тенантов лежали плоско в общем temp-каталоге ОС с правами по умолчанию.

Реализация (`rustok-page-builder/admin/src/draft_session.rs`):

- **Раскладка:** `<dir>/t-<тенант>/<token>.json`, `_unattributed/`, `_locks/`. Каталог задаётся `RUSTOK_DRAFT_SESSION_DIR`, иначе `temp_dir()/rustok_draft_sessions`. Недопустимый идентификатор тенанта → `InvalidOwner` (422); токен чужого тенанта → `OwnerMismatch`.
- **Права:** каталоги 0700, файлы 0600. Корень-симлинк отклоняется. Недоступный каталог — отказ, без отката на другой путь. Владелец корня проверяется явно — добавлено на код-ревью, см. находку 4 ниже.
- **Атомарность:** запись через временный файл, `fsync`, `rename`; продление TTL — CAS и только когда запись устарела.
- **`prune`:** обходит все разделы, удаляет устаревшие lock-файлы, временные файлы и файлы старой плоской раскладки старше одного TTL.
- **§6.5:** `commit` сохраняет runtime-контекст; замена — отдельным `commit_with_context`.
- **§6.6:** `DurableSsrDraftSessionStore::prune` возвращает сумму по уровням.
- **§6.7:** сбой продления TTL при `load` возвращается вызывающему.
- **§5.3:** `InMemorySsrDraftSessionStore::load` берёт блокировку на чтение; срок жизни записи — атомарное поле.

Тесты: 19 новых (всего 42 в файле), выполнены через `#[path]`-крейт. Набор прогнан 40 раз подряд без падений. Выборочно проверено, что тесты падают при временной порче проверяемого кода.

Верификация: выполнено на rustc 1.88 вне настоящего крейта. Сам `rustok-page-builder-admin` не собирался. Права проверены только на Linux; ветка для не-Unix не компилировалась.

Известные ограничения (не исправлены):

1. `prune` в production никем не вызывается по расписанию — было и до фикса. Без планировщика устаревшие файлы копятся до ручной очистки.
2. `fsync` добавляет задержку к каждой фиксации черновика; не измерялась.
3. Падение между записью в раздел тенанта и удалением из `_unattributed` оставляет устаревшую копию до истечения TTL.
4. Повреждённые `.json` в разделах `prune` не удаляет.
5. Черновики старой плоской раскладки не переносятся: после выката активные сессии редакторов начнутся с сохранённой ревизии. Несохранённые правки, существовавшие только в старом черновике, будут потеряны.
6. SSR-восстановление в Pages по-прежнему проглатывает ошибки загрузки черновика.

### Сопутствующие P3: §4.3 и §5.2 — РЕАЛИЗОВАНЫ

- **§4.3** (`fly/src/command/model.rs`, `fly-ui/src/machine.rs`): правка после неудачного сохранения больше не сбрасывает `save_failed`. Флаг снимают только начало нового сохранения и его подтверждение. Тесты в обоих крейтах, выполнены.
- **§5.2** (`fly/src/command/editor.rs`): если инкрементальная валидация отказалась, уже применённая рабочая копия передаётся в полный путь вместо повторного clone+apply. Нового теста нет; путь покрыт существующими тестами `fly`, они проходят. Выигрыш не измерялся.

### §7.1 (2026-11-10): `scopeLandingCss` — ИСПРАВЛЕНО ЧАСТИЧНО

Аудит указал три вещи: некорректное экранирование кавычек, passthrough при префиксе `@scope` и завышенное обещание `contain`. При разборе нашлись ещё две, причём одна серьёзнее всех названных.

Найдено и исправлено (`apps/next-frontend/packages/rustok-pages/src/components/css-scope.ts`):

1. **Выход CSS за контейнер.** Лишняя `}` во входе закрывала обёртку `@scope`, и всё после неё применялось к странице хоста: `} header { display: none; }` скрывал шапку витрины. То же давал passthrough: CSS, начинающийся с `@scope (...) {…}`, возвращался как есть вместе с правилами после блока. Теперь результат — всегда ровно один полный блок `@scope` (в первой версии это утверждение было неверным: код-ревью нашло три способа обхода, см. находку 3 ниже): лишняя `}` вне блоков отбрасывается; незакрытые блоки, строки, комментарии и `url()` закрываются. Без изменений пропускается только собственный выход функции (один блок с тем же селектором).
2. **Строки.** Экранирование определялось по предыдущему символу, поэтому после `content: "\\"` остаток CSS считался строкой. Теперь обратная косая экранирует следующий символ, а неэкранированный перевод строки завершает строку — как в браузере.
3. **Ложная замена селекторов.** `body`/`html`/`:root` заменялись на `:scope` в `@keyframes body` и в `[rel=html]`. Теперь замена только в позиции селектора.
4. **Документация `contain`.** README и CRATE_API пакета утверждали, что `contain: style layout` не выпускает стили наружу. Это неверно: изоляцию правил даёт только `@scope`. Текст исправлен; сам inline-стиль оставлен.

Тесты: 4 новых и 1 изменённый (всего 13) в `pages-frontend.test.mjs`, выполнены через `node --test`. Набор «несбалансированный вход» (17 случаев) проверяется отдельным, независимо написанным разборщиком в тесте. На версии файла с HEAD четыре из новых тестов падают, остальные девять проходят. `tsc --strict` по файлу — без ошибок.

Не сделано:

- **Fallback для браузеров без `@scope` не реализован.** В них блок отбрасывается целиком, и лендинг отображается без стилей. Формулировка аудита «CSS выйдет за контейнер» для таких браузеров неверна: стили не применяются вовсе, хост не страдает. Порог поддержки (Chrome/Edge 118, Safari 17.4, Firefox 128) теперь записан в README и CRATE_API. Нужен ли fallback с префиксацией селекторов — продуктовое решение.
- **Разборщик остался рукописным.** Замена на готовый парсер потребовала бы новой зависимости в пакете. Вложенные правила CSS и авторские `@scope`/неизвестные at-правила разбираются как блоки объявлений: `body` внутри них не заменяется.
- В реальном браузере результат не проверялся — только структурно, тестами.

### Код-ревью серии (2026-11-10) — 5 дефектов найдено и исправлено; тесты НЕ запускались

Метод: чтение диффа серии и вызываемого им кода. По решению заказчика наборы тестов в этом раунде не запускались — ни Rust, ни node. Всё, что ниже названо «исправлено», проверено только чтением и статически: разбор (`rustc -Z parse-crate-root-only`), `rustfmt --check` (новых расхождений нет), `cargo check --all-targets` и `clippy` scratch-крейта (в него входят `draft_session.rs` целиком и извлечённый `failed_save_hash` с тестом), `tsc --strict` и `node --check` для TS/JS. Новые тесты написаны, но ни разу не выполнялись.

**Находка 1. Ссылка rustdoc на приватный метод ломала CI (Фикс 8).** Документация публичного `publish_reviewed` ссылалась на приватный `precompile_reviewed_publish`. CI запускает `cargo doc` с `RUSTDOCFLAGS=-D warnings`, поэтому сборка упала бы. Ссылка заменена текстом в обратных кавычках. Остальные добавленные doc-комментарии серии проверены скриптом: ссылок на приватные элементы, голых тегов и URL нет. `cargo doc` здесь не запускался.

**Находка 2. Цикл перезагрузок после неудачного сохранения (Фикс 10).** Автосохранение синтезируется после любого интента на «грязном» черновике, включая `select`. Фикс 10 стал сохранять черновик с флагом `save_failed`, и каждый следующий клик повторял то же сохранение, получал тот же отказ и перезагружал страницу. При устойчивой причине (блокирующая диагностика, 422 от Pages, недоступный сервис) редактор был бы непригоден.

- Исправление (`rustok-pages/admin/src/browser_intent.rs`): автосохранение пропускается, если до интента черновик уже был в состоянии `save_failed` и хэш проекта после интента не изменился. Повтор происходит при следующей правке документа или при явном `save`.
- Следствие: после временного сбоя клики по холсту сохранение не повторяют; индикатор «не сохранено» в тулбаре остаётся.
- Тест `failed_save_hash_tracks_the_document_the_save_failed_for` проверяет только вспомогательную функцию. Само условие в dispatch-функции тестом не покрыто: она требует leptos и транспорт.
- Документация: `rustok-pages/admin/CRATE_API.md`.

**Находка 3. `scopeLandingCss` допускал выход CSS за контейнер тремя способами (§7.1).** Во всех трёх разборщик и браузер видели разную структуру блоков, и `}` из входа закрывала обёртку `@scope`.

1. Обратная косая перед CR LF внутри строки. Для браузера это одно продолжение строки; разборщик пропускал только CR и на LF считал строку оконченной.
2. Распознавание `url(` по соседним символам. `#url(`, `@url(` и `éurl(` разборщик считал url-токеном (непрозрачным до `)`), а браузер — обычными скобками. Обратный случай: `u\72l(` — url-токен для браузера и обычные скобки для разборщика.
3. Пробелы после `url(` определялись через `\s`. Неразрывный пробел для CSS — часть имени, а не пробел.

- Исправление (`css-scope.ts`): разбор переписан потокенно по CSS Syntax — имена с экранированием, hash, at-keyword, числа с единицами, строки, комментарии, url-токен. Url-токен распознаётся только там, где его начал бы токенайзер браузера.
- Дополнительно: `</style` в результате записывается как `<\/style`. Раньше такая последовательность в CSS закрывала элемент `<style>` в SSR-разметке.
- Тесты: 2 новых в `pages-frontend.test.mjs` (6 векторов обхода с точным ожидаемым выходом, 4 случая «похоже на `url(`, но не он», `</style`). Ожидаемые строки выведены вручную, без запуска — при первом прогоне возможны расхождения именно в ожиданиях.
- Оценка риска: политика публикации на Rust запрещает в значениях CSS обратную косую, управляющие символы и фигурные скобки, поэтому опубликованные по текущей политике артефакты эти векторы не несут. Дефект был в заявленном втором рубеже защиты.
- Не закрыто: at-правила с глобальными именами (`@keyframes`, `@font-face`, `@property`) `@scope` не ограничивает. HTML артефакта по-прежнему вставляется как есть, без второй санитизации (источник разметки исправлен в записи «§7.2»).

**Находка 4. Проверка владельца каталога черновиков не работала для привилегированного процесса (Фикс 11).** Владение проверялось косвенно: `chmod` чужого каталога падает, а в чужой каталог 0700 нельзя войти. Для процесса с правами root оба утверждения неверны. Локальный пользователь мог заранее создать `<temp>/rustok_draft_sessions`, и сервер писал бы черновики в каталог, который тот может переименовывать и наполнять.

- Исправление (`draft_session.rs`): `ensure_owned_by_process` при создании стора создаёт в корне пробный файл и сравнивает его uid с uid каталога. Несовпадение — отказ (стор недоступен, операции возвращают `Io`). Проверяется только корень: разделы создаются внутри него.
- Тест `file_store_checks_the_storage_root_owner_and_leaves_no_probe` покрывает только принимающую ветку. Ветка отказа требует каталога другого пользователя и не тестируется.
- Не-Unix: проверки нет, как и прав доступа.

**Находка 5. Недоступное хранилище черновиков отдавалось как 422 (Фикс 11 / §6.3).** После Фикса 11 неисправный каталог даёт `Io` на каждый интент; клиент получал «Unprocessable Entity». Теперь `SsrDraftSessionError::Io` → 503, как `LockTimeout`. Тест `unusable_draft_storage_is_service_unavailable`.

Проверено чтением, дефектов не найдено: Фикс 8 (сигнатуры вызываемых сервисов, порядок блокировок, состав отпечатка входов, отсутствие чтений из пула внутри транзакции); Фикс 9 (политика общая, импорт под `ssr`, неиспользуемых импортов не осталось); серверная часть Фикса 10 (типы, заимствования, коды статусов); §5.2 (переиспользуемая копия эквивалентна повторному clone+apply); удалённая из `lib.rs` инлайн-копия `mod model` совпадает с `model.rs` с точностью до отступов.

Замечено и оставлено без изменений:

1. В production-сторе (`DurableSsrDraftSessionStore`) срок жизни записи на диске продлевается только фиксацией, а не чтением: `load` идёт через `load_record`. Так было и до серии. Продление TTL по CAS и возврат его ошибки (§6.7) действуют только при прямом использовании `FileSsrDraftSessionStore`. Комментарий в `prune` о старой раскладке исправлен под это.
2. Текст ошибки недоступного хранилища содержит путь каталога и уходит в ответ редактору. Эндпоинт доступен только авторизованным редакторам.
3. Поиск записи по токену при промахе обходит каталоги всех тенантов (один `stat` на тенанта).
4. Ключ лимита «сессий на владельца» в `prune` склеивает tenant и user через `:` — было до серии; при `:` в идентификаторах возможна коллизия.
5. Сообщение о неудачном сохранении после перезагрузки исчезает при первом же следующем интенте; остаётся только индикатор в тулбаре.

### Проход по AGENTS.md (2026-11-10): паритет сторфронтов, обработка отказов, фикс 8 — 5 дефектов исправлено; тесты не запускались

Правки серии сверены с `AGENTS.md`. Ниже — что это изменило в коде. Тесты по-прежнему не запускались: всё проверено чтением, компиляцией доступной части и статическими скриптами (перечень в конце записи).

**База.** `origin/main` ушёл вперёд на два коммита (`4e24e1c` → `357a961`). Рабочее дерево перенесено на `357a961`. Коммит `357a961` чинит те же две ошибки компиляции, что описаны выше в «Дефектах сборки» (пункты 1 и 2). Оставлен вариант этой серии: в `lib.rs` подключается файл `model.rs` (в `main` осталась его вставленная копия, то есть два экземпляра одного кода), в `draft_session.rs` — общий `commit_internal`.

**1. Leptos-сторфронт: стили лендинга не применялись, а два экрана из четырёх их не ограничивали** (`crates/modules/rustok-page-builder/storefront`). Аудит (§7.1) смотрел только Next-пакет; у функции есть двойник на Rust, `scope_landing_css`, и фикс §7.1 его не затронул.

- CSS оборачивался в `@scope (.rustok-page-builder-storefront)`, но класс корневого элемента задаёт хост. `rustok-pages-storefront` передаёт `rustok-pages-storefront__builder-body`, поэтому элемента с нужным классом на странице не было и правила ни к чему не применялись: опубликованный лендинг отображался без стилей. Теперь корень — элемент, который компонент рендерит сам: `[data-fly-page-body]` (`LANDING_SCOPE_SELECTOR`).
- `LocalizedPageBuilderStorefront` и редактор на сторфронте (`inline_edit.rs`) выводили CSS проекта без `@scope` — стили лендинга действовали на всю страницу хоста. Теперь все четыре поверхности идут через `scope_landing_css`.
- Сама функция имела те же дефекты, что Next-версия до §7.1, и один дополнительный: вход, начинающийся с `@scope`, возвращался без изменений. Реализация заменена портом `css-scope.ts` (новый файл `storefront/src/css_scope.rs`, старый код удалён).
- Чтобы две реализации не расходились снова, 26 случаев с точным ожидаемым результатом вынесены в `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json`; их проверяют и Rust-тест, и `pages-frontend.test.mjs`. Для совпадения в `css-scope.ts` обрезка пробелов заменена с `String.prototype.trim` на пробельные символы CSS (неразрывный пробел для CSS — часть имени).

Тесты: 9 в `css_scope.rs`; в `contract_tests.rs` один изменён и один новый (новый проверяет по исходникам, что каждая поверхность выводит только обёрнутый CSS); в Node-файле один тест заменён на проверку общих векторов (всего 15). Ожидаемые значения выведены вручную. `css_scope.rs` компилируется в рабочей среде (вместе с тестами); остальной крейт зависит от Leptos и здесь не собирается — правки в `lib.rs`, `localized_route.rs`, `inline_edit.rs` проверены только разбором синтаксиса. В браузере не проверялось, в том числе то, что `<style>` в Leptos SSR выводит текст без HTML-экранирования.

**2. Бесконечный цикл при неудаляемой устаревшей блокировке** (`draft_session.rs`, `with_token_lock`). Устаревший lock-файл удалялся через `let _ = fs::remove_file(..); continue;`. Если удаление не удавалось, файл снова оказывался устаревшим, а `continue` обходил проверку дедлайна: поток запроса крутился без остановки. Теперь ошибка удаления возвращается как `Io`. Тест: `stale_token_lock_that_cannot_be_removed_fails_instead_of_retrying_forever`. По чтению кода, до исправления он бы завис; сам тест не запускался.

**3. `prune` считал удалёнными записи, которые удалить не удалось** (файловый и KV-бэкенд). Счётчик увеличивался независимо от результата. Теперь учитываются только фактические удаления, отказы пишутся в лог.

**4. Лимит сессий на владельца молча превышался.** Если при создании сессии не удавалось удалить самую старую, новая всё равно записывалась. Теперь создание завершается ошибкой `Io` до записи.

**5. Фикс 8 ломал два контрактных верификатора; переработан.** Порядок операций `publish_reviewed` закреплён в `rustok-page-builder/contracts/page-builder-publish-runtime-review.json` (`new_operation_order`) и проверяется скриптами `verify-page-builder-publish-runtime-review.mjs` и `verify-pages-builder-scenario-baseline.mjs`. Первая версия фикса 8 убрала из транзакции разбор проекта, сценарный гейт и компиляцию в одну общую функцию, и оба скрипта перестали проходить. При записи фикса 8 эти скрипты не запускались — ошибка обнаружена только сейчас, сравнением результатов всех верификаторов с `main`.

Текущая версия:

- Транзакция снова выполняет шаги в порядке контракта и с прежним приоритетом ошибок: раскрытие символов и layout → разбор проекта → проверка настройки модуля (с блокировкой) → чтение baseline (с блокировкой) → сценарный гейт → компиляция. Изменение приоритета ошибок, описанное в замечании 1 к фиксу 8, устранено.
- До транзакции по-прежнему считаются только две тяжёлые операции: сценарный гейт и компиляция. Отпечаток теперь берётся от того, что они читают, — раскрытых документов всех локалей и baseline. Транзакция использует готовый результат только при равном отпечатке, иначе выполняет оба шага сама.
- Раскрытие символов и layout в транзакции выполняется всегда (как до фикса 8), то есть под блокировкой остаётся больше работы, чем в первой версии.
- Ошибка предварительной фазы больше не отбрасывается молча: пишется в лог уровня `debug` (замечание 3 к фиксу 8).
- Контракт изменён: в `new_operation_order` маркер `ensure_candidates_allowed_in_tx` заменён на `load_scenario_baseline_in_tx`, `reviewed_publish_fingerprint`, `ensure_candidates_allowed(` (функция разделена на чтение под блокировкой и чистую проверку); добавлен раздел `precompiled_evaluation`. Оба верификатора обновлены под это. В первый добавлены три проверки: результат используется только при `precompiled.fingerprint == fingerprint`; внутри транзакции нет обращений к `self.db`; в предварительной фазе гейт идёт перед компиляцией. Каждая проверена на намеренно испорченной копии файла — скрипт завершается с ошибкой.
- Тесты в `reviewed_publish.rs`: два переписаны под новые функции (`evaluation_fingerprint_tracks_documents_and_baseline`, `expansion_requires_the_layout_template_for_every_locale`). Файл по-прежнему ни разу не компилировался.
- Фикс 8 описан в `rustok-pages/CRATE_API.md` (инвариант) и `rustok-pages/README.md` (ограничения) — раньше его там не было.

**Остальные отброшенные результаты в `draft_session.rs`.** В не-тестовом коде не осталось `let _ = <вызов>`. Проба владельца каталога (ревью, находка 4) теперь возвращает и ошибку удаления пробного файла. Там, где отказ не должен ронять операцию (удаление просроченной записи, временного файла, отпущенной блокировки, сброс каталога на диск), он пишется через `tracing`; путь в лог не попадает, потому что имена файлов — это токены сессий. Для этого в `rustok-page-builder-admin` добавлена зависимость `tracing` (workspace-зависимость, уже используется в 29 из 46 admin-крейтов); `Cargo.lock` поправлен вручную одной строкой, `cargo` его не пересчитывал — проверить `cargo metadata --locked` на полном тулчейне.

**Документация.**

- Ограничения перенесены из этого журнала в README владельцев, раздел «Known Limitations / Pending Implementation»: `rustok-page-builder/admin`, `rustok-page-builder/storefront`, `rustok-pages/admin`, `apps/next-frontend/packages/rustok-pages`. При сверке с кодом уточнено одно: общий лимит сессий (`maximum_entries`) файлового и KV-бэкенда применяется только в `prune`, а `prune` никто не вызывает.
- Оставшаяся работа записана в `docs/modules/page-builder-backlog.md`, пункт 10 (это единственный план Page Builder).
- В `docs/standards/logic-hygiene-patterns.md` добавлены три паттерна по найденным дефектам: `P-CONSUMER-GRAMMAR`, `P-TWIN-VECTORS`, `P-RETRY-PROGRESS`.

**Что запускалось.** `rsparse` и `rustfmt --check` по изменённым `.rs` (новых расхождений форматирования нет); `cargo check --all-targets` и `clippy` рабочей среды (rustc 1.88; `draft_session.rs`, `css_scope.rs` и их тесты компилируются, предупреждений в новых строках нет); `tsc --strict` для `css-scope.ts`, `node --check` для тестового файла; `verify-remediation-gate.py --skip-cargo` — проверка диффа пройдена, шаг архитектурных зависимостей падает на `rustok-server → flex` (есть в `main`, диффом не затронуто); все 1331 скрипта `scripts/verify/*.mjs` и `crates/**/scripts/verify/*.mjs` (без `*.test.mjs`) запущены на рабочем дереве и на чистой копии `357a961`, вывод сравнён построчно: результаты совпадают (714 проходят в обоих, 617 падают в обоих с одинаковым выводом; причины падений на `main` не разбирались). Среди проходящих — `verify-docs`, оба контрактных верификатора из пункта 5, `verify-page-builder-contract-parity`, `verify-pages-ui-boundary`, `verify-ui-parity`.

**Что не запускалось.** Все тесты, включая новые и `*.test.mjs` верификаторов. Сборка `rustok-page-builder-storefront`, `rustok-page-builder-admin`, `rustok-pages`, `rustok-pages-admin` как крейтов; `cargo doc`; `rustok-doctor`; Playwright.

**Не сделано.**

- `draft_session.rs` вырос до ~150 КБ (в `main` — 96 КБ), около 40 % строк — тесты. Разбиение по ответственности записано в бэклог.
- Этот файл написан по-русски, а `AGENTS.md` §12 требует английский для всех артефактов, кроме `README.ru.md`. Файл был русским до начала серии; перевод не делался, решение за владельцем.

### §7.2 (2026-11-10): Next-сторфронт показывал исходник страницы вместо опубликованного артефакта — ИСПРАВЛЕНО НА СТОРОНЕ NEXT; тесты написаны, не запускались

Аудит описал §7.2 как хрупкий regex (P3). При чтении обоих концов транспорта оказалось, что `parseArtifactHtml` на реальных страницах вообще не вызывался. Вывод сделан чтением кода; маршрут в браузере не открывался.

**Что было.** Маршрут `app/[locale]/pages/[slug]` передавал в `fetchPageArtifact` поле `body.content` из GraphQL. `pageBySlug.body` — это рабочий документ страницы (`format: "grapesjs"`, JSON проекта), а не артефакт. `fetchPageArtifact` шёл за артефактом по HTTP только при `format === "fly_artifact_url"` или если строка начиналась с `/api/pages/`. Формат `fly_artifact_url` не выдаёт ни один серверный путь: в `crates/modules/rustok-pages/src` и `apps/server/src` его нет, Leptos-адаптер выдаёт `fly_artifact_payload`. Поэтому каждая страница попадала в последнюю ветку: `<div class="fly-raw-content">${content}</div>` и дальше в `dangerouslySetInnerHTML`.

Следствия:

1. Опубликованный лендинг на Next-сторфронте не отображался: вместо страницы выводился JSON проекта.
2. Разметка из текстового поля рабочего документа попадала в DOM без санитайзера публикации. Санитайзер работает при компиляции артефакта и рабочий документ не меняет; `serde_json` символ `<` не экранирует. Попытка эксплуатации не проводилась; проверяет ли сохранение черновика текстовые поля на разметку, не выяснялось.
3. Строка, начинающаяся с `http`, запрашивалась сервером Next как есть.
4. Запрос артефакта не нёс заголовок тенанта, в отличие от GraphQL-запроса.
5. Сам `parseArtifactHtml` был fail-open: не найдя `<body>`, возвращал весь вход как разметку; ленивое совпадение обрезало тело на первом `</body>`; стили собирались со всего документа, включая тело.

Leptos-сторфронт этого дефекта не имеет: его адаптер подменяет тело опубликованным артефактом. Это расхождение хостов по §5.6 `AGENTS.md`.

**Что сделано** (только TypeScript; сервер не менялся, `rustok-pages` здесь не собирается):

- `fetchPageArtifact(pageId, locale, options)` запрашивает `GET /api/pages/{id}/artifact?locale=…` с `Accept: text/html` и `X-Tenant-Slug`. `pageId` обязан быть UUID, локаль кодируется. 404 возвращает `null`, прочие коды и неверный документ — ошибка. Старая сигнатура, ветка абсолютного URL, ветка «встроенный документ» и ветка `fly-raw-content` удалены.
- `parseArtifactHtml` принимает только огибающую `compose_document_html` (`crates/ui/fly/src/render.rs`): фиксированное начало документа, `<style>` последним элементом `head`, `</style></head><body>`, тело, `</body></html>` в самом конце. Рендерер экранирует каждый `<` в текстах и значениях атрибутов `head` и в стилях, поэтому первое вхождение `</style></head><body>` и есть граница. Всё остальное, включая `<` в стилях, даёт `ArtifactEnvelopeError`; вход как разметка не возвращается никогда.
- GraphQL-запрос сторфронта больше не запрашивает `body`; тип `PageBody` и поле `PageDetail.body` удалены. Маршрут при отсутствии артефакта и при ошибке отвечает `notFound()`.
- Построение запроса и разбор ответа вынесены в чистые функции `artifact.ts` (`pageArtifactPath`, `buildPageArtifactRequest`, `readPageArtifactResponse`), чтобы их можно было тестировать без Next.

**Тесты** (`src/pages-frontend.test.mjs`, написаны, не запускались): три старых теста `parseArtifactHtml` заменены восемью, в файле теперь 20 тестов. Среди новых — тело, содержащее `</body>` и текст границы; девять отклоняемых входов, включая JSON проекта и документ с содержимым после `</html>`; сверка `ARTIFACT_ENVELOPE` со строкой формата в `render.rs`; путь эндпоинта и отказ для не-UUID; политика кэша; коды ответа 200/404/500/304.

**Что запускалось.** `tsc --strict --noEmit` (TypeScript 7.0.2) по `artifact.ts`, `pages.ts`, `css-scope.ts` и отдельно по `page.tsx`, оба раза с заглушками вместо `next` и `@/shared/*`; `node --check` тестового файла; `grep` строки формата в `render.rs` (одно вхождение); `verify-docs.mjs` — код 0; gate `verify-remediation-gate.py --skip-cargo` — инварианты диффа пройдены, отказ на прежнем нарушении `rustok-server` → `flex` (есть и в `main`). `npm run typecheck` и `next build` не запускались: зависимости приложения не установлены.

**Не закрыто** (в бэклоге, пункт 10, и в README пакета):

- Транспорт остался HTML-документом. Отдельные поля тела и стилей требуют изменения эндпоинта в `rustok-pages`.
- При смене огибающей в `render.rs` все лендинги на Next ответят 404, пока не обновлён `ARTIFACT_ENVELOPE`. Сверочный тест это ловит, если его запускают.
- Публичное поле `pageBySlug.body` по-прежнему отдаёт анонимному клиенту рабочий документ опубликованной страницы.
- Draft Mode не показывает неопубликованное: `pageBySlug` и эндпоинт артефакта отдают только опубликованное. Раньше README утверждал обратное; исправлено.
- Канал в запросе артефакта не передаётся: в Next-сторфронте каналов нет.
- Для страницы без артефакта Leptos показывает уведомление «unsupported format», Next отвечает 404.

### §7.2, продолжение (2026-11-10): транспорт опубликованной страницы — ПЕРЕПИСАН; не собирался, Rust-тесты не запускались

Запись «§7.2» выше описывает промежуточное состояние: Next запрашивал HTML-документ артефакта и разбирал его по огибающей. Оно снято целиком. Решение записано в `DECISIONS/2026-11-10-published-page-landing-transport.md` (статус Accepted / In progress, пока наборы тестов не пройдены).

**Что решено.**

1. Тело страницы (`body`) — рабочий документ редактора, а не источник для показа. Его получает только тот, кто может этот документ сохранить или опубликовать: `pages:update` или `pages:publish` над страницей, то же правило `enforce_owned_scope`, что у команд записи. Черновик опубликованной страницы — только при `pages:update`, как требует ADR от 2026-10-09. Остальные, включая анонимных читателей, покупателей и сотрудников с одним `pages:read`, получают `body: null`. Метка роли не учитывается.
2. Опубликованное содержимое — типизированное поле `landing { locale html css artifactHash }` в GraphQL-типе страницы. Оно берётся из привязанного артефакта (`load_public_bound_artifact_with_fallback`) для локали страницы и канала запроса и равно `null`, если страница не опубликована, не видна в канале или не имеет лендинга.
3. Оба сторфронта показывают только `landing`. У них нет ни типа, ни выборки, ни ветки рендера для `body`.
4. SEO-запись страницы (`seo_targets.rs`) берёт запасное описание и основное изображение из разметки лендинга и `body` не читает.

**Что удалено.** В Leptos-сторфронте: синтетические форматы тела `fly_artifact_payload` и `fly_artifact_url`, ветка рендера рабочего документа, iframe-ветка, сводка «сырого» тела и ключ `pages.body.rawFormat`, зависимость `fly` в `ssr`. В Next-пакете: `artifact.ts` целиком (`fetchPageArtifact`, `parseArtifactHtml`, `ARTIFACT_ENVELOPE`), `pageCacheTags`, второй HTTP-запрос. Страница и её разметка теперь приходят одним GraphQL-запросом (`STOREFRONT_PAGE_QUERY`); ошибка GraphQL при частичных данных считается ошибкой чтения, а не «лендинга нет».

**Найдено по ходу.**

- До правки SEO читал страницу по id от имени системы и получал рабочую копию. Для опубликованной страницы с черновиком запасное описание (когда `meta_description` пуст) было первыми 180 символами сериализованного JSON проекта, а изображение искалось в неопубликованном документе. Вывод сделан чтением кода; SEO-ответ живой страницы не запрашивался.
- Метка роли запроса выводится из набора прав (`SecurityContext::from_permission_snapshot`), и стандартный набор покупателя удовлетворяет эвристике «менеджер» (`PRODUCTS_READ` и `ORDERS_READ`). `can_read_non_public_pages` смотрит на метку, поэтому вошедший покупатель, судя по коду, может прочитать запись неопубликованной страницы (заголовок, slug, метаданные; тело — нет). Не воспроизводилось и не исправлялось: причина в `rustok-core`, а `from_permission_snapshot` используется по всему репозиторию (порядка ста обращений вне тестов). Записано в бэклог.

**Что изменилось для пользователей.**

- Опубликованная страница без лендинга (например, опубликованная через `publish_non_builder_if_current`) не показывает содержимого: Leptos выводит пустое состояние в карточке страницы, Next отвечает 404. Запасного источника нет.
- Сотрудник с одним `pages:read` больше не откроет документ страницы. Preflight возможностей Page Builder по-прежнему сопоставляет `Preview` и `Tree` с `pages:read`; такой пользователь пройдёт его и получит пустой документ.
- Внешний GraphQL- или REST-клиент, читавший `body` без `pages:update` или `pages:publish`, получит `null`.
- Маршрут страницы в Next больше не кэшируется: чтение идёт через `storefrontGraphql` с `no-store`. Раньше артефакт кэшировался на 60 секунд с тегами. Каждый просмотр — один GraphQL-запрос и одна загрузка артефакта на сервере.
- SEO: изображение, заданное только как CSS-фон в стилях лендинга, больше не находится — поиск идёт по элементам `<img>` разметки. Страница без лендинга не получает ни описания из содержимого, ни изображения.
- Более старый релиз, прочитав запись кэша сторфронта, записанную новым, покажет страницу без содержимого до истечения TTL (60 секунд).

**Что запускалось.**

- `rustfmt --check --edition 2024` (rustfmt 1.8.0 из Rust 1.88, не версия проекта) и синтаксический разбор `rustc -Zparse-crate-root-only` по 40 изменённым `.rs`. Разбор прошёл для всех. Форматирование расходится в четырёх файлах админки (`rustok-page-builder/admin/src/{draft_session.rs,lib.rs}`, `rustok-pages/admin/src/{browser_intent.rs,builder.rs}`); они так же расходятся в `main` (`357a961`).
- `node --experimental-strip-types --test packages/rustok-pages/src/pages-frontend.test.mjs` — 16 из 16.
- Поиск изображения в разметке (`first_image_in_markup` и два его помощника): функции скопированы в отдельный файл и собраны `rustc --test` вне крейта — 7 из 7. Сам `seo_targets.rs` не собирался; два его теста, зависящие от типов крейта, не запускались.
- 1307 скриптов `verify-*.mjs` на чистом `357a961` и на рабочем дереве: 706 проходят и 601 падает в обоих случаях, состав совпадает. Нормализованный вывод различается у трёх скриптов, все три проходят: число ADR (178 и 179), число ключей каталога `rustok-pages/storefront` (39 и 38), число `.rs`-файлов (6436 и 6438). 601 скрипт падает уже в `main`, поэтому для них сравнение ничего не говорит о новых изменениях.
- `verify-adrs.mjs` — проходит, 179 решений.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy` — ни для одного крейта: в этой среде нет доступа к crates.io. Весь серверный и Leptos-код этой записи не компилировался. Проверка типов TypeScript после последних правок документации не повторялась (до них `tsc` с заглушками вместо `next` и `@/shared/*` проходил; `.ts`-файлы с тех пор не менялись). Сторфронты в браузере не открывались.

**Не закрыто** на момент этой записи: шесть вопросов (эндпоинт артефакта без читателя, кэш чтений в Next, Draft Mode с одним баннером, разный ответ Leptos и Next на страницу без лендинга, отсутствие интеграционного теста SEO-записи, черновик для обладателя `pages:publish`). Четыре из них решены в следующей записи; два оставшихся перечислены в её конце.

### §7.2, открытые вопросы транспорта — РЕШЕНЫ (2026-11-10); не собиралось, Rust-тесты не запускались

Решения записаны в `DECISIONS/2026-11-10-published-page-landing-transport.md` (пункты 1, 5, 6 и раздел «Alternatives considered»); правило о черновике в `DECISIONS/2026-10-09-page-body-draft-and-revision-history.md` дополнено.

**Что решено и сделано.**

1. **Черновик виден тому, кто может сохранить или опубликовать документ.** Публикация вливает черновик в текущее тело, поэтому обладатель `pages:publish` должен видеть то, что публикует. `can_read_page_draft` удалена, правило одно: `can_read_page_source`.
2. **Неопубликованная страница существует только для авторов.** `can_read_non_public_pages` больше не смотрит на метку роли. Запись неопубликованной страницы получает тот, у кого есть авторское право на страницы: `pages:create`, `pages:update`, `pages:delete` или `pages:publish`. `pages:read` и `pages:list` не подходят: они есть у анонимного читателя и у покупателя. Это закрывает находку про метку роли для Pages: стандартный набор прав покупателя больше не открывает неопубликованные записи.
3. **Вошедший покупатель в GraphQL считается посетителем.** Раньше любой токен обходил проверку канала и привязки модуля к каналу. Теперь обходит только автор. `page(id)` неопубликованной страницы отвечает посетителю `null`, а не ошибкой прав: ошибка подтверждала, что id занят.
4. **Лендинг кэшируется один раз, на сервере.** `GqlPage.landing` читается через кэш Pages с поколениями (`PagesCacheReadRuntime`, TTL 60 секунд), которым уже пользуются Leptos-сторфронт и REST-эндпоинт артефакта. В ключе все три поколения, поэтому запись сбрасывает любое событие страницы, а не только публикация: попадание в кэш пропускает проверку статуса и канала. Отсутствие лендинга не кэшируется. Любой сбой кэша приводит к чтению из источника.
5. **В Next своего кэша страниц нет, и всё, что его обслуживало, удалено.** Удалены маршруты `/api/revalidate`, `/api/draft`, `/api/disable-draft` и `DraftPreviewBanner`. Маршрут инвалидации никто не вызывал, и ни одно чтение не было помечено тегом. Draft Mode не мог показать черновик: у сторфронта нет авторского токена, а рабочий документ не санитизирован для показа. Черновик просматривается в Page Builder.
6. **В Next-админке убрана ссылка «Preview draft».** Она вела на 404 и несла секрет в `href`, то есть в HTML страницы админки. `storefrontUrlForPage` возвращает `null` для неопубликованной страницы и не принимает секрет; `REVALIDATE_SECRET` и `PREVIEW_SECRET` больше не читаются.
7. **`GET /api/pages/{id}/artifact` остаётся.** Это единственное представление опубликованной страницы, которое является цельным документом и отдаётся кэшируемым `GET` (ETag, `304`, `Cache-Control`). Оно нужно для iframe, CDN и внешних клиентов. Сторфронты репозитория его не читают намеренно: они встраивают `landing` в свой документ.

**Что изменилось для пользователей.**

- Токен с одними `pages:read` и `pages:list` (без авторских прав) больше не видит неопубликованные страницы, а в GraphQL на него действуют видимость по каналу и привязка модуля к каналу. Роль «только чтение» для черновиков требует отдельного права; его нет.
- Обладатель `pages:publish` без `pages:update` видит черновик опубликованной страницы.
- Вызов `/api/revalidate`, `/api/draft` и `/api/disable-draft` Next-сторфронта отвечает 404. Развёртывание, которое вызывало `/api/revalidate`, может убрать вызов: он ничего не сбрасывал.
- В Next-админке у неопубликованной страницы в колонке Storefront прочерк вместо ссылки.

**Что запускалось.**

- `rustfmt --check --edition 2024` (rustfmt 1.8.0 из Rust 1.88, не версия проекта) и синтаксический разбор `rustc -Zparse-crate-root-only` по 42 изменённым `.rs`. Разбор проходит для всех. Расхождения форматирования есть в пяти файлах, и все пять расходятся и на чистом `357a961`: четыре admin-файла из прошлой записи и `crates/modules/rustok-pages/src/services/mod.rs`, где изменена одна строка.
- `node --experimental-strip-types --test`: `apps/next-frontend/packages/rustok-pages/src/pages-frontend.test.mjs` — 16 из 16; `apps/next-admin/packages/pages/src/pages-admin.test.mjs` — 9 из 9.
- 1307 скриптов `verify-*.mjs`: 706 проходят и 601 падает, состав тот же, что на чистом `357a961`.
- `verify-adrs.mjs` — проходит, 179 решений.
- Синтаксическая проверка изменённых `.ts` и `.tsx` через `tsc --noResolve` (только ошибки разбора; типы без зависимостей не проверялись) и `tsc --strict` для `urls.ts`.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy` — ни для одного крейта. Серверный код этой записи (правило чтения, разделение автор/посетитель в GraphQL, кэш лендинга и три новых теста в `tests/public_page_landing_graphql_sqlite.rs`) проверен только чтением. `next build` и проверка типов Next-приложений не запускались.

**Не закрыто** (бэклог, пункт 10):

- Метка роли по-прежнему выводится из набора прав. От неё зависят `permission_scope_for_set` в `rustok-core` (токен с меткой «не сотрудник» получает область `Own` для `pages:update` и `pages:delete`) и правило чтения неопубликованного в `rustok-blog`.
- Права на чтение авторского состояния без права записи нет. Редактор админки открывает сотруднику с одним `pages:read` стартовый документ вместо сообщения, что документ недоступен.
- Запись кэша REST-эндпоинта артефакта зависит только от поколения артефакта: смена видимости по каналу доходит до него после TTL.
- REST `GET /api/pages` требует токен и не применяет видимость по каналу.
- Leptos и Next по-разному отвечают на страницу без лендинга (пустое состояние и 404).
- Интеграционного теста SEO-записи опубликованной страницы нет.

### §7.2, три пункта из «не закрыто» — СДЕЛАНЫ (2026-11-10); не собиралось, Rust-тесты не запускались

Закрыты три пункта предыдущей записи: запись кэша REST-эндпоинта артефакта, видимость по каналу в REST `GET /api/pages`, расхождение Leptos и Next на странице без лендинга. Решение `DECISIONS/2026-11-10-published-page-landing-transport.md` дополнено (пункты 3, 5, 6).

**Что сделано.**

1. **Запись кэша документа артефакта сменяется при любом событии страницы.** Раньше ключ записи `GET /api/pages/{id}/artifact` зависел только от поколения артефакта, а обновление страницы (в том числе смена списка каналов) меняет поколения маршрута и страницы. Сервер мог до 60 секунд отдавать документ и `304` на канале, где страница уже скрыта. Теперь вариант ключа строит одна функция `published_landing_cache_variant` (`src/cache_invalidation.rs`) для обоих представлений: GraphQL `landing` и REST-документа. В вариант входят представление, поколения маршрута и страницы, локаль, запасная локаль и канал (без пробелов, в нижнем регистре). Нижестоящий кэш (CDN, браузер) по-прежнему держит документ столько, сколько объявлено в `Cache-Control`.
2. **REST `GET /api/pages` применяет правило посетителя.** Токен без авторского права на страницы получает `404 page_not_found`, если страница не видна на канале запроса или модуль Pages не привязан к этому каналу. Автор находит страницу на любом канале. Это то же правило, что в GraphQL (`can_read_non_public_pages`).
3. **У опубликованной страницы без лендинга нет маршрута на сторфронте.** Решение о маршруте Leptos-хоста (`resolve_storefront_page_route`) отвечает `not_found` для канонического адреса и для его алиасов, как Next отвечает 404. Раньше хост отдавал 200 с заголовком страницы и пустым состоянием. Проверка читает только привязку лендинга (`PageBuilderArtifactService::has_public_bound_artifact_with_fallback`), сам артефакт не загружается. Карточка страницы показывает пустое состояние только там, где её рисуют без решения хоста (клиентская навигация, другой хост).

**Найдено по ходу и исправлено.**

- `storefront/tests/native_storefront_server_fn_sqlite.rs` (собирается только с `--features ssr`) проверял в ответе сторфронта текст из тела страницы (`source-v1`) и поле `"format":"html"`. После перевода транспорта на лендинг тело в ответ не попадает, и первая транспортная правка этот тест пропустила. Тест переписан: содержимое страницы задаёт привязанный лендинг, версии `source-v1`…`source-v4` меняются заменой лендинга.
- Три теста с признаком `ssr` (`host_route_decision_sqlite.rs`, `native_storefront_channel_admission_sqlite.rs`, `native_storefront_server_fn_sqlite.rs`) создавали страницу с документом Page Builder и публиковали её через `publish_non_builder_if_current`. По исходному коду эта команда отклоняет такую страницу с кодом `PAGE_BUILDER_REVIEWED_PUBLISH_REQUIRED` (это же проверяет `tests/page_service_kind_guard.rs`), то есть подготовка данных в этих тестах не проходила и до серии. Запуском это не подтверждено. Теперь страница создаётся без документа, а лендинг привязывает общий помощник `storefront/tests/support/mod.rs`.
- При правке `src/services/page_builder_artifact.rs` была случайно удалена функция `artifact_model`. Это поймал `verify-page-builder-preview-runtime-contract.mjs`; функция восстановлена из `HEAD` без изменений.

**Что изменилось для пользователей.**

- Токен с `pages:read` без авторских прав получает от REST `GET /api/pages` 404 для страницы, скрытой на канале запроса. Раньше получал запись страницы.
- Leptos-сторфронт отвечает 404 на адрес опубликованной страницы без лендинга. Штатная публикация (`publish_reviewed`) лендинг привязывает всегда; без лендинга страницу оставляет только сервисная команда `publish_non_builder`.
- Сервер перестаёт отдавать документ артефакта и `304` из кэша сразу после обновления, снятия с публикации или смены каналов страницы.

**Что запускалось.**

- `rustfmt --check --edition 2024` (rustfmt 1.8.0 из Rust 1.88, не версия проекта) и синтаксический разбор `rustc -Zparse-crate-root-only` по 51 изменённому и новому файлу `.rs`: все разбираются. Форматирование расходится в пяти файлах, во всех пяти оно расходится и на чистом `357a961`.
- 1307 скриптов `verify-*.mjs`: 706 проходят и 601 падает, состав тот же, что на чистом `357a961`.
- `verify-adrs.mjs` и `verify-docs.mjs`.
- `node --experimental-strip-types --test`: Next-сторфронт 16 из 16, Next-админка 9 из 9 (код Next в этой записи не менялся).

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy` — ни для одного крейта. Весь серверный код и все тесты этой записи не проверены компилятором. Нужно запустить: `cargo test -p rustok-pages` (в первую очередь `tests/artifact_http_cache_sqlite.rs`, `tests/page_http_visitor_channel_sqlite.rs`, модульные тесты `src/controllers/mod.rs` и `src/cache_invalidation.rs`) и `cargo test -p rustok-pages-storefront --features ssr` (все пять файлов в `storefront/tests/`).

**Не закрыто** (бэклог, пункт 10):

- Метка роли выводится из набора прав (`rustok-core`, `rustok-blog`).
- Права на чтение авторского состояния без права записи нет; редактор админки открывает сотруднику с одним `pages:read` стартовый документ.
- Интеграционного теста SEO-записи опубликованной страницы нет.
- Команды записи возвращают страницу так, как её может прочитать вызвавший: создавший страницу без права сохранять и публиковать получает ответ без тела.
- Места в Leptos-админке, которые читают `page.body` и считают отсутствие тела пустым документом, не пересмотрены.
- Пакет свидетельств `pages-artifact-http-cache-source.json` описывает эндпоинт артефакта до этой правки; его верификатор не разбирается (`SyntaxError`) и на чистом `main`.

### §7.2, ещё три пункта из «не закрыто» — СДЕЛАНЫ (2026-11-10); не собиралось, Rust-тесты не запускались

Закрыты три пункта предыдущей записи: редактор админки для сотрудника без права на документ, места админки, которые читали отсутствие тела как пустой документ, и интеграционный тест SEO-записи. Пункт «команды записи возвращают страницу так, как её может прочитать вызвавший» закрыт как правило, а не как дефект: оно записано в решении и в README модуля.

**Что сделано.**

1. **Сервер сообщает, скрыт ли документ.** В ответе страницы появилось поле `document_readable` (`documentReadable` в GraphQL). `false` означает, что документ есть или может быть, но вызвавшему он не показан. `true` при пустом `body` означает, что документа в этой локали нет. Раньше оба случая выглядели одинаково: `body: null`. Файлы: `src/dto/page.rs`, `src/services/page/read.rs`, `src/graphql/types.rs`.
2. **Leptos-админка не открывает редактор без документа.** При `documentReadable: false` рабочая область страницы показывает «Document not available» вместо редактора и истории ревизий, а в метаданных вместо формата и ревизии стоит «not available». Запросы билдера к такой странице отвечают `403 DOCUMENT_UNAVAILABLE`, загрузка проекта отвечает `Forbidden`, запрос истории ревизий возвращает ошибку. Стартовый документ больше не подставляется на место скрытого. Файлы: `admin/src/{model,core,browser_intent,browser_problem,builder,composition}.rs`, `admin/src/transport/graphql_adapter.rs`.
3. **Интеграционный тест SEO-записи.** Новый файл `src/seo_target_tests.rs`: страница публикуется штатной командой `publish_reviewed`, запись читается через зарегистрированный провайдер. Пять тестов: описание и картинка берутся из лендинга; неопубликованная правка документа не меняет запись ни для публичного чтения, ни для авторского; написанное вручную мета-описание важнее текста лендинга; посетитель другого канала записи не получает; без привязки лендинга картинки нет, описание равно заголовку. Тест лежит внутри крейта, потому что модуль `seo_targets` закрытый.

**Проверено чтением, правок не потребовало.**

- Остальные чтения `page.body` в `admin/src` (`inline_edit_launch.rs`, `metadata_properties.rs`, `page_locale` в `builder.rs`) берут из тела только локаль как запасное значение. Документ они не подменяют.
- Next-админка тело не читает: `packages/pages/src/api/pages.ts` только отправляет `body: null`, `pages-page.tsx` поля `body` не использует.

**Что изменилось для пользователей.**

- Сотрудник с `pages:read` без права сохранять и публиковать видит в Leptos-админке запись страницы и сообщение о недоступном документе. Раньше ему открывался редактор со стартовым документом, который выглядел как содержимое страницы.
- Клиенты GraphQL и REST получают новое поле `documentReadable` / `document_readable`. Старые поля не менялись.
- Запросы Leptos-админки теперь выбирают `documentReadable`. Админка новой сборки со старым сервером работать не будет: сервер отклонит запрос с неизвестным полем.

**Что запускалось.**

- `rustfmt --check --edition 2024` (rustfmt из Rust 1.88, не версия проекта) и синтаксический разбор `rustc -Zparse-crate-root-only` по всем изменённым и новым файлам `.rs` этой записи. Все разбираются. Форматирование расходится только в `admin/src/browser_intent.rs` и `admin/src/builder.rs`, и расхождение то же, что на чистом `357a961`.
- 1307 скриптов `verify-*.mjs`: 706 проходят и 601 падает, состав тот же, что на чистом `357a961`.
- `verify-adrs.mjs` и `verify-docs.mjs` проходят.
- `node --experimental-strip-types --test`: Next-сторфронт 16 из 16, Next-админка 9 из 9 (код Next в этой записи не менялся).

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy` — ни для одного крейта. Код и тесты этой записи не проверены компилятором. Нужно запустить: `cargo test -p rustok-pages` (в первую очередь `seo_target_tests` и `tests/page_body_draft_and_history_sqlite.rs`) и `cargo test -p rustok-pages-admin`.

**Не закрыто** (бэклог, пункт 10):

- Метка роли выводится из набора прав (`rustok-core`, `rustok-blog`).
- Права на чтение авторского состояния без права записи нет. Роль «рецензент» без него невозможна.
- Тексты админки о недоступном документе написаны по-английски в коде, как и остальная рабочая область страницы; в `admin/locales` их нет.
- Ветка рабочей области админки и отказ загрузки проекта тестом не покрыты; покрыто только отображение ошибки в `403`.
- SEO: картинка, заданная только фоном в CSS лендинга, не находится.
- Пакет свидетельств `pages-artifact-http-cache-source.json` устарел; его верификатор не разбирается (`SyntaxError`) и на чистом `main`.

### Код-ревью §7.2 (2026-11-10): 1 дефект поведения, 1 дефект обработки отказа, 3 неточности документации — ИСПРАВЛЕНЫ; не собиралось, Rust-тесты не запускались

Перечитан дифф серии §7.2 относительно `357a961`: сервер `rustok-pages` (права, чтение страницы, GraphQL, REST, кэш, сервис артефактов, SEO, reviewed publish), оба сторфронта, места Leptos-админки, которые читают документ, новые и переписанные тесты. Дифф `rustok-page-builder/admin/src/draft_session.rs` и `crates/ui/fly` (фиксы 1–11 этого журнала) в этом проходе не перечитывался.

**Найдено и исправлено.**

1. **SEO-редактор терял описание и картинку у страницы с ограничением по каналам.** `rustok-seo` загружает запись в авторском режиме без канала (`targets.rs`, `diagnostics.rs`), а владелец артефакта отдаёт лендинг страницы с ограничением только на одном из её каналов. В редакторе такая страница оставалась без описания из текста лендинга и без картинки. Тест этого не ловил: он вызывал авторский режим с каналом `web`, чего в рабочем коде не бывает. Теперь авторский режим читает лендинг на канале запроса, если он допускает страницу, иначе на первом канале страницы (`landing_channel` в `src/seo_targets.rs`). Лендинг одинаков на всех каналах, которые допускают страницу. Публичный маршрут по-прежнему читает только на канале запроса. Добавлены два модульных теста и интеграционный тест `an_authoring_view_reads_the_landing_of_a_channel_restricted_page`; существующий тест авторского режима теперь идёт без канала.
2. **Сохранение документа, скрытого от пользователя, выглядело как конфликт ревизий.** `save_project` в `admin/src/builder.rs` сравнивал ревизию редактора с ревизией страницы, у которой документ не показан (`page:{id}:initial`), и сообщал о конфликте. Это случается, если право отозвали во время сессии. Теперь ответ — `Forbidden` с тем же текстом, что при загрузке проекта. Сервер запись и раньше не принимал.
3. **Документация утверждала, что метка роли не участвует в чтении документа.** Это неточно: `can_read_page_source` применяет правило записи, а `permission_scope_for_set` сужает `pages:update` пользователя без штатной метки до его собственных страниц. Сама по себе метка документ не открывает и не закрывает. Исправлены комментарий в `src/services/rbac.rs`, ADR (решение 1 и запрещённые состояния), `README.md` модуля и бэклог.
4. **`README.md` модуля сообщал, что интеграционного теста SEO-записи нет.** Он есть с прошлой записи (`src/seo_target_tests.rs`).
5. **ADR описывал старое поведение авторского режима SEO** («страница, не видимая без канала, не имеет описания из содержимого»). Текст приведён к пункту 1.

**Найдено, не исправлялось** (бэклог, пункт 10):

- Поисковая проекция (`src/search_projection.rs`) не читает привязку лендинга. Она берёт сохранённый артефакт локали без сортировки, поэтому после повторной публикации может проиндексировать текст прежнего лендинга, а для локали без артефакта берёт текст тела страницы. Так было и до серии; ADR оставляет проекцию вне своих границ.
- GraphQL-запросы страниц строят контекст безопасности через `from_permission_snapshot`, а REST и мутации — через `security_context_from_access_token`. Различие касается токенов `client_credentials`. Так было и до серии.
- `pageBodyRevisions` требует `pages:update`, а документ читается и с `pages:publish`. Пользователь, который может публиковать, но не сохранять, видит редактор и ошибку в панели истории ревизий. До серии история для него тоже была закрыта.
- Карта сайта читает артефакт целиком на каждую страницу (нужны картинки записи), то есть делает по одному чтению на кандидата.
- `site_symbol_usage` пропускает любое авторское право на страницы, а комментарий говорит о «редакторе».
- Leptos-сторфронт не передаёт `page_id` в разметку лендинга, атрибут `data-page-id` пуст.

**Проверено, правок не потребовало.** Фикстуры новых тестов сверены с сущностями и миграциями (`tenant_modules`, уникальный индекс артефактов, набор полей `AuthContext`, `page`, `page_translation`). Разметка лендинга не содержит `<script>` и `<style>`: политика публикации их запрещает, поэтому поиск первой картинки не встретит их текста. Импортёр стартовых данных читает тело под системным контекстом и получает его. GraphQL и REST одинаково пропускают автора мимо привязки модуля к каналу.

**Что запускалось.**

- Отдельные сборки `rustc --test` (Rust 1.88, без зависимостей проекта) для чистых функций, вынесенных из исходников: разбор разметки SEO 9 из 9, выбор канала лендинга 2 из 2, правила `rbac.rs` 3 из 3, `css_scope.rs` с 26 векторами паритета 9 из 9, варианты ключа кэша 3 из 3. Это не прогон тестов проекта.
- `rustfmt --check --edition 2024` и синтаксический разбор изменённых файлов `.rs`. Форматирование расходится в тех же пяти файлах, что и на чистом `357a961`.
- 1307 скриптов `verify-*.mjs`: 706 проходят и 601 падает, состав тот же, что на чистом `357a961`. `verify-adrs.mjs` и `verify-docs.mjs` проходят.
- `node --test`: Next-сторфронт 16 из 16, Next-админка 9 из 9.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy` — ни для одного крейта. Весь Rust-код серии §7.2, включая правки этой записи, не проверен компилятором.

### §7.2, находки код-ревью — ЗАКРЫТЫ (2026-11-10); не собиралось, Rust-тесты не запускались

Закрыты три находки предыдущей записи и одна новая того же рода (карта сайта).

**Что сделано.**

1. **Поиск индексирует опубликованный лендинг, а не то, что нашлось в таблице.** `src/search_projection.rs` читал любой сохранённый артефакт локали без сортировки и при его отсутствии брал текст тела страницы. Теперь текст документа — это текст лендинга, на который указывает привязка локали страницы (`page_published_landing_artifacts`). Тело страницы проекция больше не читает; функции `extract_page_plain_text` и `collect_text_from_json` удалены.
2. **Страница без опубликованного лендинга не попадает ни в поиск, ни в карту сайта.** У такой страницы нет маршрута на сторфронте, и ссылка из поиска или из `sitemap.xml` вела бы на 404. Проекция не выдаёт документ для страницы без единой привязки; `load_page_sitemap_candidate` в `src/seo_targets.rs` пропускает страницу, для которой чтение в локали по умолчанию не нашло лендинга.
3. **Журнал ревизий читается по правилу чтения документа.** `body_revision_history` требовал `pages:update`, хотя документ читается и с `pages:publish`. Теперь журнал доступен тому же, кому документ (`can_read_page_source`). Восстановление ревизии — это сохранение, оно по-прежнему требует `pages:update`. REST-маршрут `GET /api/admin/pages/{id}/document/history` принимает любое из двух прав.
4. **GraphQL-запросы страниц строят контекст безопасности той же функцией, что REST и мутации** (`security_context_from_access_token`). Токен `client_credentials` теперь везде сервисный участник без владения страницами.
5. Комментарий `site_symbol_usage` приведён к правилу, которое функция применяет.

**Что изменилось для пользователей.**

- Опубликованные страницы без лендинга исчезнут из поиска и из карты сайта после следующего обновления индекса и следующей сборки карты. На сторфронтах они и так отвечают 404.
- Локаль страницы без собственного лендинга индексируется только с заголовками. Раньше в индекс попадал текст её тела.
- Пользователь с `pages:publish` без `pages:update` видит журнал ревизий. Кнопка «Restore» ему показана, но ответит отказом: у админки нет набора прав, чтобы её скрыть.

**Тесты (написаны, не запускались).**

- Новый `tests/search_projection_sqlite.rs`, три теста: в индекс попадает текст привязанного лендинга, а не заменённого и не тела; страница без привязки не индексируется; локаль без своего лендинга индексируется без текста.
- `src/seo_target_tests.rs`: карта сайта содержит страницу с картинкой лендинга и не содержит страницу без лендинга.
- `tests/page_body_draft_and_history_sqlite.rs`: журнал ревизий открыт публикующему и закрыт читателю.

**Проверено чтением.** В `rustok-page-builder/admin/src/draft_session.rs` перечитан файловый стор: раздел по тенантам, блокировка токена, закрытый каталог, проверка владельца, атомарная запись. Новых дефектов не найдено. Гонка при снятии устаревшей блокировки двумя ожидающими описана в коде и была до серии. Остальная часть диффа этого файла и `crates/ui/fly` в этом проходе не перечитывались. Они дочитаны в следующей записи; там же найдены дефекты файлового стора, которых этот проход не увидел.

**Что запускалось.**

- `rustfmt --check --edition 2024` и синтаксический разбор изменённых файлов `.rs`. Форматирование расходится в тех же файлах, что и на чистом `357a961`.
- 1307 скриптов `verify-*.mjs`: 706 проходят и 601 падает, состав тот же, что на чистом `357a961`. `verify-adrs.mjs` и `verify-docs.mjs` проходят.
- `node --test`: Next-сторфронт 16 из 16, Next-админка 9 из 9 (код Next в этой записи не менялся).

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy` — ни для одного крейта. Код и тесты этой записи не проверены компилятором.

**Не закрыто** (бэклог, пункт 10):

- Локаль без своего лендинга остаётся в индексе, даже если ни одна запасная локаль тенанта лендинга не имеет: проекции неизвестна локаль тенанта по умолчанию.
- Карта сайта читает и проверяет артефакт по одному на страницу.
- Кнопка «Restore» показана пользователю без `pages:update`.
- Leptos-сторфронт не передаёт `page_id` в разметку лендинга; атрибут `data-page-id` пуст, его никто не читает. (Закрыто в следующей записи: так было и до серии.)

### Ревью `draft_session.rs` и `crates/ui/fly` — ЗАВЕРШЕНО, три дефекта исправлены (2026-11-10); не собиралось, Rust-тесты не запускались

Дочитаны диффы, оставшиеся от предыдущей записи: `rustok-page-builder/admin/src/draft_session.rs` (весь нетестовый код), `admin/src/lib.rs` и `crates/ui/fly/**`. `lib.rs` — чистый перенос типов в `model.rs`, расхождений с исходным текстом нет.

**Исправлено.**

1. **Сообщение о несохранённой правке пропадало при первом нажатии клавиши.** После отказа в сохранении сервер не повторяет сохранение, пока документ не изменился, и отвечает на такие интенты без `save_failure`. Клиент считал любой успешный ответ концом проблемы и стирал сообщение и его копию в `sessionStorage`. Теперь `fly-browser.js` держит сообщение, пока ответ сообщает `dirty: true`, и убирает его, когда рабочая копия сохранена. Пока сообщение на экране, оно не объявляется повторно; на корне стоит `data-fly-save-failed="true"`. Прежнее ограничение «сообщение показывается один раз» из README снято.
2. **Сбой записи черновика в хранилище ломал сессию до перезапуска процесса.** `DurableSsrDraftSessionStore` сначала фиксировал правку в памяти, потом писал в бэкенд. Если запись падала (диск, таймаут блокировки, недоступный KV), память оставалась на поколение впереди сохранённой записи, и каждая следующая фиксация этой сессии отвечала конфликтом поколений. Теперь при сбое записи запись в памяти сбрасывается, и следующее чтение берёт сохранённое состояние. Дефект был и до серии.
3. **Нечитаемая запись черновика ломала свой токен навсегда и никогда не удалялась.** Файловый стор отвечал ошибкой разбора на каждый запрос с таким токеном, а `prune` такие файлы пропускал. Теперь запись, которая не разбирается и не перезаписывалась целый TTL, считается истёкшей: чтение отвечает «сессии нет», `prune` удаляет файл. Пока TTL не прошёл, ошибка остаётся: без версии формата нельзя отличить повреждённую запись от живой сессии другой сборки.
4. **Истёкшая запись удаляется только под блокировкой токена.** Чтение без блокировки могло прочитать файл прямо перед тем, как фиксация его заменила, и удалить уже новую запись. Теперь чтение и `prune` перед удалением берут блокировку токена и перечитывают файл. Если блокировка занята дольше двух секунд, файл остаётся до следующего чтения.

**Находка G закрыта без правки.** Пустой `data-page-id` у опубликованного лендинга — не регрессия: на исходном коммите сторфронт тоже не передавал идентификатор в `StaticLandingStorefront`. Атрибут на лендинге никто не читает; браузерный тест Next-админки читает его у корня редактора, а не у лендинга.

**Тесты.**

- Выполнено: изолированный сценарий на Node (в репозиторий не включён) исполняет вспомогательную часть `fly-browser.js` на подставном DOM, 8 случаев проходят: сбой сохраняется и показывается, держится при `dirty: true` без повторного объявления, снимается при сохранённой копии, возвращается после другой проблемы, устаревает через 10 минут.
- Выполнено: строки контрактного теста `public_bundle_surfaces_save_failures_across_the_reload` сверены с бандлом тем же правилом (без пробелов), все 10 найдены. Сам тест через `cargo` не запускался.
- Написано, не запускалось (`draft_session.rs`): `durable_commit_that_fails_to_persist_leaves_the_session_usable`, `durable_create_that_fails_to_persist_leaves_no_session_behind`, `file_store_deletes_an_expired_record_only_under_the_token_lock`, `file_store_treats_an_unreadable_record_as_expired_after_one_ttl`, `file_store_prune_collects_unreadable_records_after_one_ttl`.

**Что запускалось.** `node --check` бандла; `rustfmt --check --edition 2024` и синтаксический разбор изменённых `.rs` (расхождения форматирования только в прежних местах); 1307 скриптов `verify-*.mjs` — 706 проходят и 601 падает, состав как на чистом `357a961`; `verify-adrs.mjs` и `verify-docs.mjs` проходят.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy`, Playwright-тесты адаптера (`apps/next-admin/tests/e2e/fly-browser-*.spec.ts`).

**Не закрыто.**

- `prune` по-прежнему никто не вызывает по расписанию. Файл истёкшей сессии, которую больше не читают, остаётся на диске; общий лимит `maximum_entries` не применяется.
- У записи черновика нет версии формата. Нечитаемая запись ломает свой токен на срок до одного TTL.
- `enforce_owner_cap` и `prune` разбирают каждую запись раздела целиком, чтобы узнать владельца и срок.
- GraphQL-контекст безопасности для токена `client_credentials` не покрыт тестом на уровне запросов страниц. *(Закрыто в следующей записи.)*

### Хвосты ревью — закрыты пять пунктов, один из них — найденный при ревью дефект KV-бэкенда (2026-11-10); не собиралось, Rust-тесты не запускались

**Исправлено.**

1. **KV-бэкенд черновиков мог удалить только что зафиксированную правку.** `ExternalKvDraftSessionBackend` при чтении истёкшей записи и в `prune` удалял ключ безусловно. Если между чтением и удалением другой под успевал зафиксировать новое поколение, удалялась уже новая запись, хотя редактору было отвечено «сохранено». Это тот же дефект, что был исправлен в файловом сторе. В `DraftSessionKvDriver` добавлен обязательный метод `compare_and_delete(key, expected_cas)`: ключ удаляется, только пока в нём то поколение, которое было прочитано. Чтение при промахе перечитывает ключ один раз и возвращает живую запись; `prune` такую запись пропускает и не считает. Нечитаемая KV-запись по-прежнему отвечает `Serialization` и не удаляется бэкендом: её снимает TTL драйвера.
   Следствие для хостов: драйвер, написанный под прежний трейт, должен добавить метод. В репозитории драйверов два, оба обновлены: `MemoryKvDriver` и тестовый `RacingKvDriver`.
2. **«Restore» в истории ревизий показывается только при праве `pages:update`.** Раньше кнопку видел каждый, кто читает историю, включая держателя одного `pages:publish`, и получал отказ при нажатии. Запись в backlog утверждала, что у админки нет снимка прав; он есть (`AuthUser.permissions`), по нему же строится политика редактора. Решение вынесено в `access::pages_can_restore_body_revision`.
3. **Тексты «документ недоступен» переведены.** Заголовок, пояснение и значения «not available» в метаданных берутся из `admin/locales` (`pages-document-*`, en и ru). Текст ошибки `403` остаётся английским, как остальные тексты ошибок эндпоинта.
4. **Отказ `load_project` / `save_project` для скрытого документа покрыт тестом.** Обе операции теперь проходят через одну функцию `ensure_document_readable`; тесты проверяют отказ `Forbidden`, пропуск страницы без документа и то, что токен ревизии скрытого и пустого документа одинаков (поэтому проверка стоит до сравнения ревизий).
5. **Тест запроса страниц для токена `client_credentials`.** `a_service_token_never_owns_a_page_by_its_subject_id` в `tests/public_page_landing_graphql_sqlite.rs`: автор страницы равен субъекту токена; пользователь с `pages:update` получает документ, сервисный токен с тем же идентификатором и теми же правами — нет (`documentReadable: false`, лендинг на месте), сервисный токен с `pages:manage` — получает.

**Проверено чтением, правок не потребовалось.**

- Вложенность ответа: `settleSaveFailure` читает `dirty` из `result.result`. Это совпадает с сервером: тело ответа — `PagesBrowserIntentResponse`, поле `result` — `BrowserIntentDispatchResult` с `dirty`; адаптер передаёт в событие тело ответа целиком.
- Диффы `rustok-page-builder/storefront/src/lib.rs`, `src/static_landing.rs`, `src/static_publish_policy.rs`. Первый — замена прежнего скоупинга на модуль `css_scope` во всех четырёх поверхностях; второй — общая политика рендера для публикации и превью с тестами; третий — только комментарии, их утверждения сверены с кодом (`validate_static_publish_document` не принимает политику, `verify_integrity` сверяет хэш с политикой по умолчанию).
- Пункт 1 раздела «Замечено и оставлено» (durable-стор продлевает срок записи только фиксацией) подтверждён: `load` идёт через `load_record`, а TTL продлевает только `FileSsrDraftSessionStore::load`.
- `FlakyBackend` реализует все пять методов `PersistentDraftSessionBackend`; `file_is_older_than`, `UNATTRIBUTED_PARTITION`, `InMemorySsrDraftSessionStore::{is_empty, generation}` существуют; среди прежних тестов нет ожидающих удаление истёкшего файла без блокировки.
- Пункт 10 backlog целиком добавлен этой серией; из остального файла изменены три строки про транспорт лендинга.

**Тесты.**

- Выполнено в изолированном стенде (в репозиторий не включён; `rustc --test`, без `cargo`): текст `ExternalKvDraftSessionBackend`, `MemoryKvDriver` и новых тестов взят из `draft_session.rs` дословно, `serde_json`, контроллер и `tracing` заменены заглушками. 5 тестов проходят. С безусловным удалением вместо условного три из них падают, то есть тесты различают старое и новое поведение.
- Выполнено там же: `pages_can_restore_body_revision` с его тестом и `kv_driver_compare_and_set_is_atomic` — проходят.
- Написано, не запускалось: `kv_driver_compare_and_delete_requires_the_stored_token`, `kv_backend_drops_an_expired_record_on_read`, `kv_backend_read_keeps_a_commit_that_replaced_an_expired_record`, `kv_backend_reports_an_unparsable_record_without_deleting_it`, `kv_prune_keeps_a_session_committed_during_the_scan` (`draft_session.rs`); `restoring_a_body_revision_needs_the_save_grant` (`admin/src/access.rs`); три теста `document_gate_tests` (`admin/src/builder.rs`, под `ssr`); `a_service_token_never_owns_a_page_by_its_subject_id`.

**Что запускалось.** Синтаксический разбор всех изменённых `.rs` и `rustfmt --check --edition 2024` (расхождения только в прежних местах); 1307 скриптов `verify-*.mjs` — 706 проходят и 601 падает, состав как на чистом `357a961`; `verify-adrs.mjs` и `verify-docs.mjs` проходят.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy`. Правки Leptos-компонента (`BodyRevisionHistory`, `PageWorkspace`) проверены только разбором синтаксиса.

**Не закрыто.**

- Ветка `PageWorkspace` с уведомлением и история без «Restore» не имеют теста отрисовки: в крейте нет стенда для компонентов.
- Остальные строки `PageWorkspace` (заголовки, история ревизий, расписание публикации) остаются английскими литералами.
- Файловый стор при чтении на границе срока отвечает «сессии нет», даже если под блокировкой оказалось, что запись только что заменена; KV-бэкенд в этом случае перечитывает. Различие оставлено. *(Закрыто в следующей записи.)*
- `prune` по расписанию, версия формата записи черновика — без изменений.

### Проверка TypeScript обоих Next-приложений и граница срока в файловом сторе (2026-11-10); Rust не собирался

**Исправлено.**

1. **Файловый стор черновиков: чтение на границе срока возвращает запись, которую успела записать фиксация.** Фиксация берёт блокировку токена, читает запись до истечения срока и записывает новое поколение уже после него. Чтение без блокировки в этом промежутке видело истёкшую запись, под блокировкой убеждалось, что файл заменён, не удаляло его, но отвечало «сессии нет». Редактор, открывший страницу в этот момент, не получал свой черновик. Теперь `remove_dead_record` возвращает исход (`DeadRecord::{Removed, Replaced, Left}`), и при `Replaced` `read_located` читает файл ещё раз, один раз. Поведение совпало с KV-бэкендом. `prune` считает удалённым только `Removed`, как раньше. Тест: `file_store_read_returns_the_record_a_commit_wrote_while_the_read_waited`.

**Проверено, правок не потребовалось.**

- **Типы TypeScript.** Зависимости обоих приложений установлены (`npm ci --ignore-scripts`), `next typegen` выполнен. `tsc --noEmit` с закоммиченным `src/types/i18n.d.ts` проходит без ошибок в `apps/next-frontend` и в `apps/next-admin`; изменённые серией файлы входят в проверяемую программу (сверено по `tsc --listFiles`). Удаление `api/artifact.ts`, `draft-preview-banner.tsx` и трёх маршрутов `src/app/api/*` не оставило ссылок.
- **Скрипт `npm run typecheck` падает в обоих приложениях не из-за этой серии.** Он сначала пересоздаёт `src/types/i18n.d.ts` из `messages/en.ftl`, и результат отличается от закоммиченного файла (в сторфронте 1279 добавленных и 421 удалённая строка). С пересозданным файлом: сторфронт — 3 ошибки (`rustok-blog`, `rustok-pricing`), админка — 10 ошибок (`packages/blog`). Пример причины: `Pricing-totalProducts = Total products: {  }` с пустой подстановкой не даёт ключа. Ни сообщения, ни эти файлы серия не меняла. Пересозданные файлы в рабочее дерево не включены.
- **Скрипты `verify-*.mjs`.** 601 падающий скрипт печатает на рабочем дереве тот же текст, что на чистом `357a961` (сравнение полного вывода, а не только кодов возврата). 75 из них не разбираются (`SyntaxError`) в обоих деревьях: файлы закоммичены обрезанными.

**Решение по пакету свидетельств `pages-artifact-http-cache-source.json`: не переписан.** Пакет датирован 2026-08-04 и описывает первый тест стенда; этот тест серия не меняла, и утверждения пакета о нём остаются верными. Поля `production_cache_behavior_changed: false` и `public_transport_changed: false` относятся к августовскому изменению, а не к этой серии, которая ключ кэша артефакта меняла. Верификатор пакета `verify-pages-artifact-http-cache.mjs` не разбирается, закоммичен таким (`712bae99`), полной копии в истории нет, поэтому неизвестно, что именно он закрепляет в пакете. Восстановление верификатора и перевыпуск пакета со вторым тестом записаны в backlog.

**Найдено вне серии, записано в backlog.** Шаг «Verify cache-consumer source boundaries» в `.github/workflows/pages-cache-consumer-exact-main-evidence.yml` запускает девять верификаторов под `set -euo pipefail`; на чистом `357a961` восемь из них не разбираются, девятый падает на отсутствии `fn register_event_listeners(`. В закоммиченном виде шаг пройти не может.

**Тесты.**

- Выполнено: `npm run test:pages` в `apps/next-frontend` — 16 из 16; `npm run test:pages` в `apps/next-admin` — 9 из 9; `npm run test:seo` (7) и `npm test` в сторфронте проходят.
- Выполнено на модели (в репозиторий не включена; `rustc`, только `std`): порядок действий `read_located` / `remove_dead_record` воспроизведён с упрощённым форматом записи. С повторным чтением гонка возвращает поколение 2; без него — «сессии нет» при живой записи; блокировка, отпущенная без фиксации, по-прежнему приводит к удалению файла. Это проверка логики, а не кода крейта.
- Написано, не запускалось: `file_store_read_returns_the_record_a_commit_wrote_while_the_read_waited` (ждёт около 0,3 с намеренно).

**Что запускалось ещё.** Синтаксический разбор `draft_session.rs` и `rustfmt --check --edition 2024` (расхождения только в прежних местах файла).

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy`. Playwright: в среде нет браузера, а узел загрузки Chromium недоступен; `tests/` исключён из `tsconfig.json` админки, поэтому спецификации `fly-browser-*.spec.ts` не проверены и по типам. *(Закрыто в следующей записи.)* ESLint изменённые файлы не покрывает: конфигурация сторфронта задаёт разбор только для `packages/rustok-blog`, в админке для этих файлов нет подходящей конфигурации.

### Адаптер `fly-browser.js` исполнен в браузере; новый e2e-тест отказа сохранения (2026-11-10); Rust не собирался

**Что стало возможно.** В среде не было браузера. Chromium 149 взят из npm-пакета `@sparticuz/chromium` (каталог вне репозитория), Playwright 1.64.1 запущен с ним через отдельную конфигурацию, тоже вне репозитория. Playwright этой версии рассчитан на Chromium 156; расхождение версий на результатах не сказалось, но прогон в CI со штатным браузером остаётся нужным. Вместо dev-сервера Next на порту 3200 стояла заглушка, отдающая пустую страницу: спецификациям адаптера от хоста нужен только источник (origin).

**Добавлено.**

- `apps/next-admin/tests/e2e/fly-browser-save-failure.spec.ts`, три теста:
  1. ответ `200` с `save_failure` и `reload: true` приводит к настоящей перезагрузке страницы; запись в `sessionStorage` её переживает; адаптер нового документа сообщает об отказе при монтировании; ответ с `dirty: true` не снимает сообщение и не объявляет его повторно; ответ с `dirty: false` снимает сообщение и удаляет запись;
  2. отказ, вытесненный другой проблемой (`403`), возвращается при следующем ответе с `dirty: true`;
  3. запись старше десяти минут при монтировании не показывается и удаляется.
- `tests/public_page_landing_graphql_sqlite.rs`: автор не получает неопубликованную страницу через `pageBySlug` (по `id` получает). Правило записано в комментарии к `get_by_slug_with_locale_fallback` и в README модуля: чтение по slug — поиск маршрута витрины, только опубликованные страницы, для любого вызывающего.
- `tests/search_projection_sqlite.rs`: `a_binding_to_a_landing_of_another_page_locale_is_not_indexed` — привязка, указывающая на артефакт другой локали страницы, ничего не публикует в поиск. Это условие `binding_matches_artifact`; оно совпадает с условием владельца артефакта (`load_bound_artifact_in_tx` фильтрует по странице и локали). **Исправлено позже:** этот тест не мог пройти — такую привязку отвергает схема; см. запись «Часть Rust-кода серии собрана `cargo`…» ниже, там он заменён.

**Проверено, правок не потребовалось.**

- Все 21 прежний тест `fly-browser-*.spec.ts` проходят на изменённом адаптере.
- Тест `a_service_token_never_owns_a_page_by_its_subject_id`: у `pages.author_id` нет внешнего ключа (миграция `m20260328_000001`), случайный идентификатор автора допустим; конструкция теста (замыкание, возвращающее `async move` с `TestResult::Ok(…)`) собрана и выполнена отдельно на `rustc`, edition 2024.
- `css_scope.rs` (Leptos-сторфронт): стенд от прошлого прохода содержит код файла без изменений (отличие — векторы паритета вписаны в текст вместо чтения JSON через `serde`); 9 тестов модуля проходят, 26 векторов совпадают по составу с `landing-css-scope-parity.json`.
- `build_hash` двух лендингов с разными заголовками различается: в него входит хэш исходного документа (`landing_contract.rs`). Первый тест `search_projection_sqlite.rs` не упирается в уникальный индекс артефактов.
- `rustok_core::Error` реализует `Display` (`thiserror`), `projection_error` в тесте поиска корректен.

**Найдено, записано в backlog.** `PagesFlyBuilder` при открытии страницы восстанавливает черновик через `.load(…).ok().flatten()`: ошибка хранилища (ввод-вывод, тайм-аут блокировки, неразбираемая запись, чужой владелец) открывает редактор на сохранённом документе без несохранённых правок, без сообщения и без записи в журнале. Об ошибке сообщает только первое действие редактора. Не исправлено: показ сообщения требует правки Leptos-компонента, а у крейта нет зависимости `tracing`; без сборки это делать не стал.

**Тесты.**

- Выполнено в Chromium 149: 24 теста `fly-browser-*.spec.ts` (21 прежний и 3 новых) — проходят.
- Выполнено там же, проверка чувствительности новых тестов: с адаптером чистого `357a961` падают все три; если любой успешный ответ снимает отказ — падают 1-й и 2-й; если отказ объявляется при каждом ответе — падает 1-й. После проверки адаптер восстановлен, контрольная сумма совпала.
- Выполнено: `tsc --noEmit` по всем `fly-browser-*.spec.ts` (отдельная конфигурация с типами Node, так как `tests/` исключён из `tsconfig.json`) — без ошибок; новый файл отформатирован `prettier`.
- Написано, не запускалось: добавленное утверждение в `an_unpublished_page_does_not_exist_for_visitors` и `a_binding_to_a_landing_of_another_page_locale_is_not_indexed`. (Второй из них оказался нерабочим и заменён, см. ту же запись ниже.)

**Что запускалось ещё.** Синтаксический разбор изменённых `.rs` и `rustfmt --check --edition 2024`; 1307 скриптов `verify-*.mjs` (состав как на чистом `357a961`); `verify-adrs.mjs`, `verify-docs.mjs`.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy`. `smoke.spec.ts` админки: ему нужен работающий Next. Rust-тест `public_bundle_surfaces_save_failures_across_the_reload` (`fly-browser`).

### Изоляция CSS лендинга проверена браузером; векторы паритета расширены (2026-11-10); Rust не собирался

**Зачем.** Тесты пакета проверяют результат `scopeLandingCss` собственным сканером: судья написан теми же руками, что и проверяемый код. Независимого судьи не было. Теперь им служит разбор CSS в Chromium 149 (как он получен — см. предыдущую запись).

**Добавлено.**

- `apps/next-frontend/tests/e2e/landing-css-scope.spec.ts`, четыре теста. Стили оборачиваются `scopeLandingCss`, вставляются текстом в `<style>` так же, как это делает сторфронт, и читаются обратно через объектную модель CSS:
  1. все общие векторы паритета, для обоих корней (`.fly-page-container` в Next, `[data-fly-page-body]` в Leptos): в таблице стилей ровно одно правило, это `@scope` с нужным корнем; элемент `<style>` один; в документ не вышло ни разметки, ни текста; элемент хоста вне лендинга сохраняет свой стиль;
  2. то же для 24 несбалансированных и враждебных таблиц (лишние `}`, незакрытые блоки, строки, комментарии и `url(`, `</style>` в строке и вне её, вложенный `@scope`, `@import`);
  3. правила `body`, `html` и `:root` стилизуют контейнер лендинга, а не документ хоста; `header` внутри лендинга стилизуется, `header` хоста — нет;
  4. строка CSS, содержащая `</style><img …>`, остаётся строкой: вычисленное `content` равно исходному тексту.
- `crates/modules/rustok-page-builder/contracts/landing-css-scope-parity.json`: 13 новых векторов (стало 39) — случаи из браузерного теста, которых не было в контракте (несколько `}` подряд, `}` после вложенного `@scope`, `}` между групповыми правилами, `{` и `}` внутри `url()`, экранированный `}` в селекторе, `}` в комментарии, экранированная кавычка, перевод строки в незакрытой строке, `}` внутри открытой скобки, `</style` вне строки и с пробелом перед `>`, `@import`). Прежние 26 векторов не изменены (сверено побайтно).

**Проверено.**

- Выполнено в Chromium 149: 4 теста проходят. Чувствительность судьи: если обернуть те же 24 таблицы без сканирования, 9 из них выходят наружу (скрывают `header` хоста или вставляют разметку в документ).
- Выполнено: файл `css_scope.rs` из репозитория собран `rustc` как модуль отдельной программы (без `cargo`; тесты модуля в сборку не входят). Его результат побайтно совпадает с результатом `scopeLandingCss` на 25 входах браузерного теста и с ожидаемым текстом всех 39 векторов, для обоих корней. Значит, вывод браузера относится и к тексту, который выдаёт Leptos-сторфронт.
- Выполнено: `npm run test:pages` в сторфронте (16 из 16, читает расширенный контракт); `tsc --noEmit` приложения (новый файл входит в программу) — без ошибок.
- Не запускалось: тест модуля `matches_the_vectors_shared_with_the_next_storefront` через `cargo test` (он читает тот же JSON).

**Измерено для открытого решения в backlog** (Chromium 149): `@keyframes`, `@font-face` и `@property` внутри `@scope` принимаются, их имена глобальны — элемент хоста может исполнить анимацию лендинга и видит начальное значение зарегистрированного свойства; `@import` внутри `@scope` отбрасывается. Компилятор публикации (`crates/ui/fly/src/render.rs`) таких правил не выдаёт: только правило токенов темы, правила по style-hook и классам с отфильтрованными объявлениями и `@media`. Поэтому опубликованный лендинг их содержать не может; вопрос касается CSS из другого источника.

**Уточнено в backlog.** Запись про английские литералы `PageWorkspace` занижала объём: Leptos-админка страниц обращается всего к семи ключам локализации, остальные 75 из 82 ключей в `admin/locales` не использует ни один исходный файл; литералами отрисованы `CreatePageCard`, `PagesNavigator`, `PageWorkspace`, `BodyRevisionHistory`, `PublishSchedulePanel`. Не исправлено: это правка разметки пяти компонентов, без сборки я её делать не стал.

**Что запускалось ещё.** 1307 скриптов `verify-*.mjs` (состав как на чистом `357a961`); `verify-adrs.mjs`, `verify-docs.mjs`.

**Что не запускалось.** `cargo check`, `cargo test`, `cargo clippy`. Браузерная проверка сделана в одном браузере; Safari и Firefox не проверялись.

### Часть Rust-кода серии собрана `cargo` и исполнена; найден и заменён нерабочий тест (2026-11-10)

**Как собрано.** У окружения нет доступа к crates.io, но есть к GitHub. Зависимости взяты клонированием их репозиториев по тегам версий из `Cargo.lock`, а где тега нет — с последнего коммита (57 репозиториев, 91 пакет; расхождения версий перечислены ниже) и подставлены через `[patch.crates-io]`; сборка — `cargo --offline`. Вне репозитория собрано зеркало: дерево символических ссылок на файлы репозитория с собственным корневым `Cargo.toml`, в котором участники — только то, что удалось обеспечить зависимостями. Исходники и манифесты крейтов — сами файлы репозитория, не копии. В репозиторий из этого ничего не добавлено.

Эта среда заменяет описанную выше в «Среда проверки для фиксов 8–11»: там `sha2`, `uuid` и `rustok-ui-i18n` были заглушками, а property-тесты `fly` вырезались. Теперь эти три зависимости настоящие и `proptest` есть.

Чем сборка отличается от штатной:

- компилятор `rustc 1.88.0-dev`, тогда как workspace требует 1.96 (сборка с `--ignore-rust-version`); в клоне `uuid 1.29.0` включена одна нестабильная возможность компилятора, без неё он на 1.88 не собирается;
- 12 пакетов взяты на патч- или минорную версию ниже или выше, чем в `Cargo.lock` (`cfg-if 1.0.5` вместо `1.0.4`; утилиты ICU из тега `icu@2.2.0`: `icu_locale_core` и `icu_provider` 2.2.0 вместо 2.3.0, `zerovec`, `yoke`, `tinystr`, `writeable`, `litemap`, `zerotrie`, `zerofrom`, `potential_utf`, `zerovec-derive`);
- `uuid` без `v5`/`v7`/`js`, `proptest` без `fork`/`timeout`/`bit-set`;
- пустыми заглушками заменены `criterion`, `tokio`, `async-trait`, `sea-orm-migration`, `wasm-bindgen`, `js-sys`, `r-efi` и три внутренних крейта, нужных только возможности `server` (`rustok-api`, `rustok-core`, `rustok-telemetry`); ни одна собранная цель их кодом не пользуется;
- `rustok-page-builder` собран без возможности `server`;
- `rustdoc` и `clippy` в окружении нет: doc-тесты и `clippy` не запускались.

**Собрано и исполнено целиком (настоящие крейты, `cargo test --lib --tests`).**

| Крейт | Тесты |
|---|---|
| `fly` | 311 |
| `fly-ui` | 80 |
| `fly-browser` | 16 модульных и 21 в десяти интеграционных целях |
| `rustok-page-builder` без `server` | 127 модульных и 3 в `tests/static_publish_policy.rs` |
| `rustok-ui-i18n` (в серии не менялся) | 20 модульных и 121 в 22 интеграционных целях |
| `rustok-ui-core` (в серии не менялся) | 47 |

**Собрано и исполнено частично.** Четыре крейта зависят от `leptos`, которого здесь нет. Для каждого сделан крейт-зеркало, чей `lib.rs` подключает файлы репозитория по пути (`#[path]`) — только те, что не импортируют `leptos`:

| Крейт | Подключённые файлы | Тесты |
|---|---|---|
| `rustok-page-builder-admin` | `model.rs`, `asset_provider.rs`, `draft_session.rs`, `consumer_properties.rs`, `core/session.rs`, `provider_status.rs` | 74, из них 55 в `draft_session` |
| `rustok-page-builder-storefront` | `core.rs`, `css_scope.rs`, `i18n.rs`, `contract_tests.rs` | 18 |
| `rustok-pages-storefront` | `core.rs`, `i18n.rs`, `model.rs` | 9 |
| `rustok-pages-admin` | `access.rs`, `contributions.rs`, `core.rs`, `i18n.rs`, `model.rs`, `starters.rs`; выполнен настоящий `build.rs` | 30 |

Оговорки к зеркалам: константа `FLY_STOREFRONT_BRIDGE_JS` скопирована из `lib.rs` сторфронта конструктора при генерации зеркала (её читает `contract_tests.rs`); набор реэкспортов в зеркалах свой. Отдельно сверено по тексту, что все 48 имён, которые настоящий `lib.rs` админки конструктора реэкспортирует из подключённых файлов, в них объявлены.

Во всех перечисленных файлах репозитория компилятор не выдал ни одного предупреждения, кроме одного, вызванного самим зеркалом (`pub use … page_starters` в `core.rs` админки страниц: его потребитель — `composition.rs`, который в зеркало не входит).

**Что впервые исполнено.** Ниже — тесты, которые в предыдущих записях значились как «написано, не запускалось» либо исполнялись только в прежней среде с заглушками. Все проходят.

- `draft_session.rs` (55 тестов), среди них добавленные после прежнего прогона: `durable_commit_that_fails_to_persist_leaves_the_session_usable`, `durable_create_that_fails_to_persist_leaves_no_session_behind`, `file_store_deletes_an_expired_record_only_under_the_token_lock`, `file_store_treats_an_unreadable_record_as_expired_after_one_ttl`, `file_store_prune_collects_unreadable_records_after_one_ttl`, `file_store_read_returns_the_record_a_commit_wrote_while_the_read_waited` и пять тестов `kv_*`.
- `public_bundle_surfaces_save_failures_across_the_reload` (`fly-browser`) — на итоговом тексте адаптера.
- `matches_the_vectors_shared_with_the_next_storefront` (39 векторов) и `test_every_surface_scopes_project_css_to_the_landing_body` (сторфронт конструктора) — впервые через `cargo test`.
- `restoring_a_body_revision_needs_the_save_grant` (`rustok-pages/admin/src/access.rs`).
- Property-тесты `fly` (`proptest`) — раньше вырезались.

**Проверка чувствительности тестов `draft_session`.** Пять правок-мутаций, каждая в копии файла вне репозитория, каждая возвращает один из исправленных дефектов. Все пять обнаружены:

| Мутация | Упавшие тесты |
|---|---|
| файловый стор не перечитывает запись после взятия блокировки | `file_store_read_returns_the_record_a_commit_wrote_while_the_read_waited` |
| KV-чтение не повторяется после проигранного `compare_and_delete` | `kv_backend_read_keeps_a_commit_that_replaced_an_expired_record` |
| KV-чтение удаляет ключ безусловно | тот же |
| неудачная запись в бэкенд не откатывает память | `durable_commit_that_fails_to_persist_leaves_the_session_usable`, `durable_create_that_fails_to_persist_leaves_no_session_behind` |
| чтение без блокировки удаляет истёкшую запись само | `file_store_deletes_an_expired_record_only_under_the_token_lock` и тест перечитывания |

**Найден дефект в тесте этой серии.** `a_binding_to_a_landing_of_another_page_locale_is_not_indexed` (`tests/search_projection_sqlite.rs`) не мог пройти. Он вставлял артефакт локали `ru` для страницы, у которой есть только перевод `en`, и привязывал его к телу `en`. Комментарий в тесте утверждал, что схема связывает привязку с артефактом только по идентификатору. Это верно для миграции `m20260718_000002`, но `m20260721_000004` добавляет составной внешний ключ `(tenant_id, page_id, locale, artifact_id)` в PostgreSQL и триггеры в SQLite: артефакт локали без перевода и привязка к артефакту другой локали отвергаются. Тест упал бы уже на вставке артефакта.

Как установлено: десять триггеров SQLite взяты текстом из файла миграции и исполнены модулем `sqlite3` Python над таблицами, написанными от руки (только столбцы, которые читают триггеры). Старая последовательность вставок отвергнута на артефакте (`page artifact locale has no matching translation`); привязка тела `en` к артефакту `ru` отвергнута (`published artifact locale binding mismatch`); привязка к артефакту `en` принята.

Исправление:

- тест заменён на `a_binding_names_a_landing_of_its_own_page_locale`: у страницы переводы `en` и `ru`, хранятся лендинги обеих локалей; привязка тела `en` к лендингу `ru` должна быть отвергнута схемой, страница при этом не индексируется; та же привязка к лендингу `en` сохраняется, индексируется текст лендинга `en`, документ `ru` текста лендинга не несёт;
- комментарий к `binding_matches_artifact` (`src/search_projection.rs`) теперь говорит, что условие гарантирует схема, а проверка в проекции — вторая линия; поведение кода не менялось.
- сама проверка в проекции при применённых миграциях недостижима, и интеграционным тестом её больше не покрыть. Добавлен модульный тест `binding_matches_only_an_artifact_of_its_own_page_locale` (`src/search_projection.rs`): артефакт той же страницы и локали подходит, другой локали или другой страницы — нет.

Проверка обоих тестов (сами они в составе крейта не собирались и не запускались):

- интеграционный: его последовательность вставок пройдена по тем же триггерам в `sqlite3` — обе локали и оба лендинга вставляются, привязка `en` → `ru` отвергается и строки не оставляет, привязка `en` → `en` принимается, перевод существующей привязки на лендинг `ru` (`UPDATE`) отвергается;
- модульный: текст функции, теста и обеих структур `Model` (поля взяты из файлов сущностей) собран отдельной программой и исполнен — проходит; если убрать из функции сравнение локали или сравнение страницы, падает. Подменены только типы `sea-orm` и `chrono` (`Json`, `DateTimeWithTimeZone`, `Utc::now()`), так что имена и типы остальных полей проверены компилятором, а вызов `chrono::Utc::now().into()` — нет.

Остальные прямые вставки строк в тестах, которые серия добавила или изменила, сверены с теми же триггерами и уникальными индексами по тексту: три остальных теста `search_projection_sqlite.rs`, `page_http_visitor_channel_sqlite.rs`, `artifact_http_cache_sqlite.rs`, `page_body_draft_and_history_sqlite.rs`, `storefront/tests/support/mod.rs` и оба его потребителя (`host_route_decision_sqlite.rs`, `native_storefront_server_fn_sqlite.rs`). Везде перевод локали вставляется раньше тела и лендинга, а привязка называет лендинг своей локали; нарушений нет.

**Фикстуры лендингов исполнены настоящим компилятором.** Проекты из `search_projection_sqlite.rs`, `seo_target_tests.rs` и `storefront/tests/support/mod.rs` (взяты текстом из файлов) пропущены через `StaticLandingCompiler::compile_publish` и `verify_integrity`: компилируются; два разных заголовка дают разные `build_hash` (значит, два артефакта одной локали не упрутся в уникальный индекс); заголовок, маркер и внешний адрес картинки попадают в `body_html`; `strip_html_tags` оставляет заголовок без разметки.

**Не собрано, как и раньше.** Серия меняет или добавляет 64 файла `.rs`; компилятор видел 16 из них (8 в `fly`, `fly-ui`, `fly-browser` и `rustok-page-builder` и 8 из таблицы зеркал). Остальные 48:

- 23 файла серверного крейта `rustok-pages` (`src/**`, `tests/**`) и `apps/server`. В замыкании `rustok-pages` по `Cargo.lock` 586 внешних пакетов (`sea-orm`, `axum`, `async-graphql`, `tokio`); клонировать их по одному нецелесообразно. Здесь лежит основной объём правок серии, и оба теста, написанных в этой записи, тоже здесь.
- 25 файлов четырёх крейтов админки и сторфронта, не вошедших в зеркала: они импортируют `leptos` (291 пакет в замыкании) либо модули, которые его импортируют, либо это интеграционные тесты сторфронта страниц (`storefront/tests/**`, им нужны `sea-orm` и `tokio`). Среди них `lib.rs` админки конструктора; `lib.rs`, `inline_edit.rs`, `localized_route.rs` сторфронта конструктора; `builder.rs`, `composition.rs`, `browser_intent.rs`, `browser_problem.rs`, `metadata_properties.rs`, `standalone_metadata.rs` админки страниц; `lib.rs`, `builder.rs`, `ui/leptos.rs`, `transport/**` сторфронта страниц.

**Что запускалось ещё.** `rustfmt --check --edition 2024` двух изменённых файлов `.rs`; 1307 скриптов `verify-*.mjs` (состав как на чистом `357a961`); `verify-adrs.mjs`, `verify-docs.mjs`.

### Не реализовано в этой серии

- **§8.1** (перенос Leptos-слоя в `core/` + `ui/`), **§8.2** (переименование capability `Publish` → `Save`) — структурные изменения уровня major. Не начаты.
- **§8.3** (`PortContext.tenant_id` содержит slug): тип общий, из `rustok-api`; переименование затрагивает все модули и здесь не компилируется. Не начато.
- **§8.4** (Tailwind-палитра в `next-admin/packages/pages`). Не начато.
- **§8.5** — действий не требует.

Эти пункты и остальная незавершённая работа серии записаны в `docs/modules/page-builder-backlog.md`, пункт 10.

Очередь §10 закрыта: пункты 1–11 реализованы, пункт 12 закрыт фиксами 3–6. Степень проверки у пунктов разная — см. «Верификация» в каждой записи; фикс 8 не исполнялся вовсе.
