mod m20260326_000001_create_profiles_tables;
mod m20260330_000002_create_profile_tags;
mod m20260721_000009_expand_profile_locale_storage_columns;
mod m20260721_000010_move_profile_display_name_to_translations;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260326_000001_create_profiles_tables::Migration),
        Box::new(m20260330_000002_create_profile_tags::Migration),
        Box::new(m20260721_000009_expand_profile_locale_storage_columns::Migration),
        Box::new(m20260721_000010_move_profile_display_name_to_translations::Migration),
    ]
}
