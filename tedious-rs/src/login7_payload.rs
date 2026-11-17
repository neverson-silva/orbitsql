use std::fmt;

use crate::tds_versions::versions;

pub const FLAG2_INTEGRATED_SECURITY_ON: u8 = 0x80;
pub const TYPE_FLAG_READ_ONLY_INTENT: u8 = 0x20;
pub const FLAG3_EXTENSION_USED: u8 = 0x10;

#[derive(Clone, PartialEq, Eq)]
pub enum FedAuth {
    Adal { echo: bool, integrated: bool },
    SecurityToken { echo: bool, token: String },
}

impl fmt::Debug for FedAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Adal { echo, integrated } => formatter
                .debug_struct("Adal")
                .field("echo", echo)
                .field("integrated", integrated)
                .finish(),
            Self::SecurityToken { echo, token } => formatter
                .debug_struct("SecurityToken")
                .field("echo", echo)
                .field("token", &format_args!("[REDACTED; {} bytes]", token.len()))
                .finish(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login7Options {
    pub tds_version: u32,
    pub packet_size: u32,
    pub client_prog_ver: u32,
    pub client_pid: u32,
    pub connection_id: u32,
    pub client_time_zone: i32,
    pub client_lcid: u32,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Login7Payload {
    pub tds_version: u32,
    pub packet_size: u32,
    pub client_prog_ver: u32,
    pub client_pid: u32,
    pub connection_id: u32,
    pub client_time_zone: i32,
    pub client_lcid: u32,
    pub read_only_intent: bool,
    pub init_db_fatal: bool,
    pub user_name: Option<String>,
    pub password: Option<String>,
    pub server_name: Option<String>,
    pub app_name: Option<String>,
    pub hostname: Option<String>,
    pub library_name: Option<String>,
    pub language: Option<String>,
    pub database: Option<String>,
    pub client_id: Option<Vec<u8>>,
    pub sspi: Option<Vec<u8>>,
    pub attach_db_file: Option<String>,
    pub change_password: Option<String>,
    pub fed_auth: Option<FedAuth>,
}

impl fmt::Debug for Login7Payload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Login7Payload")
            .field("tds_version", &self.tds_version)
            .field("packet_size", &self.packet_size)
            .field("client_prog_ver", &self.client_prog_ver)
            .field("client_pid", &self.client_pid)
            .field("connection_id", &self.connection_id)
            .field("client_time_zone", &self.client_time_zone)
            .field("client_lcid", &self.client_lcid)
            .field("read_only_intent", &self.read_only_intent)
            .field("init_db_fatal", &self.init_db_fatal)
            .field("user_name", &self.user_name)
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("server_name", &self.server_name)
            .field("app_name", &self.app_name)
            .field("hostname", &self.hostname)
            .field("library_name", &self.library_name)
            .field("language", &self.language)
            .field("database", &self.database)
            .field("client_id", &self.client_id.as_ref().map(Vec::len))
            .field(
                "sspi",
                &self
                    .sspi
                    .as_ref()
                    .map(|value| format!("[REDACTED; {} bytes]", value.len())),
            )
            .field("attach_db_file", &self.attach_db_file)
            .field(
                "change_password",
                &self.change_password.as_ref().map(|_| "[REDACTED]"),
            )
            .field("fed_auth", &self.fed_auth)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Login7Error(pub String);

fn utf16(value: &str) -> Vec<u8> {
    value.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn write_u16(data: &mut [u8], offset: usize, value: usize) -> Result<(), Login7Error> {
    let value = u16::try_from(value)
        .map_err(|_| Login7Error("LOGIN7 value exceeds u16 wire capacity".to_owned()))?;
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    Ok(())
}
fn write_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn append_string(
    fixed: &mut [u8],
    variable: &mut Vec<u8>,
    data_offset: &mut usize,
    field: usize,
    value: &Option<String>,
) -> Result<(), Login7Error> {
    if let Some(value) = value {
        write_u16(fixed, field, *data_offset)?;
        let bytes = utf16(value);
        write_u16(fixed, field + 2, bytes.len() / 2)?;
        variable.extend(bytes);
        *data_offset = fixed.len() + variable.len();
    } else {
        let offset = u16::try_from(*data_offset).unwrap_or(0);
        fixed[field..field + 2].copy_from_slice(&offset.to_le_bytes());
        write_u16(fixed, field + 2, 0)?;
    }
    Ok(())
}

impl Login7Payload {
    pub fn new(options: Login7Options) -> Self {
        Self {
            tds_version: options.tds_version,
            packet_size: options.packet_size,
            client_prog_ver: options.client_prog_ver,
            client_pid: options.client_pid,
            connection_id: options.connection_id,
            client_time_zone: options.client_time_zone,
            client_lcid: options.client_lcid,
            read_only_intent: false,
            init_db_fatal: false,
            user_name: None,
            password: None,
            server_name: None,
            app_name: None,
            hostname: None,
            library_name: None,
            language: None,
            database: None,
            client_id: None,
            sspi: None,
            attach_db_file: None,
            change_password: None,
            fed_auth: None,
        }
    }

    fn flags1(&self) -> u8 {
        0x10 | 0x20 | if self.init_db_fatal { 0x40 } else { 0 } | 0x80
    }
    fn flags2(&self) -> u8 {
        if self.sspi.is_some() {
            FLAG2_INTEGRATED_SECURITY_ON
        } else {
            0
        }
    }
    fn type_flags(&self) -> u8 {
        if self.read_only_intent {
            TYPE_FLAG_READ_ONLY_INTENT
        } else {
            0
        }
    }
    fn flags3(&self) -> u8 {
        0x08 | FLAG3_EXTENSION_USED
    }

    fn feature_ext(&self) -> Vec<u8> {
        let mut result = Vec::new();
        if let Some(fed_auth) = &self.fed_auth {
            match fed_auth {
                FedAuth::Adal { echo, integrated } => result.extend([
                    2,
                    2,
                    0,
                    0,
                    0,
                    if *echo { 5 } else { 4 },
                    if *integrated { 2 } else { 1 },
                ]),
                FedAuth::SecurityToken { echo, token } => {
                    let token = utf16(token);
                    result.push(2);
                    result.extend((token.len() as u32 + 5).to_le_bytes());
                    result.push(if *echo { 3 } else { 2 });
                    result.extend((token.len() as i32).to_le_bytes());
                    result.extend(token);
                }
            }
        }
        result.extend([0x0A, 1, 0, 0, 0, 1, 0xFF]);
        result
    }

    pub fn to_buffer(&self) -> Result<Vec<u8>, Login7Error> {
        self.validate_lengths()?;
        let mut fixed = vec![0u8; 94];
        write_u32(&mut fixed, 4, self.tds_version);
        write_u32(&mut fixed, 8, self.packet_size);
        write_u32(&mut fixed, 12, self.client_prog_ver);
        write_u32(&mut fixed, 16, self.client_pid);
        write_u32(&mut fixed, 20, self.connection_id);
        fixed[24] = self.flags1();
        fixed[25] = self.flags2();
        fixed[26] = self.type_flags();
        fixed[27] = self.flags3();
        fixed[28..32].copy_from_slice(&self.client_time_zone.to_le_bytes());
        write_u32(&mut fixed, 32, self.client_lcid);
        let mut variable = Vec::new();
        let mut data_offset = fixed.len();
        if self.hostname.is_some() {
            append_string(
                &mut fixed,
                &mut variable,
                &mut data_offset,
                36,
                &self.hostname,
            )?;
        } else {
            write_u16(&mut fixed, 36, data_offset)?;
            write_u16(&mut fixed, 38, data_offset)?;
        }
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            40,
            &self.user_name,
        )?;
        write_u16(&mut fixed, 44, data_offset)?;
        if let Some(value) = &self.password {
            let bytes = scramble(utf16(value));
            write_u16(&mut fixed, 46, bytes.len() / 2)?;
            variable.extend(bytes);
            data_offset = fixed.len() + variable.len();
        } else {
            write_u16(&mut fixed, 46, 0)?;
        }
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            48,
            &self.app_name,
        )?;
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            52,
            &self.server_name,
        )?;
        let mut extension_slot = None;
        write_u16(&mut fixed, 56, data_offset)?;
        if self.tds_version >= versions()["7_4"] {
            write_u16(&mut fixed, 58, 4)?;
            extension_slot = Some(variable.len());
            variable.extend([0; 4]);
            data_offset += 4;
        } else {
            write_u16(&mut fixed, 58, 0)?;
        }
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            60,
            &self.library_name,
        )?;
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            64,
            &self.language,
        )?;
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            68,
            &self.database,
        )?;
        if let Some(client_id) = &self.client_id {
            if client_id.len() > 6 {
                return Err(Login7Error(
                    "client_id exceeds LOGIN7 six-byte length".to_owned(),
                ));
            }
            fixed[72..72 + client_id.len()].copy_from_slice(client_id);
        }
        write_u16(&mut fixed, 78, data_offset)?;
        if let Some(sspi) = &self.sspi {
            write_u16(&mut fixed, 80, sspi.len().min(usize::from(u16::MAX)))?;
            variable.extend(sspi);
            data_offset += sspi.len();
        } else {
            write_u16(&mut fixed, 80, 0)?;
        }
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            82,
            &self.attach_db_file,
        )?;
        append_string(
            &mut fixed,
            &mut variable,
            &mut data_offset,
            86,
            &self.change_password,
        )?;
        let sspi_long = self
            .sspi
            .as_ref()
            .filter(|value| value.len() > u16::MAX as usize)
            .map_or(0, |value| value.len() as u32);
        write_u32(&mut fixed, 90, sspi_long);
        if let Some(slot) = extension_slot {
            let feature_offset = data_offset;
            let feature_offset = u32::try_from(feature_offset)
                .map_err(|_| Login7Error("LOGIN7 feature offset exceeds u32".to_owned()))?;
            variable[slot..slot + 4].copy_from_slice(&feature_offset.to_le_bytes());
            variable.extend(self.feature_ext());
        }
        let total = fixed.len() + variable.len();
        let total = u32::try_from(total)
            .map_err(|_| Login7Error("LOGIN7 payload exceeds u32".to_owned()))?;
        write_u32(&mut fixed, 0, total);
        fixed.extend(variable);
        Ok(fixed)
    }

    fn validate_lengths(&self) -> Result<(), Login7Error> {
        for (name, value) in [
            ("hostname", &self.hostname),
            ("user_name", &self.user_name),
            ("password", &self.password),
            ("app_name", &self.app_name),
            ("server_name", &self.server_name),
            ("library_name", &self.library_name),
            ("language", &self.language),
            ("database", &self.database),
            ("attach_db_file", &self.attach_db_file),
            ("change_password", &self.change_password),
        ] {
            if let Some(value) = value
                && value.encode_utf16().count() > usize::from(u16::MAX)
            {
                return Err(Login7Error(format!("{name} exceeds LOGIN7 u16 length")));
            }
        }
        let feature_length = self.feature_ext().len();
        if feature_length > usize::try_from(u32::MAX).unwrap_or(usize::MAX) {
            return Err(Login7Error(
                "LOGIN7 feature extension is too large".to_owned(),
            ));
        }
        if let Some(client_id) = &self.client_id
            && client_id.len() > 6
        {
            return Err(Login7Error(
                "client_id exceeds LOGIN7 six-byte length".to_owned(),
            ));
        }
        if let Some(sspi) = &self.sspi {
            u32::try_from(sspi.len())
                .map_err(|_| Login7Error("SSPI exceeds LOGIN7 u32 length".to_owned()))?;
        }
        if let Some(FedAuth::SecurityToken { token, .. }) = &self.fed_auth {
            let token_length = utf16(token).len();
            if token_length > usize::try_from(u32::MAX - 5).unwrap_or(usize::MAX) {
                return Err(Login7Error(
                    "FedAuth token exceeds LOGIN7 feature length".to_owned(),
                ));
            }
            if token_length > usize::try_from(i32::MAX).unwrap_or(usize::MAX) {
                return Err(Login7Error(
                    "FedAuth token exceeds LOGIN7 signed length".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

fn scramble(mut value: Vec<u8>) -> Vec<u8> {
    for byte in &mut value {
        *byte = ((*byte & 0x0F) << 4 | (*byte >> 4)) ^ 0xA5;
    }
    value
}
