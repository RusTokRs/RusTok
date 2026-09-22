#!/usr/bin/env bash
# RusTok — Anti-bypass audit
# Периодический аудит дублирования доменной логики и обхода crate API
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
BOLD='\033[1m'

WARNINGS=0
ERRORS=0

header() { echo -e "\n${BOLD}=== $1 ===${NC}"; }
pass()   { echo -e "  ${GREEN}✓${NC} $1"; }
warn()   { echo -e "  ${YELLOW}!${NC} $1"; WARNINGS=$((WARNINGS + 1)); }
fail()   { echo -e "  ${RED}✗${NC} $1"; ERRORS=$((ERRORS + 1)); }

RUN_MANUAL_REVIEW=0
STRICT=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --manual-review)
            RUN_MANUAL_REVIEW=1
            shift
            ;;
        --strict)
            STRICT=1
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [--manual-review] [--strict]"
            echo "  --manual-review  print расширенный список кандидатов для ревью"
            echo "  --strict         считать найденные кандидаты ошибками (exit > 0)"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 2
            ;;
    esac
done

print_hits() {
    local title="$1"
    local pattern="$2"
    shift 2
    local paths=("$@")

    header "$title"
    local hits
    hits=$(rg -n --glob '*.rs' "$pattern" "${paths[@]}" || true)

    if [[ -z "$hits" ]]; then
        pass "Кандидаты не найдены"
        return
    fi

    local count
    count=$(echo "$hits" | wc -l)
    warn "Найдено $count candidate(s) — требуется ручной review"
    echo "$hits" | head -40

    if [[ $RUN_MANUAL_REVIEW -eq 1 && $count -gt 40 ]]; then
        echo "  … (показаны первые 40; используйте локальный запуск для полного списка)"
    fi

    if [[ $STRICT -eq 1 ]]; then
        fail "strict-mode: найденные кандидаты считаются нарушением"
    fi
}

print_hits \
    "1. Дублирование валидации доменных правил в app/frontend-adapter" \
    '\bvalidate\(|\.validate\(\)|ValidationErrors|validator::Validate' \
    apps/server/src apps/admin/src apps/next-admin apps/next-frontend

print_hits \
    "2. Ручная публикация событий вместо модульного сервиса" \
    '\.publish\(' \
    apps/server/src

print_hits \
    "3. Прямые запросы к таблицам домена мимо crate API" \
    'Entity::find|Entity::find_by_id|from_raw_sql|DatabaseConnection|Statement::from_sql_and_values' \
    apps/server/src

header "4. Ключевые сигнатуры orchestration-only (контрольные ориентиры)"
service_calls=$(rg -n --glob '*.rs' 'Service::|service\.' apps/server/src apps/admin/src apps/next-admin apps/next-frontend || true)
if [[ -n "$service_calls" ]]; then
    pass "Обнаружены orchestration-вызовы сервисов"
    echo "$service_calls" | head -20
else
    warn "Не найдены явные сервисные вызовы: проверьте, не сползла ли логика в transport-layer"
fi

header "5. Политика разбора кандидатов (manual-review rule)"
pass "Не каждый кандидат = нарушение: часть orchestration и infra допустима в apps/server + rustok-core"
pass "Frontend-duplication задачи выносить в самописные frontend-библиотеки, а не в domain crates"

header "Итог"
echo -e "  warnings: $WARNINGS"
echo -e "  errors:   $ERRORS"

if [[ $ERRORS -gt 0 ]]; then
    exit $ERRORS
fi

exit 0
