use std::time::Duration;

use tedious_rs::ConnectionConfig;

#[test]
fn column_encryption_defaults_to_disabled_and_force_vector_is_public() {
    let config = ConnectionConfig::sql("localhost", "user", "password");
    assert!(!config.column_encryption_setting);
    assert_eq!(
        config.column_encryption_key_cache_ttl,
        Duration::from_secs(7200)
    );

    let mut connection = tedious_rs::Connection::new(config);
    connection.set_force_encrypt(vec![true]);
}
