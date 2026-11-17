include!("retry_reroute.rs");

#[test]
fn retry_count_starts_at_zero_and_matches_node_field_contract() {
    let config = ConnectionConfig::sql("127.0.0.1", "user", "password");
    let connection = Connection::new(config);

    assert_eq!(connection.cur_transient_retry_count, 0);
    assert_eq!(connection.config.max_retries_on_transient_errors, 3);
}
