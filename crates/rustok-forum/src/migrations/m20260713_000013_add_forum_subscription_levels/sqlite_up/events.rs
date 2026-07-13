use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_80_category_subscription_insert_event",
        "DROP TRIGGER IF EXISTS forum_80_category_subscription_update_event",
        "DROP TRIGGER IF EXISTS forum_80_category_subscription_delete_event",
        "DROP TRIGGER IF EXISTS forum_80_topic_subscription_insert_event",
        "DROP TRIGGER IF EXISTS forum_80_topic_subscription_update_event",
        "DROP TRIGGER IF EXISTS forum_80_topic_subscription_delete_event",
        r#"CREATE TRIGGER forum_80_category_subscription_insert_event AFTER INSERT ON forum_category_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        NEW.tenant_id,'category',NEW.category_id,'forum.subscription.changed.v1',1,NEW.user_id,
        json_object('target_type','category','target_id',NEW.category_id,'user_id',NEW.user_id,'previous_level','normal','level',NEW.level,
        'notify_mentions',NEW.notify_mentions,'notify_replies',NEW.notify_replies,'notify_new_topics',NEW.notify_new_topics,
        'digest_mode',NEW.digest_mode,'revision',NEW.revision)); END"#,
        r#"CREATE TRIGGER forum_80_category_subscription_update_event AFTER UPDATE ON forum_category_subscriptions
        FOR EACH ROW WHEN OLD.level IS NOT NEW.level OR OLD.notify_mentions IS NOT NEW.notify_mentions
          OR OLD.notify_replies IS NOT NEW.notify_replies OR OLD.notify_new_topics IS NOT NEW.notify_new_topics
          OR OLD.digest_mode IS NOT NEW.digest_mode OR OLD.revision IS NOT NEW.revision
        BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        NEW.tenant_id,'category',NEW.category_id,'forum.subscription.changed.v1',1,NEW.user_id,
        json_object('target_type','category','target_id',NEW.category_id,'user_id',NEW.user_id,'previous_level',OLD.level,'level',NEW.level,
        'notify_mentions',NEW.notify_mentions,'notify_replies',NEW.notify_replies,'notify_new_topics',NEW.notify_new_topics,
        'digest_mode',NEW.digest_mode,'revision',NEW.revision)); END"#,
        r#"CREATE TRIGGER forum_80_category_subscription_delete_event AFTER DELETE ON forum_category_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        OLD.tenant_id,'category',OLD.category_id,'forum.subscription.changed.v1',1,OLD.user_id,
        json_object('target_type','category','target_id',OLD.category_id,'user_id',OLD.user_id,'previous_level',OLD.level,'level','normal',
        'notify_mentions',1,'notify_replies',0,'notify_new_topics',0,'digest_mode','disabled','revision',OLD.revision+1)); END"#,
        r#"CREATE TRIGGER forum_80_topic_subscription_insert_event AFTER INSERT ON forum_topic_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        NEW.tenant_id,'topic',NEW.topic_id,'forum.subscription.changed.v1',1,NEW.user_id,
        json_object('target_type','topic','target_id',NEW.topic_id,'user_id',NEW.user_id,'previous_level','normal','level',NEW.level,
        'notify_mentions',NEW.notify_mentions,'notify_replies',NEW.notify_replies,'notify_new_topics',NEW.notify_new_topics,
        'digest_mode',NEW.digest_mode,'revision',NEW.revision)); END"#,
        r#"CREATE TRIGGER forum_80_topic_subscription_update_event AFTER UPDATE ON forum_topic_subscriptions
        FOR EACH ROW WHEN OLD.level IS NOT NEW.level OR OLD.notify_mentions IS NOT NEW.notify_mentions
          OR OLD.notify_replies IS NOT NEW.notify_replies OR OLD.notify_new_topics IS NOT NEW.notify_new_topics
          OR OLD.digest_mode IS NOT NEW.digest_mode OR OLD.revision IS NOT NEW.revision
        BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        NEW.tenant_id,'topic',NEW.topic_id,'forum.subscription.changed.v1',1,NEW.user_id,
        json_object('target_type','topic','target_id',NEW.topic_id,'user_id',NEW.user_id,'previous_level',OLD.level,'level',NEW.level,
        'notify_mentions',NEW.notify_mentions,'notify_replies',NEW.notify_replies,'notify_new_topics',NEW.notify_new_topics,
        'digest_mode',NEW.digest_mode,'revision',NEW.revision)); END"#,
        r#"CREATE TRIGGER forum_80_topic_subscription_delete_event AFTER DELETE ON forum_topic_subscriptions
        FOR EACH ROW BEGIN INSERT INTO forum_domain_events
        (event_id,tenant_id,aggregate_type,aggregate_id,event_type,schema_version,actor_id,payload)
        VALUES (lower(hex(randomblob(4)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(2)))||'-'||lower(hex(randomblob(6))),
        OLD.tenant_id,'topic',OLD.topic_id,'forum.subscription.changed.v1',1,OLD.user_id,
        json_object('target_type','topic','target_id',OLD.topic_id,'user_id',OLD.user_id,'previous_level',OLD.level,'level','normal',
        'notify_mentions',1,'notify_replies',0,'notify_new_topics',0,'digest_mode','disabled','revision',OLD.revision+1)); END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
