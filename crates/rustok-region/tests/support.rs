use rustok_region::entities::region;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Schema};

pub async fn ensure_region_schema(db: &DatabaseConnection) {
    if db.get_database_backend() != DbBackend::Sqlite {
        return;
    }

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);
    let mut statement = schema.create_table_from_entity(region::Entity);
    statement.if_not_exists();
    db.execute(builder.build(&statement))
        .await
        .expect("failed to create region test table");
}
