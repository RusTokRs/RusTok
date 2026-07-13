use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn apply(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    for statement in [
        "DROP TRIGGER IF EXISTS forum_validate_category_subscription_insert",
        r#"CREATE TRIGGER forum_validate_category_subscription_insert BEFORE INSERT ON forum_category_subscriptions
        FOR EACH ROW WHEN NEW.level NOT IN ('watching','tracking','normal','muted')
          OR NEW.digest_mode NOT IN ('immediate','daily','weekly','disabled') OR NEW.revision <= 0 OR NEW.updated_at IS NULL
          OR (NEW.level='muted' AND (NEW.notify_mentions<>0 OR NEW.notify_replies<>0 OR NEW.notify_new_topics<>0 OR NEW.digest_mode<>'disabled'))
        BEGIN SELECT RAISE(ABORT, 'invalid forum category subscription settings'); END"#,
        "DROP TRIGGER IF EXISTS forum_validate_category_subscription_update",
        r#"CREATE TRIGGER forum_validate_category_subscription_update BEFORE UPDATE ON forum_category_subscriptions
        FOR EACH ROW WHEN NEW.level NOT IN ('watching','tracking','normal','muted')
          OR NEW.digest_mode NOT IN ('immediate','daily','weekly','disabled') OR NEW.revision <> OLD.revision + 1
          OR NEW.updated_at IS NULL OR (NEW.level='muted' AND (NEW.notify_mentions<>0 OR NEW.notify_replies<>0 OR NEW.notify_new_topics<>0 OR NEW.digest_mode<>'disabled'))
        BEGIN SELECT RAISE(ABORT, 'invalid forum category subscription settings'); END"#,
        "DROP TRIGGER IF EXISTS forum_validate_topic_subscription_insert",
        r#"CREATE TRIGGER forum_validate_topic_subscription_insert BEFORE INSERT ON forum_topic_subscriptions
        FOR EACH ROW WHEN NEW.level NOT IN ('watching','tracking','normal','muted')
          OR NEW.digest_mode NOT IN ('immediate','daily','weekly','disabled') OR NEW.revision <= 0 OR NEW.updated_at IS NULL
          OR (NEW.level='muted' AND (NEW.notify_mentions<>0 OR NEW.notify_replies<>0 OR NEW.notify_new_topics<>0 OR NEW.digest_mode<>'disabled'))
        BEGIN SELECT RAISE(ABORT, 'invalid forum topic subscription settings'); END"#,
        "DROP TRIGGER IF EXISTS forum_validate_topic_subscription_update",
        r#"CREATE TRIGGER forum_validate_topic_subscription_update BEFORE UPDATE ON forum_topic_subscriptions
        FOR EACH ROW WHEN NEW.level NOT IN ('watching','tracking','normal','muted')
          OR NEW.digest_mode NOT IN ('immediate','daily','weekly','disabled') OR NEW.revision <> OLD.revision + 1
          OR NEW.updated_at IS NULL OR (NEW.level='muted' AND (NEW.notify_mentions<>0 OR NEW.notify_replies<>0 OR NEW.notify_new_topics<>0 OR NEW.digest_mode<>'disabled'))
        BEGIN SELECT RAISE(ABORT, 'invalid forum topic subscription settings'); END"#,
        "DROP TRIGGER IF EXISTS forum_validate_subscription_policy",
        r#"CREATE TRIGGER forum_validate_subscription_policy BEFORE INSERT ON forum_subscription_policies
        FOR EACH ROW WHEN NEW.topic_author_level NOT IN ('watching','tracking','normal')
          OR NEW.reply_participant_level NOT IN ('watching','tracking','normal') OR NEW.revision <= 0
        BEGIN SELECT RAISE(ABORT, 'invalid forum subscription policy'); END"#,
        "DROP TRIGGER IF EXISTS forum_validate_subscription_policy_update",
        r#"CREATE TRIGGER forum_validate_subscription_policy_update BEFORE UPDATE ON forum_subscription_policies
        FOR EACH ROW WHEN NEW.topic_author_level NOT IN ('watching','tracking','normal')
          OR NEW.reply_participant_level NOT IN ('watching','tracking','normal') OR NEW.revision <> OLD.revision + 1
        BEGIN SELECT RAISE(ABORT, 'invalid forum subscription policy revision'); END"#,
    ] {
        connection.execute_unprepared(statement).await?;
    }
    Ok(())
}
