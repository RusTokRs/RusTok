//! Newsletter module migrations.

mod m20261009_000040_create_newsletter_tables;

pub use m20261009_000040_create_newsletter_tables::Migration as CreateNewsletterTables;

pub fn all() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
    vec![Box::new(CreateNewsletterTables)]
}
