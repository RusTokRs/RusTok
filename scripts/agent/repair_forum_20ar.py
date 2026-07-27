from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def replace_once(relative: str, old: str, new: str) -> None:
    path = ROOT / relative
    text = path.read_text()
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{relative}: expected one anchor, found {count}: {old[:140]!r}")
    path.write_text(text.replace(old, new, 1))


test_path = "crates/rustok-forum/tests/topic_create_audience_enforcement_sqlite.rs"
replace_once(
    test_path,
    "    let explicit_allow =\n        create_category(&db, tenant_id, policy_admin.clone(), \"explicit-allow\", None).await;\n",
    "    let explicit_allow =\n        create_category(&db, tenant_id, policy_admin.clone(), \"explicit-allow\", None).await;\n"
    "    let explicit_deny =\n"
    "        create_category(&db, tenant_id, policy_admin.clone(), \"explicit-deny\", None).await;\n",
)
replace_once(
    test_path,
    "        .await\n        .expect(\"explicit allow topic-create layer should persist\");\n\n    let ordinary = TopicService::new(db.clone(), event_bus.clone());\n",
    "        .await\n        .expect(\"explicit allow topic-create layer should persist\");\n"
    "    policies\n"
    "        .set(\n"
    "            tenant_id,\n"
    "            explicit_deny,\n"
    "            policy_admin.clone(),\n"
    "            SetForumCategoryTopicCreateAudiencePolicyInput {\n"
    "                constraints: ForumAudienceConstraints {\n"
    "                    roles_any: vec![UserRole::Admin],\n"
    "                    deny_user_ids: vec![allowed_admin_id],\n"
    "                    ..ForumAudienceConstraints::default()\n"
    "                },\n"
    "            },\n"
    "        )\n"
    "        .await\n"
    "        .expect(\"explicit deny topic-create layer should persist\");\n\n"
    "    let ordinary = TopicService::new(db.clone(), event_bus.clone());\n",
)
replace_once(
    test_path,
    "        .await\n        .expect(\"explicit allow should short-circuit unresolved owner facts\");\n\n    let count_before_denials = topic_count(&db).await;\n",
    "        .await\n        .expect(\"explicit allow should short-circuit unresolved owner facts\");\n\n"
    "    let count_before_explicit_deny = topic_count(&db).await;\n"
    "    assert!(matches!(\n"
    "        ordinary\n"
    "            .create(\n"
    "                tenant_id,\n"
    "                allowed_admin.clone(),\n"
    "                topic_input(explicit_deny, \"explicit-denied\"),\n"
    "            )\n"
    "            .await,\n"
    "        Err(ForumError::Forbidden(_))\n"
    "    ));\n"
    "    assert_eq!(topic_count(&db).await, count_before_explicit_deny);\n\n"
    "    let count_before_denials = topic_count(&db).await;\n",
)

verifier_path = "scripts/verify/verify-forum-topic-create-audience-enforcement.mjs"
replace_once(
    verifier_path,
    "function rejectText(source, marker, message) {\n  if (source.includes(marker)) failures.push(message);\n}\n\nconst contractPath =",
    "function rejectText(source, marker, message) {\n"
    "  if (source.includes(marker)) failures.push(message);\n"
    "}\n\n"
    "function between(source, start, end, label) {\n"
    "  const from = source.indexOf(start);\n"
    "  const to = source.indexOf(end, from + start.length);\n"
    "  if (from < 0 || to < 0 || to <= from) {\n"
    "    failures.push(`${label}: bounded section is missing`);\n"
    "    return \"\";\n"
    "  }\n"
    "  return source.slice(from, to);\n"
    "}\n\n"
    "const contractPath =",
)
replace_once(
    verifier_path,
    "for (const marker of [\n  \"ForumTopicCreateAudienceAuthorizationService\",\n  \"SharedForumAudienceFactsPort\",\n  \"pub fn with_audience_facts\",\n  \"pub async fn create_with_audience_context\",\n  \"pub async fn create_command_with_audience_context\",\n  \"require(tenant_id, input.category_id, &security, context)\",\n  \".inner.create_command(tenant_id, security, input)\",\n]) {\n  requireText(facade, marker, `TopicService facade is missing ${marker}`);\n}\nconst requireIndex = facade.indexOf(\".require(tenant_id, input.category_id, &security, context)\");\nconst createIndex = facade.indexOf(\".inner.create_command(tenant_id, security, input)\");\nif (requireIndex < 0 || createIndex < 0 || requireIndex > createIndex) {\n  failures.push(\"TopicService must authorize before delegating to the topic write owner\");\n}\n",
    "for (const marker of [\n"
    "  \"ForumTopicCreateAudienceAuthorizationService\",\n"
    "  \"SharedForumAudienceFactsPort\",\n"
    "  \"pub fn with_audience_facts\",\n"
    "  \"pub async fn create_with_audience_context\",\n"
    "  \"pub async fn create_command_with_audience_context\",\n"
    "]) {\n"
    "  requireText(facade, marker, `TopicService facade is missing ${marker}`);\n"
    "}\n"
    "const createBlock = between(\n"
    "  facade,\n"
    "  \"async fn create_command_with_optional_audience_context(\",\n"
    "  \"pub async fn get(\",\n"
    "  \"topic-create facade helper\",\n"
    ");\n"
    "for (const marker of [\n"
    "  \".require(tenant_id, input.category_id, &security, context)\",\n"
    "  \".create_command(tenant_id, security, input)\",\n"
    "]) {\n"
    "  requireText(createBlock, marker, `TopicService create helper is missing ${marker}`);\n"
    "}\n"
    "const requireIndex = createBlock.indexOf(\n"
    "  \".require(tenant_id, input.category_id, &security, context)\",\n"
    ");\n"
    "const createIndex = createBlock.indexOf(\n"
    "  \".create_command(tenant_id, security, input)\",\n"
    ");\n"
    "if (requireIndex < 0 || createIndex < 0 || requireIndex > createIndex) {\n"
    "  failures.push(\"TopicService must authorize before delegating to the topic write owner\");\n"
    "}\n",
)
replace_once(
    verifier_path,
    "  \"explicit allow should short-circuit unresolved owner facts\",\n  \"count_before_denials\",\n",
    "  \"explicit allow should short-circuit unresolved owner facts\",\n"
    "  \"explicit deny topic-create layer should persist\",\n"
    "  \"count_before_explicit_deny\",\n"
    "  \"explicit-denied\",\n"
    "  \"count_before_denials\",\n",
)

for relative in [
    "scripts/agent/repair_forum_20ar.py",
    ".github/workflows/agent-repair-forum-topic-create-audience-enforcement-20ar.yml",
]:
    path = ROOT / relative
    if path.exists():
        path.unlink()
