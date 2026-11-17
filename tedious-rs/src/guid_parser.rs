const HEX: &[u8; 16] = b"0123456789abcdef";

pub fn buffer_to_upper_case_guid(buffer: &[u8]) -> Option<String> {
    format_guid(buffer, true)
}
pub fn buffer_to_lower_case_guid(buffer: &[u8]) -> Option<String> {
    format_guid(buffer, false)
}

fn format_guid(buffer: &[u8], upper: bool) -> Option<String> {
    if buffer.len() < 16 {
        return None;
    }
    let mut out = String::with_capacity(36);
    for (index, value) in [3, 2, 1, 0, 5, 4, 7, 6, 8, 9, 10, 11, 12, 13, 14, 15]
        .into_iter()
        .enumerate()
    {
        if index == 4 || index == 6 || index == 8 || index == 10 {
            out.push('-');
        }
        let byte = buffer[value];
        let high = HEX[(byte >> 4) as usize];
        let low = HEX[(byte & 15) as usize];
        out.push(if upper {
            high.to_ascii_uppercase() as char
        } else {
            high as char
        });
        out.push(if upper {
            low.to_ascii_uppercase() as char
        } else {
            low as char
        });
    }
    Some(out)
}

pub fn guid_to_array(guid: &str) -> Result<Vec<u8>, String> {
    let bytes = guid.as_bytes();
    if bytes.len() != 36
        || bytes[8] != b'-'
        || bytes[13] != b'-'
        || bytes[18] != b'-'
        || bytes[23] != b'-'
    {
        return Err("invalid GUID".into());
    }
    let mut raw = [0u8; 16];
    let positions = [6, 4, 2, 0, 11, 9, 16, 14, 19, 21, 24, 26, 28, 30, 32, 34];
    for (index, position) in positions.into_iter().enumerate() {
        raw[index] = pair(bytes[position], bytes[position + 1])?;
    }
    Ok(raw.to_vec())
}

fn pair(high: u8, low: u8) -> Result<u8, String> {
    Ok((digit(high)? << 4) | digit(low)?)
}
fn digit(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err("invalid GUID digit".into()),
    }
}
