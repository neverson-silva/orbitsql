use tedious_rs::always_encrypted::keystore_provider_azure_key_vault::AzureKeyVaultHttpClient;

#[test]
fn azure_key_vault_http_is_opt_in_without_embedded_secrets() {
    let configured = ["AZURE_CLIENT_ID", "AZURE_CLIENT_SECRET", "AZURE_TENANT_ID"]
        .iter()
        .all(|name| std::env::var(name).is_ok());
    if !configured {
        return;
    }
    let client = AzureKeyVaultHttpClient::new(
        std::env::var("AZURE_CLIENT_ID").unwrap(),
        std::env::var("AZURE_CLIENT_SECRET").unwrap(),
        std::env::var("AZURE_TENANT_ID").unwrap(),
    );
    assert!(client.is_ok());
}
