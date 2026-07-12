#!/usr/bin/env bash
# RusTok — Верификация архитектурных паттернов
# Фаза 1, 5: module registry, Axum composition, MCP, controller return types
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

SERVER_SRC="apps/server/src"
CORE_SRC="crates/rustok-core/src"

# ═══════════════════════════════════════════
# MODULE SYSTEM
# ═══════════════════════════════════════════

# ─── 1. Module registry: all modules registered ───
header "1. Module registry: build_registry()"

registry_file=$(grep -rl "build_registry\|ModuleRegistry::new\|registry\.register" "$SERVER_SRC" "$CORE_SRC" --include="*.rs" 2>/dev/null | head -1 || true)
if [[ -n "$registry_file" ]]; then
    pass "Module registry found: $registry_file"

    # Count registered modules
    reg_count=$(grep -c "register\|add_module\|push" "$registry_file" 2>/dev/null || echo "0")
    echo -e "    Registered modules: ~$reg_count"
else
    warn "No module registry found (build_registry/ModuleRegistry)"
fi

# ─── 2. Core modules: ModuleKind::Core (not toggleable) ───
header "2. Core modules: ModuleKind::Core"

core_module_refs=$(grep -rn "ModuleKind::Core\|Core\s*=>\|kind.*Core" "$CORE_SRC" "$SERVER_SRC" --include="*.rs" 2>/dev/null | grep -v "test\|// " || true)
if [[ -n "$core_module_refs" ]]; then
    pass "ModuleKind::Core defined"
else
    warn "ModuleKind::Core not found — core modules may be toggleable"
fi

# ─── 3. modules.toml vs dependencies() sync ───
header "3. modules.toml vs code dependencies() sync"

if [[ -f "modules.toml" ]]; then
    pass "modules.toml exists"

    # Check if code references modules.toml or has dependencies()
    dep_fns=$(grep -rn 'fn dependencies' "$CORE_SRC" "crates/rustok-*/src" --include="*.rs" 2>/dev/null | grep -v "test\|// " || true)
    if [[ -n "$dep_fns" ]]; then
        dep_count=$(echo "$dep_fns" | wc -l)
        pass "$dep_count module(s) implement dependencies() trait"
    else
        warn "No dependencies() trait implementations found in module crates"
    fi

    # List modules in TOML
    toml_modules=$(grep -E '^\[modules\.' "modules.toml" 2>/dev/null || grep -E '^\[' "modules.toml" 2>/dev/null || true)
    if [[ -n "$toml_modules" ]]; then
        toml_count=$(echo "$toml_modules" | wc -l)
        echo -e "    modules.toml entries: $toml_count"
    fi
else
    warn "modules.toml not found"
fi

# ═══════════════════════════════════════════
# AXUM RUNTIME COMPLIANCE
# ═══════════════════════════════════════════

# ─── 4. Axum router composition ───
header "4. Axum router composition"

hooks_impl=$(grep -rn 'impl Hooks\|fn routes\|fn after_routes\|fn connect_workers\|fn boot' "$SERVER_SRC" --include="*.rs" 2>/dev/null | grep -v "test\|// " || true)
if [[ -n "$hooks_impl" ]]; then
    pass "Server lifecycle composition found"

    # Check for routes outside Hooks
    direct_routes=$(grep -rn '\.route(\|Router::new\|axum::Router' "$SERVER_SRC" --include="*.rs" 2>/dev/null | grep -v "test\|// \|fn routes\|Hooks\|mod\.rs" || true)
    if [[ -n "$direct_routes" ]]; then
        count=$(echo "$direct_routes" | wc -l)
        warn "$count direct router definition(s) outside the composition boundary:"
        echo "$direct_routes" | head -5
    else
        pass "All routes use the composition boundary"
    fi
else
    warn "No server lifecycle composition found"
fi

# ─── 5. Controller return types ───
header "5. Controllers: transport return type"

CONTROLLERS_DIR="$SERVER_SRC/controllers"
if [[ -d "$CONTROLLERS_DIR" ]]; then
    controller_fns=$(grep -rn 'pub async fn' "$CONTROLLERS_DIR" --include="*.rs" 2>/dev/null | grep -v "test\|// " || true)
    custom_result=0
    loco_result=0

    while IFS= read -r line; do
        [[ -z "$line" ]] && continue
        file=$(echo "$line" | cut -d: -f1)
        lineno=$(echo "$line" | cut -d: -f2)
        # Get return type (next few lines)
        ret_type=$(sed -n "${lineno},$((lineno + 3))p" "$file" 2>/dev/null | tr '\n' ' ' || true)
        if echo "$ret_type" | grep -qiE 'rustok_web::HttpError\|Result<.*Response\|Result<Json'; then
            loco_result=$((loco_result + 1))
        elif echo "$ret_type" | grep -qiE 'Result<'; then
            fn_name=$(echo "$line" | grep -oP 'fn\s+\K\w+' || echo "unknown")
            warn "$(basename $file):$fn_name — unexpected transport Result type"
            custom_result=$((custom_result + 1))
        fi
    done <<< "$controller_fns"

    if [[ $custom_result -eq 0 ]]; then
        pass "All controllers use expected transport results ($loco_result functions)"
    else
        echo -e "    expected transport result: $loco_result, custom Result: $custom_result"
    fi
fi

# ═══════════════════════════════════════════
# MCP INTEGRATION
# ═══════════════════════════════════════════

# ─── 6. MCP: typed tool responses ───
header "6. MCP: McpToolResponse (not raw JSON)"

MCP_DIRS=("crates/rustok-core/src" "apps/server/src")
mcp_found=false

for mdir in "${MCP_DIRS[@]}"; do
    mcp_files=$(grep -rl "mcp\|Mcp\|MCP" "$mdir" --include="*.rs" 2>/dev/null | grep -v "test" || true)
    if [[ -n "$mcp_files" ]]; then
        mcp_found=true
        # Check for raw JSON instead of McpToolResponse
        raw_json_in_mcp=$(grep -rn 'serde_json::json!\|serde_json::to_value' $mcp_files 2>/dev/null | grep -v "McpToolResponse\|test\|// " || true)
        if [[ -n "$raw_json_in_mcp" ]]; then
            count=$(echo "$raw_json_in_mcp" | wc -l)
            warn "$count raw JSON return(s) in MCP code (should use McpToolResponse):"
            echo "$raw_json_in_mcp" | head -5
        else
            pass "MCP uses typed McpToolResponse (no raw JSON)"
        fi

        # Check for business logic in MCP adapter
        mcp_impl_lines=0
        for f in $mcp_files; do
            lines=$(wc -l < "$f" 2>/dev/null || echo "0")
            mcp_impl_lines=$((mcp_impl_lines + lines))
        done
        if [[ $mcp_impl_lines -gt 500 ]]; then
            warn "MCP adapter code is $mcp_impl_lines lines — may contain business logic (should be thin)"
        else
            pass "MCP adapter code is lean ($mcp_impl_lines lines)"
        fi
        break
    fi
done

if ! $mcp_found; then
    warn "No MCP implementation found"
fi

# ═══════════════════════════════════════════
# ARCHITECTURAL PATTERNS
# ═══════════════════════════════════════════

# ─── 7. Service layer: trait-based DI ───
header "7. Service layer: trait-based dependency injection"

trait_di=$(grep -rn 'Arc<dyn\|Box<dyn\|impl.*Repository\|impl.*Service' "$CORE_SRC" "crates/rustok-*/src" --include="*.rs" 2>/dev/null | grep -v "test\|// \|///\|mod " | head -20 || true)
concrete_di=$(grep -rn 'Arc<.*Service>\|Arc<.*Repository>' "$SERVER_SRC" --include="*.rs" 2>/dev/null | grep -v "dyn\|test\|// " || true)

if [[ -n "$trait_di" ]]; then
    count=$(echo "$trait_di" | wc -l)
    pass "$count trait-based DI reference(s) found"
fi

if [[ -n "$concrete_di" ]]; then
    count=$(echo "$concrete_di" | wc -l)
    warn "$count concrete type injection(s) — consider Arc<dyn Trait>:"
    echo "$concrete_di" | head -5
fi

# ─── 8. Migration naming convention ───
header "8. Migration naming convention"

MIGRATION_DIR="crates/rustok-migrations/src"
if [[ -d "$MIGRATION_DIR" ]]; then
    bad_names=$(find "$MIGRATION_DIR" -name "*.rs" -not -name "lib.rs" -not -name "mod.rs" | while read -r f; do
        basename_f=$(basename "$f")
        if ! echo "$basename_f" | grep -qE '^m[0-9]{8}_[0-9]{6}_'; then
            echo "  $basename_f — doesn't match mYYYYMMDD_NNNNNN_ pattern"
        fi
    done)
    if [[ -n "$bad_names" ]]; then
        count=$(echo "$bad_names" | wc -l)
        warn "$count migration(s) don't follow naming convention:"
        echo "$bad_names" | head -5
    else
        pass "All migrations follow naming convention"
    fi
fi

# ─── 9. Workspace: Cargo.toml structure ───
header "9. Workspace structure"

if [[ -f "Cargo.toml" ]]; then
    if grep -q '\[workspace\]' "Cargo.toml" 2>/dev/null; then
        pass "Workspace Cargo.toml exists"

        member_count=$(grep -c '"' "Cargo.toml" 2>/dev/null || echo "0")
        echo -e "    Workspace entries: ~$member_count"

        # Check for path dependencies outside workspace
        path_deps_outside=$(grep -rn 'path\s*=' crates/*/Cargo.toml apps/*/Cargo.toml 2>/dev/null | grep -v "crates/\|apps/\|UI/\|benches\|xtask" | grep -v "^\." || true)
        if [[ -n "$path_deps_outside" ]]; then
            warn "Path dependencies outside workspace:"
            echo "$path_deps_outside" | head -5
        else
            pass "No path dependencies outside workspace"
        fi
    fi
fi

# ─── 10. Telemetry: single initialization ───
header "10. Telemetry: single initialization"

telemetry_init=$(grep -rn 'init_subscriber\|init_telemetry\|init_tracing\|TracingSubscriber\|tracing_subscriber::fmt' "$SERVER_SRC" "$CORE_SRC" "crates/rustok-telemetry/src" --include="*.rs" 2>/dev/null | grep -v "test\|// " || true)
if [[ -n "$telemetry_init" ]]; then
    init_count=$(echo "$telemetry_init" | wc -l)
    if [[ $init_count -eq 1 ]]; then
        pass "Telemetry initialized exactly once"
    elif [[ $init_count -le 3 ]]; then
        pass "Telemetry init references: $init_count (may include declaration + call)"
    else
        warn "Multiple telemetry initializations found ($init_count) — risk of double init:"
        echo "$telemetry_init" | head -5
    fi
else
    warn "No telemetry initialization found"
fi

# ─── 11. Dependency boundaries (cargo metadata + allow/deny rules) ───
header "11. Dependency boundaries: apps/crates rules"

if python3 scripts/architecture_dependency_guard.py >/tmp/architecture_guard.log 2>&1; then
    pass "Architecture dependency guard passed"
else
    fail "Architecture dependency guard failed"
    sed -n '1,80p' /tmp/architecture_guard.log
fi

# ─── Summary ───
echo ""
echo -e "${BOLD}━━━ Architecture Summary ━━━${NC}"
if [[ $ERRORS -eq 0 && $WARNINGS -eq 0 ]]; then
    echo -e "${GREEN}All checks passed!${NC}"
elif [[ $ERRORS -eq 0 ]]; then
    echo -e "${YELLOW}$WARNINGS warning(s) — manual review recommended${NC}"
else
    echo -e "${RED}$ERRORS error(s), $WARNINGS warning(s)${NC}"
fi
exit $ERRORS
