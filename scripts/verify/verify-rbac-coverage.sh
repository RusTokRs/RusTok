#!/usr/bin/env bash
# RusTok — Верификация RBAC coverage
# Фаза 19.2: каждый handler/resolver имеет RBAC check
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

CONTROLLERS_DIR="apps/server/src/controllers"
GRAPHQL_DIR="apps/server/src/graphql"

# ─── 1. REST handlers: RBAC extractors ───
header "1. REST handlers: проверка RBAC extractors"

if [[ -d "$CONTROLLERS_DIR" ]]; then
    # Find all pub async fn (handlers)
    handler_files=$(find "$CONTROLLERS_DIR" -name "*.rs" | sort)
    total_handlers=0
    unprotected_handlers=0

    for file in $handler_files; do
        basename_f=$(basename "$file" .rs)
        # Skip graphql, swagger, metrics, health — they have their own auth model
        if echo "$basename_f" | grep -qiE "^graphql$|^swagger$|^metrics$|^mod$"; then
            continue
        fi

        # Find handler functions
        handlers=$(grep -n 'pub async fn\|pub fn' "$file" 2>/dev/null | grep -v "//\|#\[" || true)
        if [[ -z "$handlers" ]]; then
            continue
        fi

        echo -e "  ${BOLD}$file${NC}"

        while IFS= read -r line; do
            lineno=$(echo "$line" | cut -d: -f1)
            fn_name=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")
            total_handlers=$((total_handlers + 1))

            # Check function signature for RBAC extractors (Require*, Permission, etc.)
            # Look at function params (next 5 lines)
            fn_context=$(sed -n "${lineno},$((lineno + 8))p" "$file" 2>/dev/null || true)

            if echo "$fn_context" | grep -qiE "Require|Permission|SuperAdmin|AdminUser|auth::"; then
                pass "$fn_name — has RBAC extractor"
            elif echo "$basename_f" | grep -qiE "^health$|^auth$"; then
                pass "$fn_name — public/auth endpoint (OK without RBAC)"
            elif echo "$fn_name" | grep -qiE "^health|^ping|^status|^login|^register|^refresh|^reset"; then
                pass "$fn_name — public endpoint (OK without RBAC)"
            else
                warn "$fn_name (line $lineno) — no RBAC extractor found"
                unprotected_handlers=$((unprotected_handlers + 1))
            fi
        done <<< "$handlers"
    done

    echo ""
    echo -e "  Total handlers: $total_handlers, Unprotected: $unprotected_handlers"
else
    warn "Controllers directory not found: $CONTROLLERS_DIR"
fi

# ─── 2. GraphQL mutations: permission checks ───
header "2. GraphQL mutations: проверка permission checks"

if [[ -d "$GRAPHQL_DIR" ]]; then
    # Find all mutation functions
    mutation_files=$(find "$GRAPHQL_DIR" -name "*.rs" | sort)
    total_mutations=0
    unprotected_mutations=0

    for file in $mutation_files; do
        mutations=$(grep -n 'async fn.*mutation\|async fn create_\|async fn update_\|async fn delete_\|async fn publish_\|async fn execute_\|async fn add_\|async fn remove_' "$file" 2>/dev/null | grep -v "// " || true)
        if [[ -z "$mutations" ]]; then
            continue
        fi

        echo -e "  ${BOLD}$file${NC}"

        while IFS= read -r line; do
            lineno=$(echo "$line" | cut -d: -f1)
            fn_name=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")
            total_mutations=$((total_mutations + 1))

            # Check for permission/auth checks in function body (next 15 lines)
            fn_body=$(sed -n "${lineno},$((lineno + 20))p" "$file" 2>/dev/null || true)

            if echo "$fn_body" | grep -qiE "permission\|require_permission\|check_permission\|has_permission\|authorize\|ensure_permission\|guard\|RequireAuth\|current_user"; then
                pass "$fn_name — has permission check"
            elif echo "$fn_name" | grep -qiE "login|register|sign_in|sign_up|refresh_token"; then
                pass "$fn_name — auth endpoint (OK without permission)"
            else
                warn "$fn_name (line $lineno) — no permission check found"
                unprotected_mutations=$((unprotected_mutations + 1))
            fi
        done <<< "$mutations"
    done

    echo ""
    echo -e "  Total mutations: $total_mutations, Unprotected: $unprotected_mutations"
else
    warn "GraphQL directory not found: $GRAPHQL_DIR"
fi

# ─── 3. GraphQL queries: permission checks for non-public ───
header "3. GraphQL queries: permission checks"

if [[ -d "$GRAPHQL_DIR" ]]; then
    query_files=$(find "$GRAPHQL_DIR" -name "*.rs" | sort)
    total_queries=0
    unprotected_queries=0

    for file in $query_files; do
        # Find query functions (not mutations)
        queries=$(grep -n 'async fn' "$file" 2>/dev/null | grep -vE 'create_|update_|delete_|publish_|execute_|add_|remove_|mutation|mod test' || true)
        if [[ -z "$queries" ]]; then
            continue
        fi

        while IFS= read -r line; do
            lineno=$(echo "$line" | cut -d: -f1)
            fn_name=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")

            # Skip helper functions (not GraphQL resolvers)
            if echo "$fn_name" | grep -qiE "^new$|^from_|^into_|^to_|^is_|^get_|^set_|^with_|^build"; then
                continue
            fi

            total_queries=$((total_queries + 1))

            fn_body=$(sed -n "${lineno},$((lineno + 15))p" "$file" 2>/dev/null || true)

            if echo "$fn_body" | grep -qiE "permission\|require_permission\|check_permission\|authorize\|guard\|current_user\|ctx\.data"; then
                pass "$fn_name — has auth/permission context"
            else
                # Queries can be public, so this is a warning not error
                warn "$fn_name ($file:$lineno) — no auth context found (may be public)"
                unprotected_queries=$((unprotected_queries + 1))
            fi
        done <<< "$queries"
    done

    echo ""
    echo -e "  Total queries: $total_queries, Without auth: $unprotected_queries"
fi

# ─── 4. Middleware: auth middleware registered ───
header "4. Auth middleware registration"

MIDDLEWARE_DIR="apps/server/src/middleware"
SERVER_MAIN="apps/server/src"

if [[ -d "$MIDDLEWARE_DIR" ]]; then
    if [[ -f "$MIDDLEWARE_DIR/tenant.rs" ]]; then
        pass "Tenant middleware exists"
    else
        fail "Tenant middleware missing"
    fi

    if [[ -f "$MIDDLEWARE_DIR/rate_limit.rs" ]]; then
        pass "Rate limit middleware exists"
    else
        warn "Rate limit middleware missing"
    fi

    # Check if middleware is actually registered in router
    if grep -rq "middleware\|layer\|from_fn" "$SERVER_MAIN" --include="*.rs" 2>/dev/null; then
        pass "Middleware registration found in server"
    else
        warn "No middleware registration found (manual review)"
    fi
else
    warn "Middleware directory not found"
fi

# ─── Summary ───
echo ""
echo -e "${BOLD}━━━ RBAC Coverage Summary ━━━${NC}"
if [[ $ERRORS -eq 0 && $WARNINGS -eq 0 ]]; then
    echo -e "${GREEN}All checks passed!${NC}"
elif [[ $ERRORS -eq 0 ]]; then
    echo -e "${YELLOW}$WARNINGS warning(s) — manual review recommended${NC}"
else
    echo -e "${RED}$ERRORS error(s), $WARNINGS warning(s)${NC}"
fi
exit $ERRORS
