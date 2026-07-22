use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => up_postgres(manager).await,
            DatabaseBackend::Sqlite => up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum relation tenant migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down_postgres(manager).await,
            DatabaseBackend::Sqlite => down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum relation tenant migration does not support {backend:?}"
            ))),
        }
    }
}

async fn up_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM forum_topic_votes vote
        WHERE NOT EXISTS (
            SELECT 1 FROM forum_topics topic
            WHERE topic.id = vote.topic_id
              AND topic.tenant_id = vote.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'forum relation migration blocked: invalid topic vote tenant';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_reply_votes vote
        WHERE NOT EXISTS (
            SELECT 1 FROM forum_replies reply
            WHERE reply.id = vote.reply_id
              AND reply.tenant_id = vote.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'forum relation migration blocked: invalid reply vote tenant';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_category_subscriptions subscription
        WHERE NOT EXISTS (
            SELECT 1 FROM forum_categories category
            WHERE category.id = subscription.category_id
              AND category.tenant_id = subscription.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'forum relation migration blocked: invalid category subscription tenant';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_topic_subscriptions subscription
        WHERE NOT EXISTS (
            SELECT 1 FROM forum_topics topic
            WHERE topic.id = subscription.topic_id
              AND topic.tenant_id = subscription.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'forum relation migration blocked: invalid topic subscription tenant';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_solutions solution
        WHERE NOT EXISTS (
            SELECT 1 FROM forum_topics topic
            WHERE topic.id = solution.topic_id
              AND topic.tenant_id = solution.tenant_id
        ) OR NOT EXISTS (
            SELECT 1 FROM forum_replies reply
            WHERE reply.id = solution.reply_id
              AND reply.topic_id = solution.topic_id
              AND reply.tenant_id = solution.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'forum relation migration blocked: invalid solution relation';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM forum_topic_tags tag
        WHERE NOT EXISTS (
            SELECT 1 FROM forum_topics topic
            WHERE topic.id = tag.topic_id
              AND topic.tenant_id = tag.tenant_id
        ) OR NOT EXISTS (
            SELECT 1 FROM taxonomy_terms term
            WHERE term.id = tag.term_id
              AND term.tenant_id = tag.tenant_id
        )
    ) THEN
        RAISE EXCEPTION 'forum relation migration blocked: invalid topic tag tenant';
    END IF;
END $$;

ALTER TABLE forum_topic_votes
    DROP CONSTRAINT IF EXISTS fk_forum_topic_votes_topic;
ALTER TABLE forum_reply_votes
    DROP CONSTRAINT IF EXISTS fk_forum_reply_votes_reply;
ALTER TABLE forum_category_subscriptions
    DROP CONSTRAINT IF EXISTS fk_forum_category_subscriptions_category;
ALTER TABLE forum_topic_subscriptions
    DROP CONSTRAINT IF EXISTS fk_forum_topic_subscriptions_topic;
ALTER TABLE forum_solutions
    DROP CONSTRAINT IF EXISTS fk_forum_solutions_topic;
ALTER TABLE forum_solutions
    DROP CONSTRAINT IF EXISTS fk_forum_solutions_reply;
ALTER TABLE forum_topic_tags
    DROP CONSTRAINT IF EXISTS fk_forum_topic_tags_topic;
ALTER TABLE forum_topic_tags
    DROP CONSTRAINT IF EXISTS fk_forum_topic_tags_term;

ALTER TABLE forum_topic_votes
    DROP CONSTRAINT IF EXISTS forum_topic_votes_pkey;
ALTER TABLE forum_reply_votes
    DROP CONSTRAINT IF EXISTS forum_reply_votes_pkey;
ALTER TABLE forum_category_subscriptions
    DROP CONSTRAINT IF EXISTS forum_category_subscriptions_pkey;
ALTER TABLE forum_topic_subscriptions
    DROP CONSTRAINT IF EXISTS forum_topic_subscriptions_pkey;
ALTER TABLE forum_solutions
    DROP CONSTRAINT IF EXISTS forum_solutions_pkey;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conname = 'uq_forum_replies_tenant_topic_id'
    ) THEN
        ALTER TABLE forum_replies
            ADD CONSTRAINT uq_forum_replies_tenant_topic_id
            UNIQUE (tenant_id, topic_id, id);
    END IF;
END $$;

ALTER TABLE forum_topic_votes
    ADD CONSTRAINT forum_topic_votes_pkey
    PRIMARY KEY (tenant_id, topic_id, user_id);
ALTER TABLE forum_reply_votes
    ADD CONSTRAINT forum_reply_votes_pkey
    PRIMARY KEY (tenant_id, reply_id, user_id);
ALTER TABLE forum_category_subscriptions
    ADD CONSTRAINT forum_category_subscriptions_pkey
    PRIMARY KEY (tenant_id, category_id, user_id);
ALTER TABLE forum_topic_subscriptions
    ADD CONSTRAINT forum_topic_subscriptions_pkey
    PRIMARY KEY (tenant_id, topic_id, user_id);
ALTER TABLE forum_solutions
    ADD CONSTRAINT forum_solutions_pkey
    PRIMARY KEY (tenant_id, topic_id);

ALTER TABLE forum_topic_votes
    ADD CONSTRAINT fk_forum_topic_votes_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_reply_votes
    ADD CONSTRAINT fk_forum_reply_votes_reply_tenant
    FOREIGN KEY (tenant_id, reply_id)
    REFERENCES forum_replies (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_category_subscriptions
    ADD CONSTRAINT fk_forum_category_subscriptions_category_tenant
    FOREIGN KEY (tenant_id, category_id)
    REFERENCES forum_categories (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_subscriptions
    ADD CONSTRAINT fk_forum_topic_subscriptions_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_solutions
    ADD CONSTRAINT fk_forum_solutions_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_solutions
    ADD CONSTRAINT fk_forum_solutions_reply_topic_tenant
    FOREIGN KEY (tenant_id, topic_id, reply_id)
    REFERENCES forum_replies (tenant_id, topic_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_tags
    ADD CONSTRAINT fk_forum_topic_tags_topic_tenant
    FOREIGN KEY (tenant_id, topic_id)
    REFERENCES forum_topics (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_tags
    ADD CONSTRAINT fk_forum_topic_tags_term_tenant
    FOREIGN KEY (tenant_id, term_id)
    REFERENCES taxonomy_terms (tenant_id, id)
    ON UPDATE CASCADE ON DELETE CASCADE;

DROP INDEX IF EXISTS idx_forum_solutions_reply_unique;
CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_solutions_tenant_reply
    ON forum_solutions (tenant_id, reply_id);

DROP INDEX IF EXISTS idx_forum_topic_tags_topic_term;
CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_topic_tags_tenant_topic_term
    ON forum_topic_tags (tenant_id, topic_id, term_id);
"#,
        )
        .await?;
    Ok(())
}

async fn down_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE forum_topic_votes
    DROP CONSTRAINT IF EXISTS fk_forum_topic_votes_topic_tenant;
ALTER TABLE forum_reply_votes
    DROP CONSTRAINT IF EXISTS fk_forum_reply_votes_reply_tenant;
ALTER TABLE forum_category_subscriptions
    DROP CONSTRAINT IF EXISTS fk_forum_category_subscriptions_category_tenant;
ALTER TABLE forum_topic_subscriptions
    DROP CONSTRAINT IF EXISTS fk_forum_topic_subscriptions_topic_tenant;
ALTER TABLE forum_solutions
    DROP CONSTRAINT IF EXISTS fk_forum_solutions_topic_tenant;
ALTER TABLE forum_solutions
    DROP CONSTRAINT IF EXISTS fk_forum_solutions_reply_topic_tenant;
ALTER TABLE forum_topic_tags
    DROP CONSTRAINT IF EXISTS fk_forum_topic_tags_topic_tenant;
ALTER TABLE forum_topic_tags
    DROP CONSTRAINT IF EXISTS fk_forum_topic_tags_term_tenant;

ALTER TABLE forum_topic_votes
    DROP CONSTRAINT IF EXISTS forum_topic_votes_pkey;
ALTER TABLE forum_reply_votes
    DROP CONSTRAINT IF EXISTS forum_reply_votes_pkey;
ALTER TABLE forum_category_subscriptions
    DROP CONSTRAINT IF EXISTS forum_category_subscriptions_pkey;
ALTER TABLE forum_topic_subscriptions
    DROP CONSTRAINT IF EXISTS forum_topic_subscriptions_pkey;
ALTER TABLE forum_solutions
    DROP CONSTRAINT IF EXISTS forum_solutions_pkey;

ALTER TABLE forum_topic_votes
    ADD CONSTRAINT forum_topic_votes_pkey PRIMARY KEY (topic_id, user_id);
ALTER TABLE forum_reply_votes
    ADD CONSTRAINT forum_reply_votes_pkey PRIMARY KEY (reply_id, user_id);
ALTER TABLE forum_category_subscriptions
    ADD CONSTRAINT forum_category_subscriptions_pkey PRIMARY KEY (category_id, user_id);
ALTER TABLE forum_topic_subscriptions
    ADD CONSTRAINT forum_topic_subscriptions_pkey PRIMARY KEY (topic_id, user_id);
ALTER TABLE forum_solutions
    ADD CONSTRAINT forum_solutions_pkey PRIMARY KEY (topic_id);

ALTER TABLE forum_topic_votes
    ADD CONSTRAINT fk_forum_topic_votes_topic
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_reply_votes
    ADD CONSTRAINT fk_forum_reply_votes_reply
    FOREIGN KEY (reply_id) REFERENCES forum_replies (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_category_subscriptions
    ADD CONSTRAINT fk_forum_category_subscriptions_category
    FOREIGN KEY (category_id) REFERENCES forum_categories (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_subscriptions
    ADD CONSTRAINT fk_forum_topic_subscriptions_topic
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_solutions
    ADD CONSTRAINT fk_forum_solutions_topic
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_solutions
    ADD CONSTRAINT fk_forum_solutions_reply
    FOREIGN KEY (reply_id) REFERENCES forum_replies (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_tags
    ADD CONSTRAINT fk_forum_topic_tags_topic
    FOREIGN KEY (topic_id) REFERENCES forum_topics (id)
    ON UPDATE CASCADE ON DELETE CASCADE;
ALTER TABLE forum_topic_tags
    ADD CONSTRAINT fk_forum_topic_tags_term
    FOREIGN KEY (term_id) REFERENCES taxonomy_terms (id)
    ON UPDATE CASCADE ON DELETE CASCADE;

DROP INDEX IF EXISTS uq_forum_solutions_tenant_reply;
CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_solutions_reply_unique
    ON forum_solutions (reply_id);
DROP INDEX IF EXISTS uq_forum_topic_tags_tenant_topic_term;
CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_topic_tags_topic_term
    ON forum_topic_tags (topic_id, term_id);

ALTER TABLE forum_replies
    DROP CONSTRAINT IF EXISTS uq_forum_replies_tenant_topic_id;
"#,
        )
        .await?;
    Ok(())
}

async fn up_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for (query, message) in [
        (
            "SELECT COUNT(*) AS invalid_count FROM forum_topic_votes vote \
             WHERE NOT EXISTS (SELECT 1 FROM forum_topics topic \
             WHERE topic.id = vote.topic_id AND topic.tenant_id = vote.tenant_id)",
            "forum relation migration blocked: invalid topic vote tenant",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM forum_reply_votes vote \
             WHERE NOT EXISTS (SELECT 1 FROM forum_replies reply \
             WHERE reply.id = vote.reply_id AND reply.tenant_id = vote.tenant_id)",
            "forum relation migration blocked: invalid reply vote tenant",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM forum_category_subscriptions subscription \
             WHERE NOT EXISTS (SELECT 1 FROM forum_categories category \
             WHERE category.id = subscription.category_id \
             AND category.tenant_id = subscription.tenant_id)",
            "forum relation migration blocked: invalid category subscription tenant",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM forum_topic_subscriptions subscription \
             WHERE NOT EXISTS (SELECT 1 FROM forum_topics topic \
             WHERE topic.id = subscription.topic_id \
             AND topic.tenant_id = subscription.tenant_id)",
            "forum relation migration blocked: invalid topic subscription tenant",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM forum_solutions solution \
             WHERE NOT EXISTS (SELECT 1 FROM forum_topics topic \
             WHERE topic.id = solution.topic_id AND topic.tenant_id = solution.tenant_id) \
             OR NOT EXISTS (SELECT 1 FROM forum_replies reply \
             WHERE reply.id = solution.reply_id AND reply.topic_id = solution.topic_id \
             AND reply.tenant_id = solution.tenant_id)",
            "forum relation migration blocked: invalid solution relation",
        ),
        (
            "SELECT COUNT(*) AS invalid_count FROM forum_topic_tags tag \
             WHERE NOT EXISTS (SELECT 1 FROM forum_topics topic \
             WHERE topic.id = tag.topic_id AND topic.tenant_id = tag.tenant_id) \
             OR NOT EXISTS (SELECT 1 FROM taxonomy_terms term \
             WHERE term.id = tag.term_id AND term.tenant_id = tag.tenant_id)",
            "forum relation migration blocked: invalid topic tag tenant",
        ),
    ] {
        ensure_sqlite_query_empty(manager, query, message).await?;
    }

    for statement in [
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_topic_votes_tenant_topic_user ON forum_topic_votes (tenant_id, topic_id, user_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_reply_votes_tenant_reply_user ON forum_reply_votes (tenant_id, reply_id, user_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_category_subscriptions_tenant_category_user ON forum_category_subscriptions (tenant_id, category_id, user_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_topic_subscriptions_tenant_topic_user ON forum_topic_subscriptions (tenant_id, topic_id, user_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_solutions_tenant_topic ON forum_solutions (tenant_id, topic_id)",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_solutions_tenant_reply ON forum_solutions (tenant_id, reply_id)",
        "DROP INDEX IF EXISTS idx_forum_topic_tags_topic_term",
        "CREATE UNIQUE INDEX IF NOT EXISTS uq_forum_topic_tags_tenant_topic_term ON forum_topic_tags (tenant_id, topic_id, term_id)",
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }

    for statement in sqlite_triggers() {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    Ok(())
}

async fn down_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for name in [
        "forum_topic_votes_tenant_insert",
        "forum_topic_votes_tenant_update",
        "forum_reply_votes_tenant_insert",
        "forum_reply_votes_tenant_update",
        "forum_category_subscriptions_tenant_insert",
        "forum_category_subscriptions_tenant_update",
        "forum_topic_subscriptions_tenant_insert",
        "forum_topic_subscriptions_tenant_update",
        "forum_solutions_tenant_insert",
        "forum_solutions_tenant_update",
        "forum_topic_tags_tenant_insert",
        "forum_topic_tags_tenant_update",
    ] {
        manager
            .get_connection()
            .execute_unprepared(&format!("DROP TRIGGER IF EXISTS {name}"))
            .await?;
    }

    for statement in [
        "DROP INDEX IF EXISTS uq_forum_topic_votes_tenant_topic_user",
        "DROP INDEX IF EXISTS uq_forum_reply_votes_tenant_reply_user",
        "DROP INDEX IF EXISTS uq_forum_category_subscriptions_tenant_category_user",
        "DROP INDEX IF EXISTS uq_forum_topic_subscriptions_tenant_topic_user",
        "DROP INDEX IF EXISTS uq_forum_solutions_tenant_topic",
        "DROP INDEX IF EXISTS uq_forum_solutions_tenant_reply",
        "DROP INDEX IF EXISTS uq_forum_topic_tags_tenant_topic_term",
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_forum_topic_tags_topic_term ON forum_topic_tags (topic_id, term_id)",
    ] {
        manager
            .get_connection()
            .execute_unprepared(statement)
            .await?;
    }
    Ok(())
}

fn sqlite_triggers() -> [&'static str; 12] {
    [
        r#"CREATE TRIGGER forum_topic_votes_tenant_insert BEFORE INSERT ON forum_topic_votes FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum topic vote tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_topic_votes_tenant_update BEFORE UPDATE OF tenant_id, topic_id ON forum_topic_votes FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum topic vote tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_reply_votes_tenant_insert BEFORE INSERT ON forum_reply_votes FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_replies reply WHERE reply.id = NEW.reply_id AND reply.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum reply vote tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_reply_votes_tenant_update BEFORE UPDATE OF tenant_id, reply_id ON forum_reply_votes FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_replies reply WHERE reply.id = NEW.reply_id AND reply.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum reply vote tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_category_subscriptions_tenant_insert BEFORE INSERT ON forum_category_subscriptions FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_categories category WHERE category.id = NEW.category_id AND category.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum category subscription tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_category_subscriptions_tenant_update BEFORE UPDATE OF tenant_id, category_id ON forum_category_subscriptions FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_categories category WHERE category.id = NEW.category_id AND category.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum category subscription tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_topic_subscriptions_tenant_insert BEFORE INSERT ON forum_topic_subscriptions FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum topic subscription tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_topic_subscriptions_tenant_update BEFORE UPDATE OF tenant_id, topic_id ON forum_topic_subscriptions FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum topic subscription tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_solutions_tenant_insert BEFORE INSERT ON forum_solutions FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM forum_replies reply WHERE reply.id = NEW.reply_id AND reply.topic_id = NEW.topic_id AND reply.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum solution relation mismatch'); END"#,
        r#"CREATE TRIGGER forum_solutions_tenant_update BEFORE UPDATE OF tenant_id, topic_id, reply_id ON forum_solutions FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM forum_replies reply WHERE reply.id = NEW.reply_id AND reply.topic_id = NEW.topic_id AND reply.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum solution relation mismatch'); END"#,
        r#"CREATE TRIGGER forum_topic_tags_tenant_insert BEFORE INSERT ON forum_topic_tags FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM taxonomy_terms term WHERE term.id = NEW.term_id AND term.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum topic tag tenant mismatch'); END"#,
        r#"CREATE TRIGGER forum_topic_tags_tenant_update BEFORE UPDATE OF tenant_id, topic_id, term_id ON forum_topic_tags FOR EACH ROW
           WHEN NEW.tenant_id IS NULL OR NOT EXISTS (SELECT 1 FROM forum_topics topic WHERE topic.id = NEW.topic_id AND topic.tenant_id = NEW.tenant_id)
             OR NOT EXISTS (SELECT 1 FROM taxonomy_terms term WHERE term.id = NEW.term_id AND term.tenant_id = NEW.tenant_id)
           BEGIN SELECT RAISE(ABORT, 'forum topic tag tenant mismatch'); END"#,
    ]
}

async fn ensure_sqlite_query_empty(
    manager: &SchemaManager<'_>,
    query: &str,
    message: &str,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            query.to_owned(),
        ))
        .await?
        .ok_or_else(|| {
            DbErr::Custom("failed to validate forum relation tenant integrity".to_owned())
        })?;
    let invalid_count: i64 = row.try_get("", "invalid_count")?;
    if invalid_count != 0 {
        return Err(DbErr::Custom(message.to_owned()));
    }
    Ok(())
}
