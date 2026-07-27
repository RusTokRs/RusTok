from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLAN = ROOT / "crates/rustok-forum/docs/implementation-plan.md"
VERIFIER = ROOT / "scripts/verify/verify-forum-topic-create-audience-transport-composition.mjs"

section = """### Delivered in `FORUM-20AS`

- compose both legacy and inline-quote GraphQL topic-create mutations through one manifest-backed
  runtime wrapper and the existing context-aware owner methods;
- compose both REST topic-create handlers through `HostRuntimeContext`, using only authenticated
  tenant/user identity plus the middleware-resolved locale and route channel;
- attach read deadline, permission claims, and a bounded correlation id before any optional owner
  facts call, rejecting mismatched request tenant or actor before provider access;
- consume the existing feature-guarded Groups facts publication for both transports while keeping
  provider absence fail closed and adding no topic-create DTO, migration, or Forum-to-Groups dependency.

"""

plan = PLAN.read_text()
if plan.count(section) != 1:
    raise RuntimeError(f"expected one misplaced FORUM-20AS section, found {plan.count(section)}")
plan = plan.replace(section, "", 1)
anchor = """- gate every public `TopicService` create path before topic, relation, counter, user-stat, or
  event writes and publish context-aware owner seams without changing GraphQL or REST DTOs.

### Compatibility and degraded mode
"""
if plan.count(anchor) != 1:
    raise RuntimeError(f"expected one FORUM-20AR tail anchor, found {plan.count(anchor)}")
plan = plan.replace(
    anchor,
    """- gate every public `TopicService` create path before topic, relation, counter, user-stat, or
  event writes and publish context-aware owner seams without changing GraphQL or REST DTOs.

""" + section + "### Compatibility and degraded mode\n",
    1,
)
PLAN.write_text(plan)

verifier = VERIFIER.read_text()
old = """for (const marker of [
  "FORUM-20A-AS provide",
  "### Delivered in `FORUM-20AS`",
  "Forum trust and Channel membership facts adapters",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}
"""
new = """for (const marker of [
  "FORUM-20A-AS provide",
  "### Delivered in `FORUM-20AS`",
  "Forum trust and Channel membership facts adapters",
]) {
  requireText(plan, marker, `canonical Forum plan is missing ${marker}`);
}
const forum20DeliveryTail = between(
  plan,
  "### Delivered in `FORUM-20AR`",
  "### Compatibility and degraded mode",
  "FORUM-20AR/AS delivery tail",
);
requireText(
  forum20DeliveryTail,
  "### Delivered in `FORUM-20AS`",
  "FORUM-20AS delivered section must follow FORUM-20AR before compatibility",
);
"""
if verifier.count(old) != 1:
    raise RuntimeError(f"expected one verifier plan marker block, found {verifier.count(old)}")
verifier = verifier.replace(old, new, 1)
old_marker = '    "runtime.topic_service()",\n'
new_marker = '    ".topic_service()",\n'
if verifier.count(old_marker) != 1:
    raise RuntimeError(
        f"expected one multiline REST topic-service verifier marker, found {verifier.count(old_marker)}"
    )
VERIFIER.write_text(verifier.replace(old_marker, new_marker, 1))

(ROOT / "scripts/agent/repair_forum_20as_plan.py").unlink()
(ROOT / ".github/workflows/agent-repair-forum-20as-plan.yml").unlink()
