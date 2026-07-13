use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn content(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_emit_category_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'category',
            NEW.id,
            'forum.category.created',
            NULL,
            jsonb_build_object(
                'category_id', NEW.id,
                'parent_id', NEW.parent_id,
                'position', NEW.position,
                'moderated', NEW.moderated
            )
        );
        RETURN NEW;
    ELSIF TG_OP = 'DELETE' THEN
        PERFORM forum_append_domain_event(
            OLD.tenant_id,
            'category',
            OLD.id,
            'forum.category.deleted',
            NULL,
            jsonb_build_object('category_id', OLD.id)
        );
        RETURN OLD;
    END IF;

    IF ROW(OLD.parent_id, OLD.position, OLD.icon, OLD.color, OLD.moderated)
       IS DISTINCT FROM
       ROW(NEW.parent_id, NEW.position, NEW.icon, NEW.color, NEW.moderated)
    THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'category',
            NEW.id,
            'forum.category.updated',
            NULL,
            jsonb_build_object(
                'category_id', NEW.id,
                'change_scope', 'category',
                'parent_id', NEW.parent_id,
                'position', NEW.position,
                'moderated', NEW.moderated
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_category_translation_event()
RETURNS trigger AS $$
DECLARE
    translation_count bigint;
BEGIN
    IF TG_OP = 'INSERT' THEN
        SELECT COUNT(*) INTO translation_count
        FROM forum_category_translations
        WHERE tenant_id = NEW.tenant_id
          AND category_id = NEW.category_id;
        IF translation_count > 1 THEN
            PERFORM forum_append_domain_event(
                NEW.tenant_id,
                'category',
                NEW.category_id,
                'forum.category.updated',
                NULL,
                jsonb_build_object(
                    'category_id', NEW.category_id,
                    'change_scope', 'translation',
                    'locale', NEW.locale
                )
            );
        END IF;
        RETURN NEW;
    END IF;

    IF ROW(OLD.name, OLD.slug, OLD.description)
       IS DISTINCT FROM
       ROW(NEW.name, NEW.slug, NEW.description)
    THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'category',
            NEW.category_id,
            'forum.category.updated',
            NULL,
            jsonb_build_object(
                'category_id', NEW.category_id,
                'change_scope', 'translation',
                'locale', NEW.locale
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.id,
            'forum.topic.created',
            NEW.author_id,
            jsonb_build_object(
                'topic_id', NEW.id,
                'category_id', NEW.category_id,
                'author_id', NEW.author_id,
                'status', NEW.status
            )
        );
        RETURN NEW;
    END IF;

    IF OLD.category_id IS DISTINCT FROM NEW.category_id
       OR OLD.metadata IS DISTINCT FROM NEW.metadata
    THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.id,
            'forum.topic.updated',
            NULL,
            jsonb_build_object(
                'topic_id', NEW.id,
                'change_scope', 'topic',
                'category_id', NEW.category_id
            )
        );
    END IF;

    IF OLD.status IS DISTINCT FROM NEW.status THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.id,
            'forum.topic.status_changed',
            NULL,
            jsonb_build_object(
                'topic_id', NEW.id,
                'old_status', OLD.status,
                'new_status', NEW.status
            )
        );
    END IF;

    IF OLD.is_pinned IS DISTINCT FROM NEW.is_pinned THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.id,
            'forum.topic.pinned_changed',
            NULL,
            jsonb_build_object(
                'topic_id', NEW.id,
                'is_pinned', NEW.is_pinned
            )
        );
    END IF;

    IF OLD.is_locked IS DISTINCT FROM NEW.is_locked THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.id,
            'forum.topic.lock_changed',
            NULL,
            jsonb_build_object(
                'topic_id', NEW.id,
                'is_locked', NEW.is_locked
            )
        );
    END IF;

    IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.id,
            'forum.topic.deleted',
            NULL,
            jsonb_build_object(
                'topic_id', NEW.id,
                'deleted_at', NEW.deleted_at
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_translation_event()
RETURNS trigger AS $$
DECLARE
    translation_count bigint;
BEGIN
    IF NEW.title = '[deleted]' AND NEW.body = '[deleted]' THEN
        RETURN NEW;
    END IF;

    IF TG_OP = 'INSERT' THEN
        SELECT COUNT(*) INTO translation_count
        FROM forum_topic_translations
        WHERE tenant_id = NEW.tenant_id
          AND topic_id = NEW.topic_id;
        IF translation_count > 1 THEN
            PERFORM forum_append_domain_event(
                NEW.tenant_id,
                'topic',
                NEW.topic_id,
                'forum.topic.updated',
                NULL,
                jsonb_build_object(
                    'topic_id', NEW.topic_id,
                    'change_scope', 'translation',
                    'locale', NEW.locale
                )
            );
        END IF;
        RETURN NEW;
    END IF;

    IF ROW(OLD.title, OLD.slug, OLD.body, OLD.body_format)
       IS DISTINCT FROM
       ROW(NEW.title, NEW.slug, NEW.body, NEW.body_format)
    THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.topic_id,
            'forum.topic.updated',
            NULL,
            jsonb_build_object(
                'topic_id', NEW.topic_id,
                'change_scope', 'translation',
                'locale', NEW.locale
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_reply_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'reply',
            NEW.id,
            'forum.reply.created',
            NEW.author_id,
            jsonb_build_object(
                'reply_id', NEW.id,
                'topic_id', NEW.topic_id,
                'author_id', NEW.author_id,
                'parent_reply_id', NEW.parent_reply_id,
                'status', NEW.status,
                'position', NEW.position
            )
        );
        RETURN NEW;
    END IF;

    IF OLD.status IS DISTINCT FROM NEW.status THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'reply',
            NEW.id,
            'forum.reply.status_changed',
            NULL,
            jsonb_build_object(
                'reply_id', NEW.id,
                'topic_id', NEW.topic_id,
                'old_status', OLD.status,
                'new_status', NEW.status
            )
        );
    END IF;

    IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'reply',
            NEW.id,
            'forum.reply.deleted',
            NULL,
            jsonb_build_object(
                'reply_id', NEW.id,
                'topic_id', NEW.topic_id,
                'deleted_at', NEW.deleted_at
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_reply_body_event()
RETURNS trigger AS $$
DECLARE
    body_count bigint;
BEGIN
    IF NEW.body = '[deleted]' THEN
        RETURN NEW;
    END IF;

    IF TG_OP = 'INSERT' THEN
        SELECT COUNT(*) INTO body_count
        FROM forum_reply_bodies
        WHERE tenant_id = NEW.tenant_id
          AND reply_id = NEW.reply_id;
        IF body_count > 1 THEN
            PERFORM forum_append_domain_event(
                NEW.tenant_id,
                'reply',
                NEW.reply_id,
                'forum.reply.updated',
                NULL,
                jsonb_build_object(
                    'reply_id', NEW.reply_id,
                    'change_scope', 'body',
                    'locale', NEW.locale
                )
            );
        END IF;
        RETURN NEW;
    END IF;

    IF ROW(OLD.body, OLD.body_format)
       IS DISTINCT FROM
       ROW(NEW.body, NEW.body_format)
    THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'reply',
            NEW.reply_id,
            'forum.reply.updated',
            NULL,
            jsonb_build_object(
                'reply_id', NEW.reply_id,
                'change_scope', 'body',
                'locale', NEW.locale
            )
        );
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#,
        )
        .await?;
    Ok(())
}
