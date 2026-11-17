use tedious_rs::transient_error_lookup::is_transient_error;

#[test]
fn recognizes_all_sql_client_transient_errors() {
    for number in [4060, 10928, 10929, 40197, 40501, 40613] {
        assert!(is_transient_error(number));
    }
}

#[test]
fn rejects_non_transient_sql_errors() {
    for number in [0, 102, 18456, 2627, 50000, 4061] {
        assert!(!is_transient_error(number));
    }
}
