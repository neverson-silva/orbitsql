use rand::RngCore;
use tedious_rs::always_encrypted::keystore_provider_azure_key_vault::{
    AzureKeyVaultHttpClient, ColumnEncryptionAzureKeyVaultProvider,
};
use tedious_rs::always_encrypted::types::StatementColumnEncryptionSetting;
use tedious_rs::data_type::type_by_name;
use tedious_rs::parameter::ParameterOptions;
use tedious_rs::request::{Request, RequestOptions};
use tedious_rs::value_codec::Value;

fn required(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = std::env::var(name)
        .map_err(|_| format!("required Always Encrypted environment variable {name} is missing"))?;
    if value.is_empty() {
        return Err(
            format!("required Always Encrypted environment variable {name} is empty").into(),
        );
    }
    Ok(value)
}

fn sql_identifier(value: &str) -> String {
    format!("[{}]", value.replace(']', "]]"))
}

fn sql_literal(value: &str) -> String {
    value.replace('\'', "''")
}

fn hex_bytes(value: &[u8]) -> String {
    let mut output = String::with_capacity(value.len() * 2 + 2);
    output.push_str("0x");
    for byte in value {
        output.push_str(&format!("{byte:02X}"));
    }
    output
}

fn cleanup_sql(table: &str, cek: &str, cmk: &str) -> String {
    format!(
        "IF OBJECT_ID(N'{table}', N'U') IS NOT NULL DROP TABLE {cek_table}; IF EXISTS (SELECT 1 FROM sys.column_encryption_keys WHERE name = N'{cek_name}') DROP COLUMN ENCRYPTION KEY {cek_identifier}; IF EXISTS (SELECT 1 FROM sys.column_master_keys WHERE name = N'{cmk_name}') DROP COLUMN MASTER KEY {cmk_identifier};",
        table = sql_literal(table),
        cek_table = sql_identifier(table),
        cek_name = sql_literal(cek),
        cek_identifier = sql_identifier(cek),
        cmk_name = sql_literal(cmk),
        cmk_identifier = sql_identifier(cmk)
    )
}

fn configured() -> bool {
    matches!(
        std::env::var("TEDIOUS_RS_RUN_AE_INTEGRATION").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

#[tokio::test]
#[ignore = "requires explicit SQL Auth and Azure Key Vault Always Encrypted configuration"]
async fn sql_server_executes_real_azure_key_vault_always_encrypted_round_trip()
-> Result<(), Box<dyn std::error::Error>> {
    if !super::common::integration_enabled() || !configured() {
        return Ok(());
    }
    let client_id = required("AZURE_CLIENT_ID")?;
    let client_secret = required("AZURE_CLIENT_SECRET")?;
    let tenant_id = required("AZURE_TENANT_ID")?;
    let key_path = required("TEDIOUS_RS_AE_KEY_PATH")?;
    let provider = ColumnEncryptionAzureKeyVaultProvider::new(AzureKeyVaultHttpClient::new(
        client_id,
        client_secret,
        tenant_id,
    )?);
    let mut cek_plaintext = [0u8; 32];
    rand::rng().fill_bytes(&mut cek_plaintext);
    let encrypted_cek =
        provider.encrypt_column_encryption_key(&key_path, "RSA_OAEP", &cek_plaintext)?;

    let suffix = super::common::sql_auth_object_name("ae");
    let table_name = format!("{suffix}_table");
    let cmk_name = format!("{suffix}_cmk");
    let cek_name = format!("{suffix}_cek");
    let table = sql_identifier(&table_name);
    let cmk = sql_identifier(&cmk_name);
    let cek = sql_identifier(&cek_name);
    let cleanup = cleanup_sql(&table_name, &cek_name, &cmk_name);
    let mut config = super::common::sql_config()?;
    config.column_encryption_setting = true;
    config.trusted_server_name_ae = Some("localhost".to_owned());
    config.row_collection_on_request_completion = true;
    let mut connection = tedious_rs::Connection::new(config);
    connection.set_key_store_provider(provider);
    connection.connect().await?;

    let setup = format!(
        "{cleanup} CREATE COLUMN MASTER KEY {cmk} WITH (KEY_STORE_PROVIDER_NAME = N'AZURE_KEY_VAULT', KEY_PATH = N'{key_path}'); CREATE COLUMN ENCRYPTION KEY {cek} WITH VALUES (COLUMN_MASTER_KEY = {cmk}, ALGORITHM = 'RSA_OAEP', ENCRYPTED_VALUE = {encrypted_cek}); CREATE TABLE {table} (id int NOT NULL PRIMARY KEY, secret nvarchar(100) COLLATE Latin1_General_BIN2 ENCRYPTED WITH (COLUMN_ENCRYPTION_KEY = {cek}, ENCRYPTION_TYPE = DETERMINISTIC, ALGORITHM = 'AEAD_AES_256_CBC_HMAC_SHA256') NOT NULL);",
        cleanup = cleanup,
        cmk = cmk,
        key_path = sql_literal(&key_path),
        cek = cek,
        encrypted_cek = hex_bytes(&encrypted_cek),
        table = table
    );
    let operation = async {
        connection.exec_sql_batch(setup).await?;
        let mut insert = Request::with_options(
            format!("INSERT INTO {table} (id, secret) VALUES (@id, @secret);"),
            |_| {},
            RequestOptions {
                statement_column_encryption_setting: StatementColumnEncryptionSetting::Enabled,
            },
        );
        insert.add_parameter(
            "id",
            type_by_name("Int").ok_or("missing Int type")?,
            Value::Integer(7),
            ParameterOptions::default(),
        )?;
        insert.add_parameter(
            "secret",
            type_by_name("NVarChar").ok_or("missing NVarChar type")?,
            Value::Text("valor AE real".to_owned()),
            ParameterOptions::default(),
        )?;
        connection.exec_sql(&mut insert).await?;

        let mut select = Request::with_options(
            format!("SELECT id, secret FROM {table} WHERE id = @id;"),
            |_| {},
            RequestOptions {
                statement_column_encryption_setting: StatementColumnEncryptionSetting::Enabled,
            },
        );
        select.add_parameter(
            "id",
            type_by_name("Int").ok_or("missing Int type")?,
            Value::Integer(7),
            ParameterOptions::default(),
        )?;
        connection.exec_sql(&mut select).await?;
        let row = select
            .rows
            .first()
            .ok_or("Always Encrypted SELECT returned no row")?;
        if row.columns.len() != 2 {
            return Err("Always Encrypted SELECT returned an unexpected column count".into());
        }
        assert_eq!(row.columns[0].value, Value::Integer(7));
        assert_eq!(
            row.columns[1].value,
            Value::Text("valor AE real".to_owned())
        );
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    let cleanup_result = connection.exec_sql_batch(cleanup).await.map(|_| ());
    connection.close().await;
    match (operation, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(operation), Ok(())) => Err(operation),
        (Ok(()), Err(cleanup)) => Err(cleanup.into()),
        (Err(operation), Err(cleanup)) => Err(format!(
            "Always Encrypted operation failed: {operation}; cleanup failed: {cleanup}"
        )
        .into()),
    }
}
