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

echo "[3/4] Поиск Leptos-зависимостей внутри core-слоя (core.rs и core/)..."
CORE_HITS=$(rg -n --no-heading -S "leptos|leptos_router|leptos_ui_routing" crates --glob "**/core/**" --glob "**/core.rs" || true)
if [[ -n "$CORE_HITS" ]]; then
  echo "Найдены Leptos-зависимости в core-слое:" >&2
  echo "$CORE_HITS" >&2
  exit 1
fi

echo "[4/4] Проверка наличия ссылки на план в docs/index.md..."
rg -n "dioxus-ffa-ui-migration-plan" docs/index.md >/dev/null

echo "OK: FFA UI doc/pattern checks passed"
