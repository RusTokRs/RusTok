#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

TARGETS=(docs apps crates)

echo "[1/4] Проверка конфликтующих формулировок transport..."
CONFLICTS=$(rg -n --no-heading -S "(replace GraphQL|удал(ить|яем) GraphQL|only /api/fn|только /api/fn)" "${TARGETS[@]}" || true)
if [[ -n "$CONFLICTS" ]]; then
  echo "Найдены потенциально конфликтующие формулировки:" >&2
  echo "$CONFLICTS" >&2
  exit 1
fi

echo "[2/4] Проверка, что план содержит обязательные execution-разделы..."
PLAN="docs/research/dioxus-ffa-ui-migration-plan.md"
rg -n "Принцип исполнения backlog" "$PLAN" >/dev/null
rg -n "Сверка с текущим кодом" "$PLAN" >/dev/null
rg -n "Phase-gate" "$PLAN" >/dev/null
rg -n "KPI parity" "$PLAN" >/dev/null
rg -n "RACI" "$PLAN" >/dev/null

echo "[2b/4] Проверка anti-over-extraction стандарта FFA-срезов..."
rg -n "Стандарт минимального FFA-среза и anti-over-extraction" "$PLAN" >/dev/null
rg -n "FFA-срез должен уменьшать связность" "$PLAN" >/dev/null
rg -n "request/command construction, normalization и validation" "$PLAN" >/dev/null
rg -n "простые i18n label bindings" "$PLAN" >/dev/null
rg -n "reset/refresh side effects после mutation" "$PLAN" >/dev/null
rg -n "механические wrappers над одной строкой форматирования" "$PLAN" >/dev/null
rg -n "Если изменение добавляет больше boilerplate, чем удаляет coupling" "$PLAN" >/dev/null
rg -n "если обнаружен over-extraction, откатить его" "$PLAN" >/dev/null

echo "[3/4] Поиск Leptos-зависимостей внутри core-слоя (core.rs и core/)..."
CORE_HITS=$(rg -n --no-heading -S "use .*leptos|leptos::|leptos_router|leptos_ui_routing|#\[component|#\[server|IntoView|ReadSignal|WriteSignal|Resource<" crates --glob "**/core/**" --glob "**/core.rs" || true)
if [[ -n "$CORE_HITS" ]]; then
  echo "Найдены Leptos-зависимости в core-слое:" >&2
  echo "$CORE_HITS" >&2
  exit 1
fi

echo "[4/4] Проверка наличия ссылки на план в docs/index.md..."
rg -n "dioxus-ffa-ui-migration-plan" docs/index.md >/dev/null

echo "OK: FFA UI doc/pattern checks passed"
