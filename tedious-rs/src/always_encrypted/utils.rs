use super::types::StatementColumnEncryptionSetting;

pub fn should_honor_ae(
    statement_setting: StatementColumnEncryptionSetting,
    connection_setting: bool,
) -> bool {
    match statement_setting {
        StatementColumnEncryptionSetting::Disabled
        | StatementColumnEncryptionSetting::ResultSetOnly => false,
        StatementColumnEncryptionSetting::Enabled => true,
        StatementColumnEncryptionSetting::UseConnectionSetting => connection_setting,
    }
}

pub fn should_encrypt_parameters(
    statement_setting: StatementColumnEncryptionSetting,
    connection_setting: bool,
) -> bool {
    match statement_setting {
        StatementColumnEncryptionSetting::Disabled
        | StatementColumnEncryptionSetting::ResultSetOnly => false,
        StatementColumnEncryptionSetting::Enabled => true,
        StatementColumnEncryptionSetting::UseConnectionSetting => connection_setting,
    }
}

pub fn should_decrypt_results(
    statement_setting: StatementColumnEncryptionSetting,
    connection_setting: bool,
) -> bool {
    match statement_setting {
        StatementColumnEncryptionSetting::Disabled => false,
        StatementColumnEncryptionSetting::Enabled
        | StatementColumnEncryptionSetting::ResultSetOnly => true,
        StatementColumnEncryptionSetting::UseConnectionSetting => connection_setting,
    }
}
