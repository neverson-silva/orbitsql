use crate::common;
use tedious_rs::connection::Connection;
use tedious_rs::token::stream::StreamToken;

async fn ntlm_select_case(connection: &mut Connection) -> Result<(), Box<dyn std::error::Error>> {
    let tokens = connection
        .exec_sql_batch("SELECT TABLE_SCHEMA, TABLE_NAME FROM INFORMATION_SCHEMA.TABLES;")
        .await?;
    if !tokens
        .iter()
        .any(|token| matches!(token, StreamToken::ColMetadata(columns) if columns.len() == 2))
    {
        return Err("NTLM metadata result did not contain the expected columns".into());
    }
    if !tokens
        .iter()
        .any(|token| matches!(token, StreamToken::Row(row) if row.columns.len() == 2))
    {
        return Err("NTLM metadata result did not contain the expected row".into());
    }
    let error = connection
        .exec_sql_batch("INSERT INTO dbo.tedious_rs_read_only_probe(value) VALUES (1);")
        .await
        .expect_err("NTLM PROD must reject mutating SQL before network IO");
    if error.code.as_deref() != Some("EREADONLY") {
        return Err(format!("unexpected NTLM read-only error code: {:?}", error.code).into());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and NTLM PROD credentials"]
async fn ntlm_prod_profile_is_read_only_for_all_domain_states()
-> Result<(), Box<dyn std::error::Error>> {
    if !common::integration_enabled() {
        return Ok(());
    }
    for domain in common::ntlm_domain_variants() {
        common::with_ntlm_connection(domain, |connection| Box::pin(ntlm_select_case(connection)))
            .await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires TEDIOUS_RS_RUN_INTEGRATION and NTLM PROD credentials"]
async fn ntlm_environment_domain_state_is_preserved() -> Result<(), Box<dyn std::error::Error>> {
    if !common::integration_enabled() {
        return Ok(());
    }
    let domain = std::env::var("VPN_DOMAIN").ok();
    let config = common::ntlm_config(domain.clone())?;
    match (&config.authentication, domain) {
        (tedious_rs::connection::Authentication::Ntlm { domain: actual, .. }, expected) => {
            assert_eq!(actual, &expected)
        }
        _ => return Err("NTLM harness produced a non-NTLM configuration".into()),
    }
    Ok(())
}
