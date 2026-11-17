use rand::random;

pub const TOKEN_VERSION: u8 = 0x00;
pub const TOKEN_ENCRYPTION: u8 = 0x01;
pub const TOKEN_INSTANCE: u8 = 0x02;
pub const TOKEN_THREAD_ID: u8 = 0x03;
pub const TOKEN_MARS: u8 = 0x04;
pub const TOKEN_TRACE_ID: u8 = 0x05;
pub const TOKEN_FED_AUTH_REQUIRED: u8 = 0x06;
pub const TOKEN_TERMINATOR: u8 = 0xFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreloginVersion {
    pub major: u8,
    pub minor: u8,
    pub build: u16,
    pub subbuild: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreloginOptions {
    pub encrypt: bool,
    pub version: PreloginVersion,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreloginPayload {
    pub data: Vec<u8>,
    pub version: PreloginVersion,
    pub encryption: u8,
    pub encryption_string: Option<&'static str>,
    pub instance: u8,
    pub thread_id: u32,
    pub mars: u8,
    pub mars_string: Option<&'static str>,
    pub trace_id: Vec<u8>,
    pub fed_auth_required: u8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreloginError(pub String);

fn encryption_name(value: u8) -> Option<&'static str> {
    match value {
        0 => Some("OFF"),
        1 => Some("ON"),
        2 => Some("NOT_SUP"),
        3 => Some("REQ"),
        _ => None,
    }
}
fn mars_name(value: u8) -> Option<&'static str> {
    match value {
        0 => Some("OFF"),
        1 => Some("ON"),
        _ => None,
    }
}

impl PreloginPayload {
    pub fn to_string(&self, indent: &str) -> String {
        format!(
            "{}PreLogin - version:{}.{}.{}.{}, encryption:0x{:02X}({}), instopt:0x{:02X}, threadId:0x{:08X}, mars:0x{:02X}({}), traceId:{}",
            indent,
            self.version.major,
            self.version.minor,
            self.version.build,
            self.version.subbuild,
            self.encryption,
            self.encryption_string.unwrap_or(""),
            self.instance,
            self.thread_id,
            self.mars,
            self.mars_string.unwrap_or(""),
            self.trace_id
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    }

    pub fn new(options: PreloginOptions) -> Result<Self, PreloginError> {
        let trace_id: [u8; 36] = random();
        let mut payload = Self {
            data: Vec::new(),
            version: options.version,
            encryption: if options.encrypt { 1 } else { 2 },
            encryption_string: None,
            instance: 0,
            thread_id: 0,
            mars: 0,
            mars_string: None,
            trace_id: trace_id.to_vec(),
            fed_auth_required: 1,
        };
        payload.create_options();
        payload.extract_options()?;
        Ok(payload)
    }

    pub fn from_bytes(data: Vec<u8>) -> Result<Self, PreloginError> {
        let mut payload = Self {
            data,
            version: PreloginVersion {
                major: 0,
                minor: 0,
                build: 0,
                subbuild: 0,
            },
            encryption: 0,
            encryption_string: None,
            instance: 0,
            thread_id: 0,
            mars: 0,
            mars_string: None,
            trace_id: Vec::new(),
            fed_auth_required: 0,
        };
        payload.extract_options()?;
        Ok(payload)
    }

    fn create_options(&mut self) {
        let values = [
            (
                TOKEN_VERSION,
                vec![
                    self.version.major,
                    self.version.minor,
                    (self.version.build >> 8) as u8,
                    self.version.build as u8,
                    (self.version.subbuild >> 8) as u8,
                    self.version.subbuild as u8,
                ],
            ),
            (TOKEN_ENCRYPTION, vec![self.encryption]),
            (TOKEN_INSTANCE, vec![0]),
            (TOKEN_THREAD_ID, vec![0, 0, 0, 0]),
            (TOKEN_MARS, vec![0]),
            (TOKEN_TRACE_ID, self.trace_id.clone()),
            (TOKEN_FED_AUTH_REQUIRED, vec![1]),
        ];
        let header_length = values.len() * 5 + 1;
        let data_length: usize = values.iter().map(|(_, value)| value.len()).sum();
        self.data = vec![0; header_length + data_length];
        let mut header = 0;
        let mut data = header_length;
        for (token, value) in values {
            self.data[header] = token;
            self.data[header + 1..header + 3].copy_from_slice(&(data as u16).to_be_bytes());
            self.data[header + 3..header + 5].copy_from_slice(&(value.len() as u16).to_be_bytes());
            self.data[data..data + value.len()].copy_from_slice(&value);
            header += 5;
            data += value.len();
        }
        self.data[header] = TOKEN_TERMINATOR;
    }

    fn extract_options(&mut self) -> Result<(), PreloginError> {
        let mut offset = 0;
        loop {
            if offset >= self.data.len() {
                return Err(PreloginError("prelogin terminator missing".to_owned()));
            }
            if self.data[offset] == TOKEN_TERMINATOR {
                return Ok(());
            }
            if self.data.len() < offset + 5 {
                return Err(PreloginError("truncated prelogin option header".to_owned()));
            }
            let token = self.data[offset];
            let data_offset =
                u16::from_be_bytes([self.data[offset + 1], self.data[offset + 2]]) as usize;
            let length =
                u16::from_be_bytes([self.data[offset + 3], self.data[offset + 4]]) as usize;
            if data_offset.checked_add(length).is_none() || data_offset + length > self.data.len() {
                return Err(PreloginError("truncated prelogin option data".to_owned()));
            }
            let value = &self.data[data_offset..data_offset + length];
            match token {
                TOKEN_VERSION if value.len() >= 6 => {
                    self.version = PreloginVersion {
                        major: value[0],
                        minor: value[1],
                        build: u16::from_be_bytes([value[2], value[3]]),
                        subbuild: u16::from_be_bytes([value[4], value[5]]),
                    }
                }
                TOKEN_ENCRYPTION if !value.is_empty() => {
                    self.encryption = value[0];
                    self.encryption_string = encryption_name(value[0]);
                }
                TOKEN_INSTANCE if !value.is_empty() => self.instance = value[0],
                TOKEN_THREAD_ID if value.len() >= 4 => {
                    self.thread_id = u32::from_be_bytes([value[0], value[1], value[2], value[3]])
                }
                TOKEN_MARS if !value.is_empty() => {
                    self.mars = value[0];
                    self.mars_string = mars_name(value[0]);
                }
                TOKEN_TRACE_ID => self.trace_id = value[..value.len().min(36)].to_vec(),
                TOKEN_FED_AUTH_REQUIRED if !value.is_empty() => self.fed_auth_required = value[0],
                _ => {}
            }
            offset += 5;
        }
    }
}
