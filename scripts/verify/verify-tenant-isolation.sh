#!/usr/bin/env bash
# RusTok — Верификация tenant isolation
# Фаза 19.1: поиск SQL-запросов без tenant_id фильтрации
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
cd "$ROOT_DIR"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'
BOLD='\033[1m'

ERRORS=0
WARNINGS=0

header() { echo -e "\n${BOLD}=== $1 ===${NC}"; }
pass()   { echo -e "  ${GREEN}✓${NC} $1"; }
fail()   { echo -e "  ${RED}✗${NC} $1"; ERRORS=$((ERRORS + 1)); }
warn()   { echo -e "  ${YELLOW}!${NC} $1"; WARNINGS=$((WARNINGS + 1)); }

DOMAIN_CRATES=(
    "crates/rustok-content/src"
    "crates/rustok-commerce/src"
    "crates/rustok-blog/src"
    "crates/rustok-forum/src"
    "crates/rustok-pages/src"
    "crates/rustok-tenant/src"
)

EXISTING_CRATES=()
for crate in "${DOMAIN_CRATES[@]}"; do
    [[ -d "$crate" ]] && EXISTING_CRATES+=("$crate")
done

if [[ ${#EXISTING_CRATES[@]} -eq 0 ]]; then
    echo -e "${YELLOW}No domain crates found. Skipping.${NC}"
    exit 0
fi

# ─── 1. find_all() без tenant_id filter ───
header "1. Поиск .all(&db) без tenant_id filter"

results=$(grep -rn '\.all(&' "${EXISTING_CRATES[@]}" --include="*.rs" 2>/dev/null || true)
if [[ -n "$results" ]]; then
    while IFS= read -r line; do
        file=$(echo "$line" | cut -d: -f1)
        lineno=$(echo "$line" | cut -d: -f2)
        # Check if there's a tenant_id filter nearby (within 5 lines above)
        context=$(sed -n "$((lineno > 5 ? lineno - 5 : 1)),${lineno}p" "$file" 2>/dev/null || true)
        if echo "$context" | grep -qi "tenant_id"; then
            pass "$file:$lineno — .all() with tenant_id filter"
        else
            fail "$file:$lineno — .all() WITHOUT tenant_id filter"
            echo "      $(echo "$line" | cut -d: -f3-)"
        fi
    done <<< "$results"
else
    pass "No .all(&db) calls found (or all filtered)"
fi

# ─── 2. find_by_id без tenant_id ───
header "2. Поиск find_by_id без tenant_id проверки"

results=$(grep -rn 'find_by_id\|find_by_pk' "${EXISTING_CRATES[@]}" --include="*.rs" 2>/dev/null | grep -v "test" | grep -v "// " || true)
if [[ -n "$results" ]]; then
    while IFS= read -r line; do
        file=$(echo "$line" | cut -d: -f1)
        lineno=$(echo "$line" | cut -d: -f2)
        # Check surrounding context for tenant_id
        context=$(sed -n "$((lineno > 3 ? lineno - 3 : 1)),$((lineno + 5))p" "$file" 2>/dev/null || true)
        if echo "$context" | grep -qi "tenant_id"; then
            pass "$file:$lineno — find_by_id with tenant_id check"
        else
            warn "$file:$lineno — find_by_id possibly without tenant_id check (manual review)"
            echo "      $(echo "$line" | cut -d: -f3-)"
        fi
    done <<< "$results"
else
    pass "No find_by_id calls found"
fi

# ─── 3. DELETE без tenant_id ───
header "3. Поиск DELETE без tenant_id filter"

results=$(grep -rn '\.delete\|::delete_many\|::delete_by_id\|delete_many_by_id' "${EXISTING_CRATES[@]}" --include="*.rs" 2>/dev/null | grep -v "test" | grep -v "// " || true)
if [[ -n "$results" ]]; then
    while IFS= read -r line; do
        file=$(echo "$line" | cut -d: -f1)
        lineno=$(echo "$line" | cut -d: -f2)
        context=$(sed -n "$((lineno > 5 ? lineno - 5 : 1)),$((lineno + 3))p" "$file" 2>/dev/null || true)
        if echo "$context" | grep -qi "tenant_id"; then
            pass "$file:$lineno — delete with tenant_id filter"
        else
            warn "$file:$lineno — delete possibly without tenant_id filter (manual review)"
            echo "      $(echo "$line" | cut -d: -f3-)"
        fi
    done <<< "$results"
else
    pass "No delete operations found"
fi

# ─── 4. Миграции: каждая domain-таблица имеет tenant_id column ───
header "4. Миграции: tenant_id column в domain таблицах"

MIGRATION_DIR="crates/rustok-migrations/src"
if [[ -d "$MIGRATION_DIR" ]]; then
    # Find create_table calls and check for tenant_id
    migration_files=$(find "$MIGRATION_DIR" -name "*.rs" -not -name "lib.rs" | sort)
    for mig in $migration_files; do
        tables=$(grep -oP 'create_table\s*\(\s*Table::create\(\)\s*\.table\(\s*\w+::Table\s*\)' "$mig" 2>/dev/null || true)
        if [[ -n "$tables" ]]; then
            basename_mig=$(basename "$mig")
            if grep -q "tenant_id\|TenantId" "$mig" 2>/dev/null; then
                pass "$basename_mig — has tenant_id"
            else
                # Some tables (tenants, roles, permissions, sessions) don't need tenant_id
                if echo "$basename_mig" | grep -qiE "tenant|role|permission|session|metadata"; then
                    pass "$basename_mig — no tenant_id (expected for system table)"
                else
                    warn "$basename_mig — no tenant_id found (verify if needed)"
                fi
            fi
        fi
    done
else
    warn "Migration directory not found: $MIGRATION_DIR"
fi

# ─── 5. SeaORM entities: tenant_id field ───
header "5. SeaORM entities: tenant_id поле"

ENTITY_DIRS=(
    "apps/server/src/models/_entities"
    "apps/server/src/models"
)

for edir in "${ENTITY_DIRS[@]}"; do
    if [[ -d "$edir" ]]; then
        entity_files=$(find "$edir" -name "*.rs" -not -name "mod.rs" -not -name "prelude.rs" 2>/dev/null | sort)
        for efile in $entity_files; do
            basename_e=$(basename "$efile" .rs)
            # Skip system entities
            if echo "$basename_e" | grep -qiE "^_|tenant|role|permission|session|prelude|mod"; then
                continue
            fi
            if grep -q "pub tenant_id" "$efile" 2>/dev/null; then
                pass "$basename_e entity — has tenant_id field"
            elif grep -q "pub struct Model" "$efile" 2>/dev/null; then
                warn "$basename_e entity — no tenant_id field (verify if needed)"
            fi
        done
        break
    fi
done

# ─── 6. Raw SQL string concatenation (SQL injection risk) ───
header "6. Raw SQL string concatenation (SQL injection risk)"

sql_concat=$(grep -rn 'format!.*SELECT\|format!.*INSERT\|format!.*UPDATE\|format!.*DELETE\|format!.*WHERE' "${EXISTING_CRATES[@]}" "apps/server/src" --include="*.rs" 2>/dev/null | grep -v "test\|// \|///\|migration" || true)
if [[ -n "$sql_concat" ]]; then
    count=$(echo "$sql_concat" | wc -l)
    fail "$count raw SQL concatenation(s) found (use parameterized queries):"
    echo "$sql_concat" | head -10
else
    pass "No raw SQL string concatenation"
fi

# ─── 7. Hard DELETE without soft-delete pattern ───
header "7. Hard DELETE without soft-delete/archive"

hard_delete=$(grep -rn '\.delete(\|delete_by_id\|delete_many\|DELETE FROM' "${EXISTING_CRATES[@]}" --include="*.rs" 2>/dev/null | grep -v "test\|// \|migration\|soft_delete\|archive\|status" || true)
if [[ -n "$hard_delete" ]]; then
    count=$(echo "$hard_delete" | wc -l)
    warn "$count hard DELETE operation(s) — consider soft-delete (status = Archived):"
    echo "$hard_delete" | head -10
else
    pass "No hard DELETE operations (or using soft-delete)"
fi

# ─── Summary ───
echo ""
echo -e "${BOLD}━━━ Tenant Isolation Summary ━━━${NC}"
if [[ $ERRORS -eq 0 && $WARNINGS -eq 0 ]]; then
    echo -e "${GREEN}All checks passed!${NC}"
elif [[ $ERRORS -eq 0 ]]; then
    echo -e "${YELLOW}$WARNINGS warning(s) — manual review recommended${NC}"
else
    echo -e "${RED}$ERRORS error(s), $WARNINGS warning(s)${NC}"
fi
exit $ERRORS
