use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_topic_author",
        r#"CREATE TRIGGER forum_70_auto_subscribe_topic_author AFTER INSERT ON forum_topics
        FOR EACH ROW WHEN NEW.author_id IS NOT NULL AND COALESCE((SELECT auto_subscribe_topic_authors FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),1)=1
        BEGIN INSERT INTO forum_topic_subscriptions
        (topic_id,user_id,tenant_id,level,notify_mentions,notify_replies,notify_new_topics,digest_mode,last_notified_at,revision,created_at,updated_at)
        VALUES (NEW.id,NEW.author_id,NEW.tenant_id,
        COALESCE((SELECT topic_author_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'watching'),1,
        CASE COALESCE((SELECT topic_author_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'watching') WHEN 'watching' THEN 1 ELSE 0 END,
        CASE COALESCE((SELECT topic_author_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'watching') WHEN 'watching' THEN 1 ELSE 0 END,
        CASE COALESCE((SELECT topic_author_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'watching') WHEN 'watching' THEN 'immediate' ELSE 'disabled' END,
        NULL,1,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT(tenant_id,topic_id,user_id) DO UPDATE SET
          level=excluded.level,notify_mentions=excluded.notify_mentions,notify_replies=excluded.notify_replies,
          notify_new_topics=excluded.notify_new_topics,digest_mode=excluded.digest_mode,
          revision=forum_topic_subscriptions.revision+1,updated_at=CURRENT_TIMESTAMP
        WHERE forum_topic_subscriptions.level='normal'; END"#,
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_insert",
        r#"CREATE TRIGGER forum_70_auto_subscribe_reply_participant_insert AFTER INSERT ON forum_replies
        FOR EACH ROW WHEN NEW.author_id IS NOT NULL AND NEW.status='approved'
          AND COALESCE((SELECT auto_subscribe_reply_participants FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),1)=1
        BEGIN INSERT INTO forum_topic_subscriptions
        (topic_id,user_id,tenant_id,level,notify_mentions,notify_replies,notify_new_topics,digest_mode,last_notified_at,revision,created_at,updated_at)
        VALUES (NEW.topic_id,NEW.author_id,NEW.tenant_id,
        COALESCE((SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'tracking'),1,
        CASE COALESCE((SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'tracking') WHEN 'watching' THEN 1 ELSE 0 END,0,
        CASE COALESCE((SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'tracking') WHEN 'watching' THEN 'immediate' ELSE 'disabled' END,
        NULL,1,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT(tenant_id,topic_id,user_id) DO UPDATE SET
          level=excluded.level,notify_mentions=excluded.notify_mentions,notify_replies=excluded.notify_replies,
          notify_new_topics=excluded.notify_new_topics,digest_mode=excluded.digest_mode,
          revision=forum_topic_subscriptions.revision+1,updated_at=CURRENT_TIMESTAMP
        WHERE forum_topic_subscriptions.level='normal'; END"#,
        "DROP TRIGGER IF EXISTS forum_70_auto_subscribe_reply_participant_update",
        r#"CREATE TRIGGER forum_70_auto_subscribe_reply_participant_update AFTER UPDATE OF status ON forum_replies
        FOR EACH ROW WHEN NEW.author_id IS NOT NULL AND OLD.status<>'approved' AND NEW.status='approved'
          AND COALESCE((SELECT auto_subscribe_reply_participants FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),1)=1
        BEGIN INSERT INTO forum_topic_subscriptions
        (topic_id,user_id,tenant_id,level,notify_mentions,notify_replies,notify_new_topics,digest_mode,last_notified_at,revision,created_at,updated_at)
        VALUES (NEW.topic_id,NEW.author_id,NEW.tenant_id,
        COALESCE((SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'tracking'),1,
        CASE COALESCE((SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'tracking') WHEN 'watching' THEN 1 ELSE 0 END,0,
        CASE COALESCE((SELECT reply_participant_level FROM forum_subscription_policies WHERE tenant_id=NEW.tenant_id),'tracking') WHEN 'watching' THEN 'immediate' ELSE 'disabled' END,
        NULL,1,CURRENT_TIMESTAMP,CURRENT_TIMESTAMP)
        ON CONFLICT(tenant_id,topic_id,user_id) DO UPDATE SET
          level=excluded.level,notify_mentions=excluded.notify_mentions,notify_replies=excluded.notify_replies,
          notify_new_topics=excluded.notify_new_topics,digest_mode=excluded.digest_mode,
          revision=forum_topic_subscriptions.revision+1,updated_at=CURRENT_TIMESTAMP
        WHERE forum_topic_subscriptions.level='normal'; END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
