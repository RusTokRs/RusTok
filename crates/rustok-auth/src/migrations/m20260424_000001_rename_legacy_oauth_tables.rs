use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DbBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DO $$
BEGIN
    IF to_regclass('public.o_auth_apps') IS NOT NULL
       AND to_regclass('public.oauth_apps') IS NULL THEN
        ALTER TABLE public.o_auth_apps RENAME TO oauth_apps;
    END IF;

    IF to_regclass('public.o_auth_tokens') IS NOT NULL
       AND to_regclass('public.oauth_tokens') IS NULL THEN
        ALTER TABLE public.o_auth_tokens RENAME TO oauth_tokens;
    END IF;

    IF to_regclass('public.o_auth_authorization_codes') IS NOT NULL
       AND to_regclass('public.oauth_authorization_codes') IS NULL THEN
        ALTER TABLE public.o_auth_authorization_codes RENAME TO oauth_authorization_codes;
    END IF;

    IF to_regclass('public.o_auth_consents') IS NOT NULL
       AND to_regclass('public.oauth_consents') IS NULL THEN
        ALTER TABLE public.o_auth_consents RENAME TO oauth_consents;
    END IF;
END $$;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
