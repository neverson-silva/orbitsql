use std::time::Duration;

use tedious_rs::token::stream::StreamToken;
use tedious_rs::value_codec::Value;

#[tokio::test]
async fn production_request_parser_accepts_token_boundaries_split_across_packets()
-> Result<(), Box<dyn std::error::Error>> {
    let mut parser = tedious_rs::token::stream::TokenStreamParser::new("7_4", false);
    let mut payload = vec![0x81, 1, 0, 0, 0, 0, 0, 0, 0, 0x38, 2, b'I', 0, b'd', 0];
    payload.extend([0xD1, 7, 0, 0, 0]);
    let first = parser.feed(&payload[..4]);
    assert!(first.is_err());
    let second = parser.feed(&payload[4..]);
    assert!(second.is_ok_and(|tokens| tokens.len() == 2));
    parser.finish().map_err(|error| format!("{error:?}"))?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires the configured local SQL Server Authentication endpoint"]
async fn sql_auth_request_consumes_large_fragmented_result_incrementally()
-> Result<(), Box<dyn std::error::Error>> {
    if !crate::common::integration_enabled() {
        return Ok(());
    }
    let Ok(mut config) = crate::common::sql_config() else {
        return Ok(());
    };
    config.request_timeout = Duration::from_secs(30);
    let mut connection = tedious_rs::connection::Connection::new(config);
    connection.connect().await?;
    let result = async {
        let sql = "SELECT TOP (1000) a.column_id, REPLICATE(N'x', 200) AS payload FROM sys.all_columns a CROSS JOIN sys.all_columns b ORDER BY a.column_id;";
        let tokens = connection.exec_sql_batch(sql).await?;
        let rows = tokens
            .iter()
            .filter_map(|token| match token {
                StreamToken::Row(row) | StreamToken::NbcRow(row) => Some(row),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(rows.len() >= 100);
        assert!(rows.iter().all(|row| row.columns.len() == 2));
        assert!(rows.iter().all(|row| matches!(
            row.columns.get(1).map(|column| &column.value),
            Some(Value::Text(value)) if value.len() == 200
        )));
        Ok::<(), Box<dyn std::error::Error>>(())
    }
    .await;
    connection.close().await;
    result
}
