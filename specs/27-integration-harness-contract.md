# Contrato de integração

O perfil `sql_auth_mutating` usa localhost, `sa`, `DevStr0ng\@Pass` e `tedious_rs` para metadata, CRUD, TVP e bulk.

O perfil `ntlm_prod_read_only` usa TEST_DB_HOST, VPN_USER, VPN_PWD, TEST_DB_NAME e VPN_DOMAIN opcionalmente. Ele permite somente SELECT 1, SELECT e metadata.
