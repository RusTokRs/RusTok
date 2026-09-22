#!/usr/bin/env bash
# RusTok вЂ” Р’РµСЂРёС„РёРєР°С†РёСЏ unsafe code patterns
# Р¤Р°Р·Р° 19.1, 19.3: unwrap/expect/panic, blocking РІ async, std::fs РІ async
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
pass()   { echo -e "  ${GREEN}вњ“${NC} $1"; }
fail()   { echo -e "  ${RED}вњ—${NC} $1"; ERRORS=$((ERRORS + 1)); }
warn()   { echo -e "  ${YELLOW}!${NC} $1"; WARNINGS=$((WARNINGS + 1)); }

# Production code paths (excluding tests)
PROD_PATHS=(
    "crates/libs/rustok-core/src"
    "crates/modules/rustok-content/src"
    "crates/modules/rustok-commerce/src"
    "crates/modules/rustok-blog/src"
    "crates/modules/rustok-forum/src"
    "crates/modules/rustok-pages/src"
    "crates/libs/rustok-events/src"
    "crates/modules/rustok-rbac/src"
    "crates/modules/rustok-tenant/src"
    "crates/libs/rustok-telemetry/src"
    "crates/modules/rustok-iggy/src"
    "crates/modules/rustok-outbox/src"
    "crates/modules/rustok-index/src"
    "crates/modules/alloy/src"
    "apps/server/src"
)

EXISTING=()
for p in "${PROD_PATHS[@]}"; do
    [[ -d "$p" ]] && EXISTING+=("$p")
done

if [[ ${#EXISTING[@]} -eq 0 ]]; then
    echo -e "${YELLOW}No production code paths found. Skipping.${NC}"
    exit 0
fi

# Helper: filter out test files and test modules from grep output
filter_tests() {
    grep -v "_test\.rs" | grep -v "tests\.rs" | grep -v "test_utils" | grep -v "mod tests" | grep -v "#\[cfg(test)\]" | grep -v "#\[test\]" | grep -v "proptest" || true
}

# в”Ђв”Ђв”Ђ 1. unwrap() РІ production РєРѕРґРµ в”Ђв”Ђв”Ђ
header "1. РџРѕРёСЃРє .unwrap() РІ production РєРѕРґРµ"

count=$(grep -rn '\.unwrap()' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No .unwrap() calls in production code"
else
    warn "$count .unwrap() call(s) found вЂ” review each one:"
    grep -rn '\.unwrap()' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -20
    if [[ $count -gt 20 ]]; then
        echo "      ... and $((count - 20)) more"
    fi
fi

# в”Ђв”Ђв”Ђ 2. expect() РІ production РєРѕРґРµ в”Ђв”Ђв”Ђ
header "2. РџРѕРёСЃРє .expect() РІ production РєРѕРґРµ"

count=$(grep -rn '\.expect(' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No .expect() calls in production code"
else
    warn "$count .expect() call(s) found вЂ” review each (some may be justified):"
    grep -rn '\.expect(' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -20
    if [[ $count -gt 20 ]]; then
        echo "      ... and $((count - 20)) more"
    fi
fi

# в”Ђв”Ђв”Ђ 3. panic!() РІ production РєРѕРґРµ в”Ђв”Ђв”Ђ
header "3. РџРѕРёСЃРє panic!() РІ production РєРѕРґРµ"

count=$(grep -rn 'panic!(' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No panic!() in production code"
else
    fail "$count panic!() call(s) found:"
    grep -rn 'panic!(' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -20
fi

# в”Ђв”Ђв”Ђ 4. todo!() / unimplemented!() РІ production РєРѕРґРµ в”Ђв”Ђв”Ђ
header "4. РџРѕРёСЃРє todo!() / unimplemented!() РІ production РєРѕРґРµ"

count=$(grep -rn 'todo!()\|unimplemented!()' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No todo!()/unimplemented!() in production code"
else
    fail "$count todo!()/unimplemented!() call(s) found:"
    grep -rn 'todo!()\|unimplemented!()' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -20
fi

# в”Ђв”Ђв”Ђ 5. std::thread::sleep РІ async РєРѕРґРµ в”Ђв”Ђв”Ђ
header "5. РџРѕРёСЃРє std::thread::sleep РІ async РєРѕРґРµ"

count=$(grep -rn 'std::thread::sleep\|thread::sleep' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No std::thread::sleep in production code"
else
    fail "$count blocking sleep call(s) found (should use tokio::time::sleep):"
    grep -rn 'std::thread::sleep\|thread::sleep' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests
fi

# в”Ђв”Ђв”Ђ 6. std::fs:: РІ async РєРѕРґРµ в”Ђв”Ђв”Ђ
header "6. РџРѕРёСЃРє std::fs:: РІ async РєРѕРґРµ (should be tokio::fs::)"

count=$(grep -rn 'std::fs::' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No std::fs:: in production code"
else
    warn "$count std::fs:: usage(s) found (should use tokio::fs:: in async context):"
    grep -rn 'std::fs::' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -20
fi

# в”Ђв”Ђв”Ђ 7. block_on() РІРЅСѓС‚СЂРё async context в”Ђв”Ђв”Ђ
header "7. РџРѕРёСЃРє block_on() РІРЅСѓС‚СЂРё async context"

count=$(grep -rn 'block_on\|block_in_place' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No block_on()/block_in_place() in production code"
else
    warn "$count block_on/block_in_place call(s) found:"
    grep -rn 'block_on\|block_in_place' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -10
fi

# в”Ђв”Ђв”Ђ 8. println!/eprintln! РІРјРµСЃС‚Рѕ tracing:: в”Ђв”Ђв”Ђ
header "8. РџРѕРёСЃРє println!/eprintln! РІ production РєРѕРґРµ (should use tracing::)"

count=$(grep -rn 'println!\|eprintln!' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No println!/eprintln! in production code"
else
    warn "$count println!/eprintln! call(s) found (should use tracing::):"
    grep -rn 'println!\|eprintln!' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -20
fi

# в”Ђв”Ђв”Ђ 9. unreachable!() Р±РµР· РѕР±РѕСЃРЅРѕРІР°РЅРёСЏ в”Ђв”Ђв”Ђ
header "9. РџРѕРёСЃРє unreachable!() РІ production РєРѕРґРµ"

count=$(grep -rn 'unreachable!(' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | wc -l)
if [[ $count -eq 0 ]]; then
    pass "No unreachable!() in production code"
else
    warn "$count unreachable!() call(s) вЂ” verify each is justified:"
    grep -rn 'unreachable!(' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | head -10
fi

# в”Ђв”Ђв”Ђ 10. static / lazy_static! / once_cell::Lazy (should use AppContext) в”Ђв”Ђв”Ђ
header "10. РџРѕРёСЃРє global state: static / lazy_static / once_cell::Lazy"

static_state=$(grep -rn 'static\s\+mut\|lazy_static!\|once_cell::sync::Lazy\|static\s\+ref' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | filter_tests | grep -v "const\|type\|str\|// " || true)
if [[ -n "$static_state" ]]; then
    count=$(echo "$static_state" | wc -l)
    warn "$count global state declaration(s) вЂ” should use AppContext.shared_store:"
    echo "$static_state" | head -10
else
    pass "No global mutable state (lazy_static/once_cell)"
fi

# в”Ђв”Ђв”Ђ 11. Unsafe fallback defaults for secrets в”Ђв”Ђв”Ђ
header "11. Unsafe fallback defaults for secrets"

unsafe_fallback=$(grep -rn 'unwrap_or\|unwrap_or_else\|unwrap_or_default' "${EXISTING[@]}" --include="*.rs" 2>/dev/null | grep -iE 'secret\|password\|jwt\|token\|key\|api_key' | filter_tests || true)
if [[ -n "$unsafe_fallback" ]]; then
    count=$(echo "$unsafe_fallback" | wc -l)
    fail "$count unsafe fallback(s) for secrets (should fail loudly, not use defaults):"
    echo "$unsafe_fallback" | head -10
else
    pass "No unsafe fallback defaults for secrets"
fi

# в”Ђв”Ђв”Ђ Summary в”Ђв”Ђв”Ђ
echo ""
echo -e "${BOLD}в”Ѓв”Ѓв”Ѓ Unsafe Code Summary в”Ѓв”Ѓв”Ѓ${NC}"
if [[ $ERRORS -eq 0 && $WARNINGS -eq 0 ]]; then
    echo -e "${GREEN}All checks passed!${NC}"
elif [[ $ERRORS -eq 0 ]]; then
    echo -e "${YELLOW}$WARNINGS warning(s) вЂ” manual review recommended${NC}"
else
    echo -e "${RED}$ERRORS error(s), $WARNINGS warning(s)${NC}"
fi
exit $ERRORS

