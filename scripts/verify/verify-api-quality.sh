#!/usr/bin/env bash
# RusTok — Верификация API quality
# Фаза 19.12-19.14: GraphQL antipatterns, REST antipatterns, REST↔GraphQL parity
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

# ═══════════════════════════════════════════
# GRAPHQL CHECKS
# ═══════════════════════════════════════════

header "GRAPHQL: N+1 query detection (DataLoader usage)"

if [[ -d "$GRAPHQL_DIR" ]]; then
    # Check if DataLoader is used
    if [[ -f "$GRAPHQL_DIR/loaders.rs" ]]; then
        pass "loaders.rs exists (DataLoader definitions)"
        loader_count=$(grep -c 'impl Loader\|impl BatchFn\|DataLoader' "$GRAPHQL_DIR/loaders.rs" 2>/dev/null || echo "0")
        echo -e "    DataLoader implementations found: $loader_count"
    else
        warn "loaders.rs not found — DataLoaders may not be implemented"
    fi

    # Find resolvers that directly access DB (potential N+1)
    direct_db_in_resolvers=$(grep -rn '\.find\|\.all(&\|\.filter(' "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null | grep -v "loaders\|test\|mod\.rs" || true)
    if [[ -n "$direct_db_in_resolvers" ]]; then
        count=$(echo "$direct_db_in_resolvers" | wc -l)
        warn "$count direct DB access(es) in GraphQL resolvers (potential N+1):"
        echo "$direct_db_in_resolvers" | head -10
    else
        pass "No direct DB access in resolvers (using DataLoaders or services)"
    fi
fi

# ─── GraphQL: MergedObject ───
header "GRAPHQL: MergedObject for modular schema"

if [[ -d "$GRAPHQL_DIR" ]]; then
    if grep -rq "MergedObject" "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null; then
        pass "MergedObject used for schema composition"
        grep -rn "MergedObject" "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null | head -5
    else
        warn "MergedObject not found — schema may be monolithic"
    fi
fi

# ─── GraphQL: Error extensions ───
header "GRAPHQL: Error handling (extensions vs String errors)"

if [[ -d "$GRAPHQL_DIR" ]]; then
    string_errors=$(grep -rn 'FieldError::new\|Err("' "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null || true)
    if [[ -n "$string_errors" ]]; then
        count=$(echo "$string_errors" | wc -l)
        warn "$count string-based error(s) in GraphQL (consider structured extensions):"
        echo "$string_errors" | head -10
    else
        pass "No raw string errors in GraphQL"
    fi

    # Check for error extensions usage
    if grep -rq "extend_with\|ErrorExtensions\|extensions" "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null; then
        pass "Error extensions used"
    fi
fi

# ─── GraphQL: TenantContext in resolvers ───
header "GRAPHQL: TenantContext in resolvers"

if [[ -d "$GRAPHQL_DIR" ]]; then
    # Check that resolvers access TenantContext
    resolvers_with_ctx=$(grep -rl 'ctx\.data\|context\.data\|TenantContext\|tenant_id' "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null | grep -v "mod\.rs\|types\.rs\|common\.rs" || true)
    resolver_files=$(find "$GRAPHQL_DIR" -name "*.rs" -not -name "mod.rs" -not -name "types.rs" -not -name "common.rs" -not -name "errors.rs" -not -name "schema.rs" -not -name "persisted.rs" | sort)

    for file in $resolver_files; do
        basename_f=$(basename "$file" .rs)
        if echo "$resolvers_with_ctx" | grep -q "$file" 2>/dev/null; then
            pass "$basename_f — accesses context/tenant data"
        else
            # Some files may not need tenant context (observability, loaders, types)
            if echo "$basename_f" | grep -qiE "observability|loader|type|error|common|schema"; then
                pass "$basename_f — no tenant context needed"
            else
                warn "$basename_f — no tenant context access found"
            fi
        fi
    done
fi

# ─── GraphQL: Pagination in list queries ───
header "GRAPHQL: Pagination in list queries"

if [[ -d "$GRAPHQL_DIR" ]]; then
    list_fns=$(grep -rn 'async fn.*s(\|async fn list\|async fn all\|async fn search' "$GRAPHQL_DIR" --include="*.rs" 2>/dev/null | grep -v "test\|mod\|type" || true)
    if [[ -n "$list_fns" ]]; then
        while IFS= read -r line; do
            file=$(echo "$line" | cut -d: -f1)
            lineno=$(echo "$line" | cut -d: -f2)
            fn_name=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")
            fn_context=$(sed -n "${lineno},$((lineno + 10))p" "$file" 2>/dev/null || true)
            if echo "$fn_context" | grep -qiE "pagination\|page\|limit\|offset\|first\|after\|cursor\|per_page\|Paginated\|Connection"; then
                pass "$fn_name — has pagination"
            else
                warn "$fn_name ($file:$lineno) — no pagination detected"
            fi
        done <<< "$list_fns"
    fi
fi

# ═══════════════════════════════════════════
# REST CHECKS
# ═══════════════════════════════════════════

header "REST: OpenAPI annotations (#[utoipa::path])"

if [[ -d "$CONTROLLERS_DIR" ]]; then
    controller_files=$(find "$CONTROLLERS_DIR" -name "*.rs" | sort)
    total_handlers=0
    annotated_handlers=0

    for file in $controller_files; do
        basename_f=$(basename "$file" .rs)
        # Skip files that don't have HTTP handlers
        if echo "$basename_f" | grep -qiE "^mod$|^graphql$"; then
            continue
        fi

        handlers=$(grep -n 'pub async fn' "$file" 2>/dev/null || true)
        if [[ -z "$handlers" ]]; then
            continue
        fi

        while IFS= read -r line; do
            lineno=$(echo "$line" | cut -d: -f1)
            fn_name=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")
            total_handlers=$((total_handlers + 1))

            # Check for utoipa annotation above the function (within 10 lines)
            start=$((lineno > 10 ? lineno - 10 : 1))
            above=$(sed -n "${start},${lineno}p" "$file" 2>/dev/null || true)
            if echo "$above" | grep -q "utoipa::path\|#\[utoipa"; then
                pass "$basename_f::$fn_name — has #[utoipa::path]"
                annotated_handlers=$((annotated_handlers + 1))
            else
                warn "$basename_f::$fn_name ($file:$lineno) — missing #[utoipa::path]"
            fi
        done <<< "$handlers"
    done

    echo ""
    echo -e "  OpenAPI coverage: $annotated_handlers/$total_handlers handlers annotated"
fi

# ─── REST: HTTP status codes ───
header "REST: HTTP status codes correctness"

if [[ -d "$CONTROLLERS_DIR" ]]; then
    # Check that POST handlers return 201, not 200
    post_handlers=$(grep -rn '#\[post\|post(' "$CONTROLLERS_DIR" --include="*.rs" 2>/dev/null || true)
    if [[ -n "$post_handlers" ]]; then
        while IFS= read -r line; do
            file=$(echo "$line" | cut -d: -f1)
            lineno=$(echo "$line" | cut -d: -f2)
            fn_context=$(sed -n "${lineno},$((lineno + 20))p" "$file" 2>/dev/null || true)
            if echo "$fn_context" | grep -qiE "StatusCode::CREATED\|201\|status = 201"; then
                fn_name=$(echo "$fn_context" | grep -oP 'fn\s+\K\w+' | head -1 || echo "unknown")
                pass "$fn_name — returns 201 Created"
            fi
        done <<< "$post_handlers"
    fi

    # Check for status 200 on create operations (antipattern)
    create_with_200=$(grep -rn 'StatusCode::OK' "$CONTROLLERS_DIR" --include="*.rs" 2>/dev/null | grep -i "create" || true)
    if [[ -n "$create_with_200" ]]; then
        count=$(echo "$create_with_200" | wc -l)
        warn "$count create operation(s) returning 200 instead of 201"
    fi
fi

# ─── REST: Input validation ───
header "REST: Input validation (validator::Validate)"

if [[ -d "$CONTROLLERS_DIR" ]]; then
    # Check that Input structs use Validate derive
    input_structs=$(grep -rn 'Input\|Request\|Payload' "$CONTROLLERS_DIR" --include="*.rs" 2>/dev/null | grep -i "struct\|Json<" || true)
    validate_usage=$(grep -rc 'Validate\|validate()' "$CONTROLLERS_DIR" --include="*.rs" 2>/dev/null || true)
    total_validate=$(echo "$validate_usage" | awk -F: '{sum += $2} END {print sum+0}')

    if [[ $total_validate -gt 0 ]]; then
        pass "validator::Validate used ($total_validate references)"
    else
        warn "No Validate usage found in controllers (input may not be validated)"
    fi
fi

# ─── REST: Rate limiting on auth endpoints ───
header "REST: Rate limiting on auth endpoints"

if [[ -f "apps/server/src/middleware/rate_limit.rs" ]]; then
    pass "Rate limiting middleware exists"
    if grep -q "auth\|login\|register" "apps/server/src/middleware/rate_limit.rs" 2>/dev/null; then
        pass "Rate limiting references auth endpoints"
    else
        warn "Rate limiting doesn't explicitly reference auth endpoints"
    fi
else
    warn "Rate limiting middleware not found"
fi

# ─── REST: CORS middleware ───
header "REST: CORS middleware"

if grep -rq "CorsLayer\|cors\|Cors" "apps/server/src" --include="*.rs" 2>/dev/null; then
    pass "CORS configuration found"
else
    warn "No CORS configuration found"
fi

# ═══════════════════════════════════════════
# REST ↔ GRAPHQL PARITY
# ═══════════════════════════════════════════

header "REST ↔ GraphQL parity: auth operations"

if [[ -d "$CONTROLLERS_DIR" && -d "$GRAPHQL_DIR" ]]; then
    auth_ops=("login" "register" "refresh" "change_password")
    for op in "${auth_ops[@]}"; do
        rest_has=$(grep -rl "$op" "$CONTROLLERS_DIR/auth.rs" 2>/dev/null || true)
        gql_has=$(grep -rl "$op" "$GRAPHQL_DIR/auth/" "$GRAPHQL_DIR/mutations.rs" 2>/dev/null || true)

        if [[ -n "$rest_has" && -n "$gql_has" ]]; then
            pass "$op — available in both REST and GraphQL"
        elif [[ -n "$rest_has" ]]; then
            warn "$op — only in REST (missing from GraphQL)"
        elif [[ -n "$gql_has" ]]; then
            warn "$op — only in GraphQL (missing from REST)"
        else
            warn "$op — not found in either REST or GraphQL"
        fi
    done
fi

# ─── Parity: shared AuthLifecycleService ───
header "REST ↔ GraphQL parity: shared auth service"

auth_service=$(grep -rl "AuthLifecycleService\|IdentityService\|identity_service\|auth_lifecycle" "apps/server/src" "crates/libs/rustok-core/src" 2>/dev/null || true)
if [[ -n "$auth_service" ]]; then
    pass "Shared auth service found:"
    echo "$auth_service" | head -5
else
    warn "No shared auth service found — REST and GraphQL may duplicate auth logic"
fi

# ─── Parity: business logic not in controllers/resolvers ───
header "REST ↔ GraphQL: business logic location"

# Check if controllers call domain services (not direct DB access)
if [[ -d "$CONTROLLERS_DIR" ]]; then
    direct_db_in_controllers=$(grep -rn '\.find\|\.all(&\|\.insert\|\.save\|::create\|ActiveModel' "$CONTROLLERS_DIR" --include="*.rs" 2>/dev/null | grep -v "graphql\|swagger\|mod\.rs" || true)
    if [[ -n "$direct_db_in_controllers" ]]; then
        count=$(echo "$direct_db_in_controllers" | wc -l)
        warn "$count direct DB access(es) in controllers (should use services):"
        echo "$direct_db_in_controllers" | head -10
    else
        pass "Controllers delegate to services (no direct DB access)"
    fi
fi

# ─── Summary ───
echo ""
echo -e "${BOLD}━━━ API Quality Summary ━━━${NC}"
if [[ $ERRORS -eq 0 && $WARNINGS -eq 0 ]]; then
    echo -e "${GREEN}All checks passed!${NC}"
elif [[ $ERRORS -eq 0 ]]; then
    echo -e "${YELLOW}$WARNINGS warning(s) — manual review recommended${NC}"
else
    echo -e "${RED}$ERRORS error(s), $WARNINGS warning(s)${NC}"
fi
exit $ERRORS
