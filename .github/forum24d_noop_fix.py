from pathlib import Path

path = Path("crates/rustok-forum/src/services/topic_owner.rs")
text = path.read_text()
old = '''        let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
        let mut active: forum_topic::ActiveModel = topic.into();
        active.updated_at = Set(Utc::now().into());
        active.update(&txn).await?;
        publish_forum_topic_projection_in_tx(
            &self.event_bus,
            &txn,
            tenant_id,
            security.user_id,
            topic_id,
        )
        .await?;
        txn.commit().await?;
'''
new = '''        if result.changed {
            let topic = topic::TopicService::find_topic_in_tx(&txn, tenant_id, topic_id).await?;
            let mut active: forum_topic::ActiveModel = topic.into();
            active.updated_at = Set(Utc::now().into());
            active.update(&txn).await?;
            publish_forum_topic_projection_in_tx(
                &self.event_bus,
                &txn,
                tenant_id,
                security.user_id,
                topic_id,
            )
            .await?;
        }
        txn.commit().await?;
'''
if text.count(old) != 1:
    raise SystemExit("FORUM-24D rename owner no-op anchor changed")
path.write_text(text.replace(old, new, 1))
