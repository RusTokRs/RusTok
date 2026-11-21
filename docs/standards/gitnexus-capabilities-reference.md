# Справочник возможностей и архитектурная книга рецептов GitNexus

Этот документ представляет собой исчерпывающий технический справочник по всем возможностям **GitNexus** (Knowledge Graph & Program Dependence Engine) и практическое руководство по его использованию для глубокого статического анализа, поиска антипаттернов, проверки инвариантов и рефакторинга.

---

## 1. Архитектура и принципы работы

GitNexus преобразует кодовую базу в обогащенный свойствами граф (**Code Knowledge Graph**), размещенный во встроенной графовой базе данных **LadybugDB / Kùzu** (буферный пул до 2048 MiB, нативная интеграция C++ через Node ABI).

```mermaid
flowchart TD
    subgraph Ingestion ["1. Входные данные"]
        Code["Исходный код (Rust, TS/JS, Python)"]
        AsyncAPI["AsyncAPI 3.x Спецификации"]
        Diff["Git Diff & Worktree State"]
    end

    subgraph Engine ["2. Движок GitNexus (LadybugDB / Kùzu)"]
        AST["AST & Symbol Layer"]
        CallGraph["Call & Dependency Graph"]
        PDG["PDG: CDG + REACHING_DEF"]
        Taint["Taint Layer: Sources -> Sinks"]
        ProcessGraph["Process & Community Layer"]
        ContractGraph["API & Tool Contract Layer"]
    end

    subgraph Interfaces ["3. Интерфейсы взаимодействия"]
        CLI["CLI: analyze, check, impact, detect-changes"]
        MCP["16 MCP Tools (Agent Context)"]
        WebUI["Web UI: http://127.0.0.1:4747 (serve)"]
        Cypher["Cypher Query Engine"]
    end

    Ingestion --> Engine
    Engine --> Interfaces
```

### Уровни графа (Graph Layers)

1. **AST & Символьный слой (Syntax Layer):**
   * Узлы: файлы (`File`), папки (`Folder`), функции (`Function`), структуры (`Struct`), перечисления (`Enum`), трейты (`Trait`), реализации (`Impl`), модули (`Module`), методы (`Method`), классы (`Class`), интерфейсы (`Interface`).
   * Рёбра: `DEFINES`, `CONTAINS`, `MEMBER_OF`, `HAS_METHOD`, `HAS_PROPERTY`, `EXTENDS`, `IMPLEMENTS`.
2. **Слой вызовов и зависимостей (Call & Dependency Graph):**
   * Рёбра: `CALLS`, `IMPORTS`, `ACCESSES`, `QUERIES`, `INJECTS`, `WRAPS`.
3. **Слой процессов и архитектурных сообществ (Community & Process Layer):**
   * Узлы: `Community` (кластеры связанного кода, вычисленные алгоритмом Louvain с оценкой cohesion score), `Process` (сквозные цепочки сценариев выполнения).
   * Рёбра: `STEP_IN_PROCESS`, `ENTRY_POINT_OF`.
4. **Слой API и контрактов (Contract & Client-Server Layer):**
   * Узлы: `Route` (HTTP/GraphQL эндпоинты), `Tool` (MCP/RPC функции), `Destination` (топики и очереди брокеров сообщений из AsyncAPI).
   * Рёбра: `HANDLES_ROUTE`, `FETCHES`, `HANDLES_TOOL`, `CONTRACT_LINK`.
5. **Слой потоков управления и данных (PDG Layer, флаг `--pdg`):**
   * Узлы: `BasicBlock`.
   * Рёбра: `CFG` (поток управления), `CDG` (зависимость управления / guard-клаузы), `REACHING_DEF` (определения и использования переменных).
6. **Слой безопасности (Taint Analysis Layer, флаг `--pdg`):**
   * Рёбра: `TAINTED` (внутрипроцедурное заражение), `TAINT_PATH` (межпроцедурные переходы данных), `SANITIZES` (барьеры санитизации).

---

## 2. Полный справочник CLI команд и флагов

Запуск осуществляется через проектный раннер: `node .gitnexus/run.cjs <command>` (или глобальный `gitnexus`).

### 2.1. Индексация и обновление (`analyze`)

```bash
node .gitnexus/run.cjs analyze [options] [path]
```

| Флаг | Назначение |
|---|---|
| `--pdg` | Построить слои Program Dependence Graph (CDG, REACHING_DEF, Taint). **Критично для поиска логических дыр, обходов проверок и инъекций.** |
| `--asyncapi-spec <path>` | Прочитать спеки AsyncAPI 3.x (файл или директория) и построить узлы `Destination` для очередей и топиков. |
| `--embeddings` | Включить локальную генерацию векторных эмбеддингов для семантического Graph-RAG поиска. |
| `--drop-embeddings` | Удалить существующие векторные индексы при переиндексации. |
| `--watch` | Запустить фоновый демон слежения за изменениями файлов с автообновлением графа. |
| `--debounce <ms>` | Задержка в миллисекундах перед переиндексацией в режиме watch (по умолчанию 300ms). |
| `--force` | Принудительная полная переиндексация с нуля, игнорируя кэш. |
| `--spring-actuator <path>` | Импорт метаданных Spring Actuator (для Java/Spring сервисов). |

### 2.2. Проверка инвариантов (`check`)

```bash
node .gitnexus/run.cjs check [options]
```

| Флаг | Назначение |
|---|---|
| `--cycles` | Обнаружить циклические импорты и зависимости между файлами и модулями. Завершается с ошибкой при их наличии. |
| `--json` | Вывод результата в формате JSON для CI/CD гейтов и скриптов верификации. |
| `-r, --repo <name>` | Имя целевого репозитория (при мультирепозиторной конфигурации). |

### 2.3. Визуальный Web UI Explorer (`serve`)

```bash
node .gitnexus/run.cjs serve --port 4747 --host 127.0.0.1
```
* Поднимает локальный веб-сервер на `http://127.0.0.1:4747`.
* Предоставляет интерактивный визуальный граф: кластеры, карты роутов, цепочки процессов, радиальные диаграммы связности и фильтры по типам связей.

### 2.4. Диагностика среды и движка (`doctor`)

```bash
node .gitnexus/run.cjs doctor
```
* Проверяет статус рантайма Node.js, ABI модулей, размер пула страниц LadybugDB, наличие нативных библиотек `lbugjs.node`, движка полнотекстового поиска (FTS) и расширений эмбеддингов.

### 2.5. Анализ изменений и воздействий (`detect-changes` / `impact`)

* `node .gitnexus/run.cjs detect-changes --scope all --repo .` — сопоставляет текущий незакоммиченный `git diff` с графом, определяя затронутые процессы и оценивая уровень риска (`LOW`, `MEDIUM`, `HIGH`, `CRITICAL`).
* `node .gitnexus/run.cjs impact <symbol> --direction upstream --repo .` — расчет радиуса поражения символа на глубину до 3 уровней.

### 2.6. Генерация архитектурной документации (`wiki`)

```bash
node .gitnexus/run.cjs wiki [options]
```

| Флаг | Назначение |
|---|---|
| `--provider <name>` | LLM-провайдер: `minimax`, `openai`, `openrouter`, `azure`, `claude`, `cursor`, `codex`, `opencode`. |
| `--model <name>` | Название модели (по умолчанию `MiniMax-M3`). |
| `--lang <lang>` | Язык документации (например, `russian`, `english`). |
| `--force` | Принудительная регенерация всей вики-документации из графа. |
| `--concurrency <n>` | Количество параллельных запросов к LLM (по умолчанию 3). |

### 2.7. Мультирепозиторные группы (`group`)

* `node .gitnexus/run.cjs group list` — список сконфигурированных групп репозиториев.
* `node .gitnexus/run.cjs group sync` — перестроить реестр контрактов (`Contract Registry`) между связанными репозиториями.

---

## 3. Справочник 16 MCP-инструментов

Все инструменты доступны AI-агенту через протокол MCP:

| Инструмент | Параметры | Что возвращает |
|---|---|---|
| `query` | `search_query`, `repo` | Семантический/концептуальный поиск, сгруппированный по бизнес-процессам (`Process`). |
| `context` | `name`, `repo` | 360-градусный обзор символа: входящие/исходящие вызовы, методы, процессы. |
| `impact` | `target`, `direction`, `maxDepth`, `minConfidence` | Радиус поражения при изменении символа (глубины $d=1, 2, 3$). |
| `trace` | `from`, `to`, `maxDepth`, `includeTests` | Кратчайший направленный путь вызовов между двумя символами. Сообщает точное место разрыва при `no_path`. |
| `detect_changes` | `scope`, `worktree` | Анализ влияния текущего git diff на кодовую базу (с поддержкой `worktree`). |
| `rename` | `symbol_name`, `new_name`, `dry_run` | Координированное безопасное переименование во всех файлах с разделением правок по уверенности (graph vs text). |
| `cypher` | `statement`, `repo`, `limit` | Выполнение произвольного запроса на языке Cypher к базе LadybugDB. |
| `explain` | `target`, `limit` | Отчёт о цепочках заражения (Taint findings): источник $\to$ путь $\to$ опасный сток (SQLi, XSS, Path Traversal). |
| `pdg_query` | `mode` (`controls` / `flows`), `target`, `variable` | CDG (условия выполнения и гарды) или REACHING_DEF (поток значений переменной). |
| `check` | `cycles: true` | Проверка структурных инвариантов графа на циклы. |
| `route_map` | `repo` | Карта API маршрутов: клиентские компоненты $\leftrightarrow$ обработчики эндпоинтов. |
| `shape_check` | `repo` | Поиск расхождений (`MISMATCH`) между ключами ответа сервера и ключами, читаемыми клиентом. |
| `api_impact` | `route`, `method` | Оценка риска и список потребителей перед изменением API маршрута. |
| `tool_map` | `repo` | Карта определений MCP/RPC инструментов и файлов-обработчиков. |
| `group_list` | — | Список сконфигурированных мультирепозиторных групп. |
| `group_sync` | — | Синхронизация реестра контрактов между репозиториями группы. |

---

## 4. Переменные окружения и управление хранилищем

| Переменная | Назначение |
|---|---|
| `GITNEXUS_STORAGE_PATH` | Точный абсолютный путь к внешней директории индекса (переопределяет локальный `.gitnexus/`). |
| `GITNEXUS_STORAGE_ROOT` | Корневой каталог для хранения индексов изолированных слотов репозиториев (`<repo>-<hex>/`). |
| `GITNEXUS_CONTENT_RETENTION` | Политика хранения содержимого файлов: `full` (весь текст), `symbol` (фрагменты символов), `none` (только топологический граф). |
| `GITNEXUS_SKIP_FTS` | Отключение индекса полнотекстового поиска (ускоряет анализ на слабых машинах). |

---

## 5. Кулинарная книга Cypher (Рецепты для поиска косяков)

### Рецепт 1: Поиск циклических зависимостей любой глубины
```cypher
MATCH path = (a:File)-[:CodeRelation {type: 'IMPORTS'}*2..6]->(a:File)
RETURN [n IN nodes(path) | n.filePath] AS cycle
LIMIT 20
```

### Рецепт 2: Нарушение слоистой архитектуры (Domain $\to$ Infrastructure)
```cypher
MATCH (domain:File)-[:CodeRelation {type: 'CALLS'|'IMPORTS'}]->(infra:File)
WHERE domain.filePath CONTAINS '/domain/' 
  AND (infra.filePath CONTAINS '/infrastructure/' OR infra.filePath CONTAINS '/database/')
RETURN domain.filePath AS violator, infra.filePath AS forbidden_target
```

### Рецепт 3: Нарушение изоляции БД (`P-OWNER-READ-BOUNDARY`)
Поиск функций модуля, выполняющих SQL-запросы к чужим таблицам:
```cypher
MATCH (fn:Function)-[:CodeRelation {type: 'QUERIES'}]->(table:CodeElement)
WHERE fn.filePath CONTAINS 'crates/modules/rustok-distribution/'
  AND table.name IN ['products', 'product_variants', 'channels']
RETURN fn.name, fn.filePath, table.name
```

### Рецепт 4: Утечка данных мультитенантности (Запросы без скоупа `tenant_id`)
Поиск запросов к базе данных, где в цепочке аргументов или условий отсутствует обращение к `TenantContext` или `tenant_id`:
```cypher
MATCH (fn:Function)-[:CodeRelation {type: 'QUERIES'}]->(entity:CodeElement)
WHERE fn.filePath CONTAINS 'crates/modules/'
  AND NOT EXISTS {
    MATCH (fn)-[:CodeRelation {type: 'ACCESSES'|'CALLS'}*1..2]->(tenant_arg:CodeElement)
    WHERE tenant_arg.name CONTAINS 'tenant' OR tenant_arg.name CONTAINS 'Tenant'
  }
RETURN fn.name, fn.filePath, entity.name
LIMIT 30
```

### Рецепт 5: Бесхозные события (Orphaned Events в Outbox / Iggy)
Поиск событий домена, которые никем не публикуются либо никем не слушаются:
```cypher
// 1. События без подписчиков (никто не слушает)
MATCH (ev:Struct)
WHERE ev.filePath CONTAINS '/contracts/' AND ev.name ENDS WITH 'Event'
  AND NOT (()-[:CodeRelation {type: 'CALLS'|'STEP_IN_PROCESS'}]->(ev))
RETURN ev.name AS unhandled_event, ev.filePath;

// 2. События без издателей (объявлены, но никогда не генерируются)
MATCH (ev:Struct)
WHERE ev.filePath CONTAINS '/contracts/' AND ev.name ENDS WITH 'Event'
  AND NOT EXISTS {
    MATCH (publisher:Function)-[:CodeRelation {type: 'CALLS'}]->(outbox:Function)
    WHERE outbox.name CONTAINS 'publish' AND publisher.filePath CONTAINS 'crates/modules/'
  }
RETURN ev.name AS unpublished_event, ev.filePath;
```

### Рецепт 6: Мёртвый код (Функции без входящих вызовов)
```cypher
MATCH (f:Function)
WHERE NOT (()-[:CodeRelation {type: 'CALLS'}]->(f))
  AND NOT f.filePath CONTAINS '/tests/'
  AND NOT f.filePath CONTAINS '/migrations/'
  AND NOT f.name IN ['main', 'new', 'default', 'run', 'init']
RETURN f.name, f.filePath
ORDER BY f.filePath
LIMIT 50
```

### Рецепт 7: Раздутые узлы (God Functions с аномальным Fan-out)
```cypher
MATCH (f:Function)-[:CodeRelation {type: 'CALLS'}]->(callee)
WITH f, count(DISTINCT callee) AS out_degree
WHERE out_degree > 25
RETURN f.name, f.filePath, out_degree
ORDER BY out_degree DESC
```

### Рецепт 8: Утечка инкапсуляции (Прямой доступ к полям структуры минуя методы)
```cypher
MATCH (caller:Function)-[:CodeRelation {type: 'ACCESSES'}]->(prop:CodeElement)
WHERE caller.filePath CONTAINS 'crates/modules/rustok-cart/'
  AND prop.filePath CONTAINS 'crates/modules/rustok-order/'
RETURN caller.name, caller.filePath, prop.name
```

---

## 6. Стратегии улучшения плана и поиск новых векторов аудита

1. **Регулярный кластерный скрининг:** через ресурс `gitnexus://repo/{name}/clusters` отслеживать сообщества кода со снижающимся cohesion score ($< 0.45$).
2. **Анализ временной связности (Temporal Coupling):** сопоставление Git-логов и графа позволяет находить куски кода, которые коммитятся вместе, но не имеют формальных зависимостей (признак логического дублирования).
3. **Автоматический запуск перед слиянием PR:** проверка `detect-changes` в CI для пресечения непредвиденного расширения радиуса поражения.
