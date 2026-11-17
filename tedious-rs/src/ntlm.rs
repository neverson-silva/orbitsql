use hmac::{Hmac, Mac};
use md4::{Digest, Md4};
use md5::Md5;
use std::fmt::{self, Display, Formatter};

type HmacMd5 = Hmac<Md5>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NtlmError(pub String);

impl Display for NtlmError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NtlmError {}

#[derive(Clone, PartialEq, Eq)]
pub struct NtlmChallenge {
    pub magic: [u8; 8],
    pub message_type: u32,
    pub domain_len: u16,
    pub domain_max: u16,
    pub domain_offset: u32,
    pub flags: u32,
    pub nonce: [u8; 8],
    pub zeroes: [u8; 8],
    pub target_len: u16,
    pub target_max: u16,
    pub target_offset: u32,
    pub odd_data: [u8; 8],
    pub domain: Vec<u8>,
    pub target: Vec<u8>,
}

impl fmt::Debug for NtlmChallenge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NtlmChallenge")
            .field("magic", &"[REDACTED; 8 bytes]")
            .field("message_type", &self.message_type)
            .field("domain_len", &self.domain_len)
            .field("domain_max", &self.domain_max)
            .field("domain_offset", &self.domain_offset)
            .field("flags", &self.flags)
            .field("nonce", &"[REDACTED; 8 bytes]")
            .field("zeroes", &"[REDACTED; 8 bytes]")
            .field("target_len", &self.target_len)
            .field("target_max", &self.target_max)
            .field("target_offset", &self.target_offset)
            .field("odd_data", &"[REDACTED; 8 bytes]")
            .field(
                "domain",
                &format_args!("[REDACTED; {} bytes]", self.domain.len()),
            )
            .field(
                "target",
                &format_args!("[REDACTED; {} bytes]", self.target.len()),
            )
            .finish()
    }
}

pub fn parse_challenge(buffer: &[u8]) -> Result<NtlmChallenge, NtlmError> {
    if buffer.len() < 56 {
        return Err(NtlmError("truncated NTLM challenge".to_owned()));
    }
    if &buffer[0..8] != b"NTLMSSP\0" {
        return Err(NtlmError("invalid NTLM challenge signature".to_owned()));
    }
    if u32::from_le_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]) != 2 {
        return Err(NtlmError("invalid NTLM challenge message type".to_owned()));
    }
    let take8 = |offset: usize| -> [u8; 8] {
        [
            buffer[offset],
            buffer[offset + 1],
            buffer[offset + 2],
            buffer[offset + 3],
            buffer[offset + 4],
            buffer[offset + 5],
            buffer[offset + 6],
            buffer[offset + 7],
        ]
    };
    let domain_len = u16::from_le_bytes([buffer[12], buffer[13]]);
    let target_len = u16::from_le_bytes([buffer[40], buffer[41]]);
    let domain_offset = u32::from_le_bytes([buffer[16], buffer[17], buffer[18], buffer[19]]);
    let target_offset = u32::from_le_bytes([buffer[44], buffer[45], buffer[46], buffer[47]]);
    let field = |offset: u32, length: u16, name: &str| -> Result<Vec<u8>, NtlmError> {
        let start = usize::try_from(offset)
            .map_err(|_| NtlmError(format!("invalid NTLM challenge {name} offset")))?;
        let end = start
            .checked_add(usize::from(length))
            .ok_or_else(|| NtlmError(format!("invalid NTLM challenge {name} length")))?;
        if start < 56 || end > buffer.len() {
            return Err(NtlmError(format!("truncated NTLM challenge {name} field")));
        }
        Ok(buffer[start..end].to_vec())
    };
    let domain = field(domain_offset, domain_len, "domain")?;
    let target = field(target_offset, target_len, "target")?;
    Ok(NtlmChallenge {
        magic: take8(0),
        message_type: u32::from_le_bytes([buffer[8], buffer[9], buffer[10], buffer[11]]),
        domain_len,
        domain_max: u16::from_le_bytes([buffer[14], buffer[15]]),
        domain_offset,
        flags: u32::from_le_bytes([buffer[20], buffer[21], buffer[22], buffer[23]]),
        nonce: take8(24),
        zeroes: take8(32),
        target_len,
        target_max: u16::from_le_bytes([buffer[42], buffer[43]]),
        target_offset,
        odd_data: take8(48),
        domain,
        target,
    })
}

fn ascii_upper(value: &str) -> Vec<u8> {
    value.to_ascii_uppercase().into_bytes()
}
fn utf16(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

pub fn create_ntlm_request(domain: &str, workstation: Option<&str>) -> Vec<u8> {
    let domain = ascii_upper(domain);
    let workstation = workstation.map_or_else(Vec::new, ascii_upper);
    let mut flags: u32 = 0x00000001
        | 0x00000002
        | 0x00000004
        | 0x00000200
        | 0x00001000
        | 0x00002000
        | 0x00008000
        | 0x00080000
        | 0x02000000
        | 0x20000000
        | 0x80000000;
    if workstation.is_empty() {
        flags -= 0x00002000;
    }
    let mut fixed = vec![0u8; 40];
    fixed[0..8].copy_from_slice(b"NTLMSSP\0");
    fixed[8..12].copy_from_slice(&1u32.to_le_bytes());
    fixed[12..16].copy_from_slice(&flags.to_le_bytes());
    fixed[16..18].copy_from_slice(&(domain.len() as u16).to_le_bytes());
    fixed[18..20].copy_from_slice(&(domain.len() as u16).to_le_bytes());
    fixed[20..24].copy_from_slice(&((40 + workstation.len()) as u32).to_le_bytes());
    fixed[24..26].copy_from_slice(&(workstation.len() as u16).to_le_bytes());
    fixed[26..28].copy_from_slice(&(workstation.len() as u16).to_le_bytes());
    fixed[28..32].copy_from_slice(&40u32.to_le_bytes());
    fixed[32] = 5;
    fixed[34..36].copy_from_slice(&2195u16.to_le_bytes());
    fixed[39] = 15;
    fixed.extend(workstation);
    fixed.extend(domain);
    fixed
}

pub fn nt_hash(password: &str) -> Vec<u8> {
    Md4::digest(utf16(password)).to_vec()
}

pub fn hmac_md5(data: &[u8], key: &[u8]) -> Result<Vec<u8>, NtlmError> {
    let mut mac =
        HmacMd5::new_from_slice(key).map_err(|_| NtlmError("invalid HMAC key".to_owned()))?;
    mac.update(data);
    Ok(mac.finalize().into_bytes().to_vec())
}

pub fn ntv2_hash(domain: &str, user: &str, password: &str) -> Result<Vec<u8>, NtlmError> {
    hmac_md5(
        &utf16(&(user.to_uppercase() + &domain.to_uppercase())),
        &nt_hash(password),
    )
}

pub fn lmv2_response(
    domain: &str,
    user: &str,
    password: &str,
    server_nonce: &[u8],
    client_nonce: &[u8],
) -> Result<Vec<u8>, NtlmError> {
    let hash = ntv2_hash(domain, user, password)?;
    let mut input = Vec::with_capacity(server_nonce.len() + client_nonce.len());
    input.extend_from_slice(server_nonce);
    input.extend_from_slice(client_nonce);
    let mut result = hmac_md5(&input, &hash)?;
    result.extend_from_slice(client_nonce);
    Ok(result)
}

pub fn create_timestamp(unix_millis: i64) -> [u8; 8] {
    let value = (i128::from(unix_millis) + 11_644_473_600_i128) * 10_000_000_i128;
    (value as u64).to_le_bytes()
}

pub fn ntlmv2_response(
    domain: &str,
    user: &str,
    password: &str,
    server_nonce: &[u8],
    target_info: &[u8],
    client_nonce: &[u8],
    unix_millis: i64,
) -> Result<Vec<u8>, NtlmError> {
    if server_nonce.len() != 8 {
        return Err(NtlmError(
            "NTLM server nonce must contain 8 bytes".to_owned(),
        ));
    }
    if client_nonce.len() != 8 {
        return Err(NtlmError(
            "NTLM client nonce must contain 8 bytes".to_owned(),
        ));
    }
    let hash = ntv2_hash(domain, user, password)?;
    let timestamp = create_timestamp(unix_millis);
    let mut data = vec![0u8; 40 + target_info.len()];
    data[0..8].copy_from_slice(server_nonce);
    data[8..12].copy_from_slice(&0x101u32.to_le_bytes());
    data[16..24].copy_from_slice(&timestamp);
    data[24..32].copy_from_slice(client_nonce);
    data[36..36 + target_info.len()].copy_from_slice(target_info);
    hmac_md5(&data, &hash)
}

pub fn create_ntlm_response(
    domain: &str,
    user: &str,
    password: &str,
    server_nonce: &[u8],
    target_info: &[u8],
    client_nonce: &[u8; 8],
    unix_millis: i64,
) -> Result<Vec<u8>, NtlmError> {
    if server_nonce.len() != 8 {
        return Err(NtlmError(
            "NTLM server nonce must contain 8 bytes".to_owned(),
        ));
    }
    let domain_bytes = utf16(domain);
    let user_bytes = utf16(user);
    let lm = lmv2_response(domain, user, password, server_nonce, client_nonce)?;
    let timestamp = create_timestamp(unix_millis);
    let hash = ntv2_hash(domain, user, password)?;
    let mut blob = vec![0u8; 32 + target_info.len()];
    blob[0..4].copy_from_slice(&0x101u32.to_le_bytes());
    blob[8..16].copy_from_slice(&timestamp);
    blob[16..24].copy_from_slice(client_nonce);
    blob[28..28 + target_info.len()].copy_from_slice(target_info);
    let nt_proof = hmac_md5(&[server_nonce, &blob].concat(), &hash)?;
    let mut nt = nt_proof;
    nt.extend(blob);
    let base = 64usize;
    let lm_offset = base + domain_bytes.len() + user_bytes.len();
    let nt_offset = lm_offset + lm.len();
    let mut data = vec![0u8; nt_offset + nt.len()];
    data[0..8].copy_from_slice(b"NTLMSSP\0");
    data[8..12].copy_from_slice(&3u32.to_le_bytes());
    data[12..14].copy_from_slice(&(lm.len() as u16).to_le_bytes());
    data[14..16].copy_from_slice(&(lm.len() as u16).to_le_bytes());
    data[16..20].copy_from_slice(&(lm_offset as u32).to_le_bytes());
    data[20..22].copy_from_slice(&16u16.to_le_bytes());
    data[22..24].copy_from_slice(&16u16.to_le_bytes());
    data[24..28].copy_from_slice(&(nt_offset as u32).to_le_bytes());
    data[28..30].copy_from_slice(&(domain_bytes.len() as u16).to_le_bytes());
    data[30..32].copy_from_slice(&(domain_bytes.len() as u16).to_le_bytes());
    data[32..36].copy_from_slice(&(base as u32).to_le_bytes());
    data[36..38].copy_from_slice(&(user_bytes.len() as u16).to_le_bytes());
    data[38..40].copy_from_slice(&(user_bytes.len() as u16).to_le_bytes());
    data[40..44].copy_from_slice(&((base + domain_bytes.len()) as u32).to_le_bytes());
    data[48..52].copy_from_slice(&(base as u32).to_le_bytes());
    data[56..60].copy_from_slice(&(base as u32).to_le_bytes());
    data[60..62].copy_from_slice(&0x8201u16.to_le_bytes());
    data[62..64].copy_from_slice(&8u16.to_le_bytes());
    data[base..base + domain_bytes.len()].copy_from_slice(&domain_bytes);
    data[base + domain_bytes.len()..lm_offset].copy_from_slice(&user_bytes);
    data[lm_offset..nt_offset].copy_from_slice(&lm);
    data[nt_offset..].copy_from_slice(&nt);
    Ok(data)
}
