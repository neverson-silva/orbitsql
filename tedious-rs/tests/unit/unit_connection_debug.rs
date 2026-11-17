use tedious_rs::connection::{Authentication, ConnectionConfig};

#[test]
fn authentication_debug_redacts_passwords_tokens_and_client_secrets() {
    let authentications = [
        (
            Authentication::Sql {
                user_name: "sql-user".to_owned(),
                password: "sql-password".to_owned(),
            },
            "sql-password",
        ),
        (
            Authentication::Ntlm {
                user_name: "ntlm-user".to_owned(),
                password: "ntlm-password".to_owned(),
                domain: Some("domain".to_owned()),
            },
            "ntlm-password",
        ),
        (
            Authentication::FedAuth {
                token: "fed-token".to_owned(),
            },
            "fed-token",
        ),
        (
            Authentication::AzureActiveDirectoryAccessToken {
                token: "access-token".to_owned(),
            },
            "access-token",
        ),
        (
            Authentication::AzureActiveDirectoryPassword {
                client_id: "client-id".to_owned(),
                user_name: Some("azure-user".to_owned()),
                password: Some("azure-password".to_owned()),
                tenant_id: Some("tenant-id".to_owned()),
            },
            "azure-password",
        ),
        (
            Authentication::AzureActiveDirectoryServicePrincipalSecret {
                client_id: "service-client-id".to_owned(),
                client_secret: "client-secret".to_owned(),
                tenant_id: "service-tenant-id".to_owned(),
            },
            "client-secret",
        ),
    ];

    for (authentication, secret) in authentications {
        let rendered = format!("{authentication:?}");
        assert!(!rendered.contains(secret));
        assert!(rendered.contains("[REDACTED]"));
    }
}

#[test]
fn connection_config_debug_redacts_nested_authentication_secrets() {
    let config = ConnectionConfig::azure_active_directory_service_principal_secret(
        "sql.example",
        "client-id",
        "client-secret",
        "tenant-id",
    );
    let rendered = format!("{config:?}");

    assert!(rendered.contains("sql.example"));
    assert!(rendered.contains("client-id"));
    assert!(rendered.contains("tenant-id"));
    assert!(rendered.contains("[REDACTED]"));
    assert!(!rendered.contains("client-secret"));
}
