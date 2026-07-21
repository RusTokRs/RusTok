use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;
use uuid::Uuid;

const LEGACY_UNDETERMINED_LOCALE: &str = "und";

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let backend = db.get_database_backend();
        let profiles = db
            .query_all(Statement::from_string(
                backend,
                "SELECT user_id, display_name FROM profiles".to_string(),
            ))
            .await?;

        for profile in profiles {
            let user_id: Uuid = profile.try_get("", "user_id")?;
            let display_name: String = profile.try_get("", "display_name")?;

            let matching_copy = db
                .query_one(Statement::from_sql_and_values(
                    backend,
                    "SELECT 1 AS present FROM profile_translations \
                     WHERE profile_user_id = ? AND display_name = ? LIMIT 1"
                        .to_string(),
                    vec![user_id.into(), display_name.clone().into()],
                ))
                .await?;
            if matching_copy.is_some() {
                continue;
            }

            let existing_und = db
                .query_one(Statement::from_sql_and_values(
                    backend,
                    "SELECT display_name FROM profile_translations \
                     WHERE profile_user_id = ? AND locale = ? LIMIT 1"
                        .to_string(),
                    vec![user_id.into(), LEGACY_UNDETERMINED_LOCALE.into()],
                ))
                .await?;
            if let Some(existing_und) = existing_und {
                let retained: String = existing_und.try_get("", "display_name")?;
                return Err(DbErr::Custom(format!(
                    "profile display-name cutover blocked for {user_id}: existing und copy {retained:?} conflicts with legacy base copy {display_name:?}"
                )));
            }

            db.execute(Statement::from_sql_and_values(
                backend,
                "INSERT INTO profile_translations \
                 (id, profile_user_id, locale, display_name, bio, created_at, updated_at) \
                 VALUES (?, ?, ?, ?, NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
                    .to_string(),
                vec![
                    Uuid::new_v4().into(),
                    user_id.into(),
                    LEGACY_UNDETERMINED_LOCALE.into(),
                    display_name.into(),
                ],
            ))
            .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Profiles::Table)
                    .drop_column(Profiles::DisplayName)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Profiles::Table)
                    .add_column(ColumnDef::new(Profiles::DisplayName).string_len(255).null())
                    .to_owned(),
            )
            .await?;

        let db = manager.get_connection();
        let backend = db.get_database_backend();
        let profiles = db
            .query_all(Statement::from_string(
                backend,
                "SELECT user_id, preferred_locale FROM profiles".to_string(),
            ))
            .await?;

        for profile in profiles {
            let user_id: Uuid = profile.try_get("", "user_id")?;
            let preferred_locale: Option<String> = profile.try_get("", "preferred_locale")?;
            let preferred_locale = preferred_locale
                .as_deref()
                .unwrap_or(LEGACY_UNDETERMINED_LOCALE);
            let translation = db
                .query_one(Statement::from_sql_and_values(
                    backend,
                    "SELECT display_name FROM profile_translations \
                     WHERE profile_user_id = ? \
                     ORDER BY CASE WHEN locale = ? THEN 0 WHEN locale = 'und' THEN 1 ELSE 2 END, locale \
                     LIMIT 1"
                        .to_string(),
                    vec![user_id.into(), preferred_locale.into()],
                ))
                .await?
                .ok_or_else(|| {
                    DbErr::Custom(format!(
                        "cannot restore profiles.display_name for {user_id}: no profile translation exists"
                    ))
                })?;
            let display_name: String = translation.try_get("", "display_name")?;
            db.execute(Statement::from_sql_and_values(
                backend,
                "UPDATE profiles SET display_name = ? WHERE user_id = ?".to_string(),
                vec![display_name.into(), user_id.into()],
            ))
            .await?;
        }

        Ok(())
    }
}

#[derive(DeriveIden)]
enum Profiles {
    Table,
    DisplayName,
}
