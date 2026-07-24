use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn category_topic(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();

    for statement in [
        r##"DROP TRIGGER IF EXISTS forum_80_category_created_event"##,
        r##"CREATE TRIGGER forum_80_category_created_event
AFTER INSERT ON forum_categories
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'category', NEW.id,
        'forum.category.created', 1, NULL, json_object(
            'category_id', lower(hex(NEW.id)),
            'parent_id', CASE WHEN NEW.parent_id IS NULL THEN NULL ELSE lower(hex(NEW.parent_id)) END,
            'position', NEW.position,
            'moderated', NEW.moderated
        )
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_updated_event"##,
        r##"CREATE TRIGGER forum_80_category_updated_event
AFTER UPDATE ON forum_categories
FOR EACH ROW
WHEN OLD.parent_id IS NOT NEW.parent_id
  OR OLD.position IS NOT NEW.position
  OR OLD.icon IS NOT NEW.icon
  OR OLD.color IS NOT NEW.color
  OR OLD.moderated IS NOT NEW.moderated
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'category', NEW.id,
        'forum.category.updated', 1, NULL, json_object(
            'category_id', lower(hex(NEW.id)),
            'change_scope', 'category',
            'parent_id', CASE WHEN NEW.parent_id IS NULL THEN NULL ELSE lower(hex(NEW.parent_id)) END,
            'position', NEW.position,
            'moderated', NEW.moderated
        )
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_deleted_event"##,
        r##"CREATE TRIGGER forum_80_category_deleted_event
AFTER DELETE ON forum_categories
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        OLD.tenant_id, 'category', OLD.id,
        'forum.category.deleted', 1, NULL, json_object('category_id', lower(hex(OLD.id)))
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_translation_insert_event"##,
        r##"CREATE TRIGGER forum_80_category_translation_insert_event
AFTER INSERT ON forum_category_translations
FOR EACH ROW
WHEN (
    SELECT COUNT(*)
    FROM forum_category_translations
    WHERE tenant_id = NEW.tenant_id
      AND category_id = NEW.category_id
) > 1
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'category', NEW.category_id,
        'forum.category.updated', 1, NULL, json_object('category_id', lower(hex(NEW.category_id)), 'change_scope', 'translation', 'locale', NEW.locale)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_category_translation_update_event"##,
        r##"CREATE TRIGGER forum_80_category_translation_update_event
AFTER UPDATE ON forum_category_translations
FOR EACH ROW
WHEN OLD.name IS NOT NEW.name
  OR OLD.slug IS NOT NEW.slug
  OR OLD.description IS NOT NEW.description
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'category', NEW.category_id,
        'forum.category.updated', 1, NULL, json_object('category_id', lower(hex(NEW.category_id)), 'change_scope', 'translation', 'locale', NEW.locale)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_created_event"##,
        r##"CREATE TRIGGER forum_80_topic_created_event
AFTER INSERT ON forum_topics
FOR EACH ROW
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.id,
        'forum.topic.created', 1, NEW.author_id, json_object(
            'topic_id', lower(hex(NEW.id)),
            'category_id', lower(hex(NEW.category_id)),
            'author_id', CASE WHEN NEW.author_id IS NULL THEN NULL ELSE lower(hex(NEW.author_id)) END,
            'status', NEW.status
        )
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_updated_event"##,
        r##"CREATE TRIGGER forum_80_topic_updated_event
AFTER UPDATE ON forum_topics
FOR EACH ROW
WHEN OLD.category_id IS NOT NEW.category_id
  OR OLD.metadata IS NOT NEW.metadata
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.id,
        'forum.topic.updated', 1, NULL, json_object('topic_id', lower(hex(NEW.id)), 'change_scope', 'topic', 'category_id', lower(hex(NEW.category_id)))
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_status_event"##,
        r##"CREATE TRIGGER forum_80_topic_status_event
AFTER UPDATE OF status ON forum_topics
FOR EACH ROW
WHEN OLD.status IS NOT NEW.status
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.id,
        'forum.topic.status_changed', 1, NULL, json_object('topic_id', lower(hex(NEW.id)), 'old_status', OLD.status, 'new_status', NEW.status)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_pinned_event"##,
        r##"CREATE TRIGGER forum_80_topic_pinned_event
AFTER UPDATE OF is_pinned ON forum_topics
FOR EACH ROW
WHEN OLD.is_pinned IS NOT NEW.is_pinned
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.id,
        'forum.topic.pinned_changed', 1, NULL, json_object('topic_id', lower(hex(NEW.id)), 'is_pinned', NEW.is_pinned)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_lock_event"##,
        r##"CREATE TRIGGER forum_80_topic_lock_event
AFTER UPDATE OF is_locked ON forum_topics
FOR EACH ROW
WHEN OLD.is_locked IS NOT NEW.is_locked
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.id,
        'forum.topic.lock_changed', 1, NULL, json_object('topic_id', lower(hex(NEW.id)), 'is_locked', NEW.is_locked)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_deleted_event"##,
        r##"CREATE TRIGGER forum_80_topic_deleted_event
AFTER UPDATE OF deleted_at ON forum_topics
FOR EACH ROW
WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.id,
        'forum.topic.deleted', 1, NULL, json_object('topic_id', lower(hex(NEW.id)), 'deleted_at', NEW.deleted_at)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_translation_insert_event"##,
        r##"CREATE TRIGGER forum_80_topic_translation_insert_event
AFTER INSERT ON forum_topic_translations
FOR EACH ROW
WHEN NOT (NEW.title = '[deleted]' AND NEW.body = '[deleted]')
 AND (
    SELECT COUNT(*)
    FROM forum_topic_translations
    WHERE tenant_id = NEW.tenant_id
      AND topic_id = NEW.topic_id
 ) > 1
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.topic.updated', 1, NULL, json_object('topic_id', lower(hex(NEW.topic_id)), 'change_scope', 'translation', 'locale', NEW.locale)
    );
END"##,
        r##"DROP TRIGGER IF EXISTS forum_80_topic_translation_update_event"##,
        r##"CREATE TRIGGER forum_80_topic_translation_update_event
AFTER UPDATE ON forum_topic_translations
FOR EACH ROW
WHEN NOT (NEW.title = '[deleted]' AND NEW.body = '[deleted]')
 AND (
    OLD.title IS NOT NEW.title
    OR OLD.slug IS NOT NEW.slug
    OR OLD.body IS NOT NEW.body
    OR OLD.body_format IS NOT NEW.body_format
 )
BEGIN
INSERT INTO forum_domain_events (
        event_id, tenant_id, aggregate_type, aggregate_id,
        event_type, schema_version, actor_id, payload
    ) VALUES (
        lower(hex(randomblob(4))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(2))) || '-' ||
               lower(hex(randomblob(6))),
        NEW.tenant_id, 'topic', NEW.topic_id,
        'forum.topic.updated', 1, NULL, json_object('topic_id', lower(hex(NEW.topic_id)), 'change_scope', 'translation', 'locale', NEW.locale)
    );
END"##,
    ] {
        connection.execute_unprepared(statement).await?;
    }

    Ok(())
}
