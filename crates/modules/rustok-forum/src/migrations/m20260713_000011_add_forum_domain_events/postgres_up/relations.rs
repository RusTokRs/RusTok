use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::ConnectionTrait;

pub(super) async fn relations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE OR REPLACE FUNCTION forum_emit_solution_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id,
            'topic',
            NEW.topic_id,
            'forum.solution.marked',
            NEW.marked_by_user_id,
            jsonb_build_object(
                'topic_id', NEW.topic_id,
                'reply_id', NEW.reply_id,
                'marked_by_user_id', NEW.marked_by_user_id
            )
        );
        RETURN NEW;
    END IF;

    PERFORM forum_append_domain_event(
        OLD.tenant_id,
        'topic',
        OLD.topic_id,
        'forum.solution.unmarked',
        OLD.marked_by_user_id,
        jsonb_build_object(
            'topic_id', OLD.topic_id,
            'reply_id', OLD.reply_id,
            'marked_by_user_id', OLD.marked_by_user_id
        )
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_vote_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'topic', NEW.topic_id,
            'forum.topic.vote_changed', NEW.user_id,
            jsonb_build_object(
                'topic_id', NEW.topic_id,
                'user_id', NEW.user_id,
                'previous_value', NULL,
                'value', NEW.value
            )
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        IF OLD.value IS DISTINCT FROM NEW.value THEN
            PERFORM forum_append_domain_event(
                NEW.tenant_id, 'topic', NEW.topic_id,
                'forum.topic.vote_changed', NEW.user_id,
                jsonb_build_object(
                    'topic_id', NEW.topic_id,
                    'user_id', NEW.user_id,
                    'previous_value', OLD.value,
                    'value', NEW.value
                )
            );
        END IF;
        RETURN NEW;
    END IF;

    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.vote_changed', OLD.user_id,
        jsonb_build_object(
            'topic_id', OLD.topic_id,
            'user_id', OLD.user_id,
            'previous_value', OLD.value,
            'value', NULL
        )
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_reply_vote_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'reply', NEW.reply_id,
            'forum.reply.vote_changed', NEW.user_id,
            jsonb_build_object(
                'reply_id', NEW.reply_id,
                'user_id', NEW.user_id,
                'previous_value', NULL,
                'value', NEW.value
            )
        );
        RETURN NEW;
    ELSIF TG_OP = 'UPDATE' THEN
        IF OLD.value IS DISTINCT FROM NEW.value THEN
            PERFORM forum_append_domain_event(
                NEW.tenant_id, 'reply', NEW.reply_id,
                'forum.reply.vote_changed', NEW.user_id,
                jsonb_build_object(
                    'reply_id', NEW.reply_id,
                    'user_id', NEW.user_id,
                    'previous_value', OLD.value,
                    'value', NEW.value
                )
            );
        END IF;
        RETURN NEW;
    END IF;

    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'reply', OLD.reply_id,
        'forum.reply.vote_changed', OLD.user_id,
        jsonb_build_object(
            'reply_id', OLD.reply_id,
            'user_id', OLD.user_id,
            'previous_value', OLD.value,
            'value', NULL
        )
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_category_subscription_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'category', NEW.category_id,
            'forum.category.subscription_changed', NEW.user_id,
            jsonb_build_object(
                'category_id', NEW.category_id,
                'user_id', NEW.user_id,
                'subscribed', TRUE
            )
        );
        RETURN NEW;
    END IF;

    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'category', OLD.category_id,
        'forum.category.subscription_changed', OLD.user_id,
        jsonb_build_object(
            'category_id', OLD.category_id,
            'user_id', OLD.user_id,
            'subscribed', FALSE
        )
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_subscription_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'topic', NEW.topic_id,
            'forum.topic.subscription_changed', NEW.user_id,
            jsonb_build_object(
                'topic_id', NEW.topic_id,
                'user_id', NEW.user_id,
                'subscribed', TRUE
            )
        );
        RETURN NEW;
    END IF;

    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.subscription_changed', OLD.user_id,
        jsonb_build_object(
            'topic_id', OLD.topic_id,
            'user_id', OLD.user_id,
            'subscribed', FALSE
        )
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION forum_emit_topic_tag_event()
RETURNS trigger AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        PERFORM forum_append_domain_event(
            NEW.tenant_id, 'topic', NEW.topic_id,
            'forum.topic.tags_changed', NULL,
            jsonb_build_object(
                'topic_id', NEW.topic_id,
                'term_id', NEW.term_id,
                'attached', TRUE
            )
        );
        RETURN NEW;
    END IF;

    PERFORM forum_append_domain_event(
        OLD.tenant_id, 'topic', OLD.topic_id,
        'forum.topic.tags_changed', NULL,
        jsonb_build_object(
            'topic_id', OLD.topic_id,
            'term_id', OLD.term_id,
            'attached', FALSE
        )
    );
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS forum_80_category_events ON forum_categories;
CREATE TRIGGER forum_80_category_events
AFTER INSERT OR UPDATE OR DELETE ON forum_categories
FOR EACH ROW EXECUTE FUNCTION forum_emit_category_event();

DROP TRIGGER IF EXISTS forum_80_category_translation_events ON forum_category_translations;
CREATE TRIGGER forum_80_category_translation_events
AFTER INSERT OR UPDATE ON forum_category_translations
FOR EACH ROW EXECUTE FUNCTION forum_emit_category_translation_event();

DROP TRIGGER IF EXISTS forum_80_topic_events ON forum_topics;
CREATE TRIGGER forum_80_topic_events
AFTER INSERT OR UPDATE ON forum_topics
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_event();

DROP TRIGGER IF EXISTS forum_80_topic_translation_events ON forum_topic_translations;
CREATE TRIGGER forum_80_topic_translation_events
AFTER INSERT OR UPDATE ON forum_topic_translations
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_translation_event();

DROP TRIGGER IF EXISTS forum_80_reply_events ON forum_replies;
CREATE TRIGGER forum_80_reply_events
AFTER INSERT OR UPDATE ON forum_replies
FOR EACH ROW EXECUTE FUNCTION forum_emit_reply_event();

DROP TRIGGER IF EXISTS forum_80_reply_body_events ON forum_reply_bodies;
CREATE TRIGGER forum_80_reply_body_events
AFTER INSERT OR UPDATE ON forum_reply_bodies
FOR EACH ROW EXECUTE FUNCTION forum_emit_reply_body_event();

DROP TRIGGER IF EXISTS forum_80_solution_events ON forum_solutions;
CREATE TRIGGER forum_80_solution_events
AFTER INSERT OR DELETE ON forum_solutions
FOR EACH ROW EXECUTE FUNCTION forum_emit_solution_event();

DROP TRIGGER IF EXISTS forum_80_topic_vote_events ON forum_topic_votes;
CREATE TRIGGER forum_80_topic_vote_events
AFTER INSERT OR UPDATE OR DELETE ON forum_topic_votes
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_vote_event();

DROP TRIGGER IF EXISTS forum_80_reply_vote_events ON forum_reply_votes;
CREATE TRIGGER forum_80_reply_vote_events
AFTER INSERT OR UPDATE OR DELETE ON forum_reply_votes
FOR EACH ROW EXECUTE FUNCTION forum_emit_reply_vote_event();

DROP TRIGGER IF EXISTS forum_80_category_subscription_events ON forum_category_subscriptions;
CREATE TRIGGER forum_80_category_subscription_events
AFTER INSERT OR DELETE ON forum_category_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_emit_category_subscription_event();

DROP TRIGGER IF EXISTS forum_80_topic_subscription_events ON forum_topic_subscriptions;
CREATE TRIGGER forum_80_topic_subscription_events
AFTER INSERT OR DELETE ON forum_topic_subscriptions
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_subscription_event();

DROP TRIGGER IF EXISTS forum_80_topic_tag_events ON forum_topic_tags;
CREATE TRIGGER forum_80_topic_tag_events
AFTER INSERT OR DELETE ON forum_topic_tags
FOR EACH ROW EXECUTE FUNCTION forum_emit_topic_tag_event();
"#,
        )
        .await?;
    Ok(())
}
