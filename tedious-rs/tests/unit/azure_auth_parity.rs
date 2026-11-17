use tedious_rs::azure_auth::{AzureCredentialSelection, parse_azure_credential_selection};

#[test]
fn accepts_all_default_azure_credential_names_from_node() {
    let cases = [
        (
            "EnvironmentCredential",
            AzureCredentialSelection::Environment,
        ),
        (
            "WorkloadIdentityCredential",
            AzureCredentialSelection::WorkloadIdentity,
        ),
        (
            "ManagedIdentityCredential",
            AzureCredentialSelection::ManagedIdentity,
        ),
        (
            "VisualStudioCodeCredential",
            AzureCredentialSelection::VisualStudioCode,
        ),
        ("AzureCliCredential", AzureCredentialSelection::AzureCli),
        (
            "AzurePowerShellCredential",
            AzureCredentialSelection::AzurePowerShell,
        ),
        (
            "AzureDeveloperCliCredential",
            AzureCredentialSelection::AzureDeveloperCli,
        ),
    ];

    for (value, expected) in cases {
        assert_eq!(
            parse_azure_credential_selection(Some(value)).unwrap(),
            expected
        );
    }
}

#[test]
fn preserves_node_chain_group_selectors() {
    assert_eq!(
        parse_azure_credential_selection(Some("prod")).unwrap(),
        AzureCredentialSelection::Production
    );
    assert_eq!(
        parse_azure_credential_selection(Some("dev")).unwrap(),
        AzureCredentialSelection::Development
    );
    assert_eq!(
        parse_azure_credential_selection(Some(" ")).unwrap(),
        AzureCredentialSelection::All
    );
    assert_eq!(
        parse_azure_credential_selection(None).unwrap(),
        AzureCredentialSelection::All
    );
}

#[test]
fn rejects_values_outside_node_supported_set() {
    let error = parse_azure_credential_selection(Some("BrokerCredential")).unwrap_err();
    assert_eq!(error.code.as_deref(), Some("EAUTH"));
    assert!(!error.message.contains("BrokerCredential"));
}
