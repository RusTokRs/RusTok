from pathlib import Path

path = Path('crates/rustok-forum/tests/topic_route_identity_sqlite.rs')
text = path.read_text()
old = '''    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin,
        "target-route",
    )
'''
new = '''    let target_topic_id = create_topic(
        &db,
        &event_bus,
        tenant_id,
        category_id,
        admin.clone(),
        "target-route",
    )
'''
if text.count(old) != 1:
    raise SystemExit('target topic fixture shape changed')
text = text.replace(old, new, 1)
old = '''    db.execute(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        "UPDATE forum_topics SET deleted_at = CURRENT_TIMESTAMP WHERE tenant_id = ? AND id = ?",
        vec![tenant_id.into(), source_topic_id.into()],
    ))
    .await?;
'''
new = '''    TopicService::new(db.clone(), event_bus.clone())
        .delete(tenant_id, source_topic_id, admin)
        .await?;
'''
if text.count(old) != 1:
    raise SystemExit('direct tombstone fixture shape changed')
path.write_text(text.replace(old, new, 1))
