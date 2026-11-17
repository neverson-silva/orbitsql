use crate::common;
use tedious_rs::connection::Connection;
use tedious_rs::token::stream::StreamToken;

async fn sql_auth_case(connection: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let table = format!("dbo.{}", common::sql_auth_object_name("harness"));
    let create = format!(
        "IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table}; CREATE TABLE {table} (id int NOT NULL, name nvarchar(100) NULL);"
    );
    let cleanup = format!("IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {table};");
    let result = async {
        connection.exec_sql_batch(create).await?;
        let tokens = connection
            .exec_sql_batch(format!(
                "INSERT INTO {table} (id, name) VALUES (1, N'linha'); UPDATE {table} SET name = N'atualizada' WHERE id = 1; SELECT id, name FROM {table}; DELETE FROM {table} WHERE id = 1;"
            ))
            .await?;
        if !tokens.iter().any(|token| {
            matches!(token, StreamToken::ColMetadata(columns) if columns.len() == 2)
        }) {
            return Err("SQL Auth metadata result did not contain the expected columns".into());
        }
        if !tokens.iter().any(|token| {
            matches!(token, StreamToken::Row(row) if row.columns.len() == 2)
        }) {
            return Err("SQL Auth CRUD result did not contain the expected row".into());
        }
        Ok(())
    }
    .await;
    let cleanup_result = connection
        .exec_sql_batch(cleanup)
        .await
        .map(|_| ())
        .map_err(|error| Box::new(error) as Box<dyn std::error::Error>);
    super::combine_operation_and_cleanup(result, cleanup_result)
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and a local SQL Authentication endpoint"]
async fn sql_auth_local_profile_is_mutable_and_cleaned() -> Result<(), Box<dyn std::error::Error>> {
    if !common::integration_enabled() {
        return Ok(());
    }
    common::with_sql_connection(|connection| Box::pin(sql_auth_case(connection))).await
}
