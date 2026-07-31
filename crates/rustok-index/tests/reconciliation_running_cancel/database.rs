use sea_orm::{ConnectionTrait, DatabaseConnection};
use uuid::Uuid;

use super::{
    connection::{TestResult, connect, database_url, print_skip, scoped_connection},
    prepare::prepare,
};

pub struct TestDatabase {
    control: DatabaseConnection,
    url: String,
    schema_name: String,
    pub tenant_id: Uuid,
}

impl TestDatabase {
    pub async fn setup() -> TestResult<Option<Self>> {
        let Some(url) = database_url() else {
            print_skip();
            return Ok(None);
        };
        let control = connect(&url).await?;
        let schema_name = format!(
            "rustok_index_reconciliation_running_cancel_{}",
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&url, &schema_name).await?;
        prepare(&db, tenant_id).await?;
        Ok(Some(Self {
            control,
            url,
            schema_name,
            tenant_id,
        }))
    }

    pub async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.url, &self.schema_name).await
    }

    pub async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}
