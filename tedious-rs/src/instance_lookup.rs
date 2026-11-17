use std::time::Duration;

use crate::connector::{AddressLookup, CancellationToken, SystemAddressLookup};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceLookupError(pub String);

pub fn parse_browser_response(response: &str, instance_name: &str) -> Option<u16> {
    let mut selected = false;
    for instance in response.split(";;") {
        let parts = instance.split(';').collect::<Vec<_>>();
        let mut index = 0;
        while index + 1 < parts.len() {
            let name = parts[index];
            let value = parts[index + 1];
            if name == "InstanceName" {
                selected = value.to_uppercase() == instance_name.to_uppercase();
            }
            if selected && name == "tcp" {
                return parse_js_integer(value).and_then(|port| u16::try_from(port).ok());
            }
            index += 2;
        }
    }
    None
}

fn parse_js_integer(value: &str) -> Option<i64> {
    let value = value.trim_start();
    let mut end = 0;
    if value
        .as_bytes()
        .get(end)
        .is_some_and(|byte| *byte == b'+' || *byte == b'-')
    {
        end += 1;
    }
    let digits = end;
    while value.as_bytes().get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    if end == digits {
        return None;
    }
    value[..end].parse().ok()
}

pub async fn lookup_instance(
    server: &str,
    instance_name: &str,
    timeout_duration: Duration,
    retries: u32,
) -> Result<u16, InstanceLookupError> {
    lookup_instance_with_options(
        server,
        instance_name,
        timeout_duration,
        retries,
        1434,
        &SystemAddressLookup,
        &CancellationToken::new(),
    )
    .await
}

pub async fn lookup_instance_with_options(
    server: &str,
    instance_name: &str,
    timeout_duration: Duration,
    retries: u32,
    port: u16,
    lookup: &dyn AddressLookup,
    cancellation: &CancellationToken,
) -> Result<u16, InstanceLookupError> {
    let mut response = None;
    for _ in 0..=retries {
        if cancellation.is_cancelled() {
            return Err(InstanceLookupError("operation cancelled".to_owned()));
        }
        let request = crate::sender::send_message_with_lookup(
            server,
            port,
            &[2],
            timeout_duration,
            lookup,
            cancellation,
        );
        match request.await {
            Ok(buffer) => {
                response = Some(buffer);
            }
            Err(error) if is_timeout_error(&error) => {}
            Err(error) => return Err(InstanceLookupError(error)),
        }
    }
    if cancellation.is_cancelled() {
        return Err(InstanceLookupError("operation cancelled".to_owned()));
    }
    if let Some(buffer) = response {
        let response = String::from_utf8_lossy(&buffer);
        if let Some(port) =
            parse_browser_response(response.get(3..).unwrap_or_default(), instance_name)
            && port != 0
        {
            return Ok(port);
        }
        return Err(InstanceLookupError(format!(
            "Port for {instance_name} not found in {server}"
        )));
    }
    Err(InstanceLookupError(format!(
        "Failed to get response from SQL Server Browser on {server}"
    )))
}

fn is_timeout_error(error: &str) -> bool {
    error == "UDP response timeout"
        || error
            .strip_prefix("all UDP sends failed: ")
            .is_some_and(|errors| {
                !errors.is_empty()
                    && errors
                        .split("; ")
                        .all(|error| error == "UDP response timeout")
            })
}
