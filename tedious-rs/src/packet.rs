pub const HEADER_LENGTH: usize = 8;

pub mod packet_type {
    pub const SQL_BATCH: u8 = 0x01;
    pub const RPC_REQUEST: u8 = 0x03;
    pub const TABULAR_RESULT: u8 = 0x04;
    pub const ATTENTION: u8 = 0x06;
    pub const BULK_LOAD: u8 = 0x07;
    pub const FEDAUTH_TOKEN: u8 = 0x08;
    pub const TRANSACTION_MANAGER: u8 = 0x0E;
    pub const LOGIN7: u8 = 0x10;
    pub const NTLMAUTH_PKT: u8 = 0x11;
    pub const PRELOGIN: u8 = 0x12;
}

pub mod status {
    pub const NORMAL: u8 = 0x00;
    pub const EOM: u8 = 0x01;
    pub const IGNORE: u8 = 0x02;
    pub const RESET_CONNECTION: u8 = 0x08;
    pub const RESET_CONNECTION_SKIP_TRAN: u8 = 0x10;
}

pub struct Packet {
    pub buffer: Vec<u8>,
}

impl Packet {
    pub fn new(packet_type: u8) -> Self {
        let mut buffer = vec![0; HEADER_LENGTH];
        buffer[0] = packet_type;
        buffer[1] = status::NORMAL;
        buffer[6] = 1;
        buffer[2..4].copy_from_slice(&(HEADER_LENGTH as u16).to_be_bytes());
        Self { buffer }
    }

    pub fn from_buffer(buffer: Vec<u8>) -> Result<Self, String> {
        if buffer.len() < HEADER_LENGTH {
            return Err("packet buffer is shorter than the header".to_owned());
        }
        let length = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
        if length < HEADER_LENGTH {
            return Err("packet length is shorter than the header".to_owned());
        }
        if length != buffer.len() {
            return Err("packet length does not match the buffer".to_owned());
        }
        Ok(Self { buffer })
    }
    pub fn set_length(&mut self) -> Result<(), String> {
        if self.buffer.len() < HEADER_LENGTH {
            return Err("packet buffer is shorter than the header".to_owned());
        }
        let n =
            u16::try_from(self.buffer.len()).map_err(|_| "packet length exceeds u16".to_owned())?;
        self.buffer[2..4].copy_from_slice(&n.to_be_bytes());
        Ok(())
    }
    pub fn length(&self) -> u16 {
        self.buffer
            .get(2..4)
            .and_then(|value| value.try_into().ok())
            .map(u16::from_be_bytes)
            .unwrap_or(0)
    }
    pub fn reset_connection(&mut self, reset: bool) {
        if reset {
            if self.buffer.len() > 1 {
                self.buffer[1] |= status::RESET_CONNECTION
            }
        } else {
            if self.buffer.len() > 1 {
                self.buffer[1] &= !status::RESET_CONNECTION
            }
        }
    }
    pub fn last(&mut self, last: Option<bool>) -> bool {
        if let Some(value) = last {
            if value {
                if self.buffer.len() > 1 {
                    self.buffer[1] |= status::EOM
                }
            } else {
                if self.buffer.len() > 1 {
                    self.buffer[1] &= !status::EOM
                }
            }
        }
        self.is_last()
    }
    pub fn ignore(&mut self, last: bool) {
        if self.buffer.len() > 1 {
            if last {
                self.buffer[1] |= status::IGNORE
            } else {
                self.buffer[1] &= !status::IGNORE
            }
        }
    }
    pub fn is_last(&self) -> bool {
        self.buffer
            .get(1)
            .is_some_and(|value| value & status::EOM != 0)
    }
    pub fn packet_id(&mut self, packet_id: Option<u8>) -> u8 {
        if let Some(value) = packet_id
            && value != 0
            && self.buffer.len() > 6
        {
            self.buffer[6] = value
        }
        self.buffer.get(6).copied().unwrap_or(0)
    }
    pub fn add_data(&mut self, data: &[u8]) -> Result<&mut Self, String> {
        if self.buffer.len() < HEADER_LENGTH {
            return Err("packet buffer is shorter than the header".to_owned());
        }
        let length = self
            .buffer
            .len()
            .checked_add(data.len())
            .ok_or_else(|| "packet length overflow".to_owned())?;
        if length > u16::MAX as usize {
            return Err("packet length exceeds u16".to_owned());
        }
        self.buffer.extend_from_slice(data);
        self.set_length()?;
        Ok(self)
    }
    pub fn data(&self) -> &[u8] {
        self.buffer.get(HEADER_LENGTH..).unwrap_or(&[])
    }
    pub fn packet_type(&self) -> u8 {
        self.buffer.first().copied().unwrap_or(0)
    }
    pub fn status(&self) -> u8 {
        self.buffer.get(1).copied().unwrap_or(0)
    }

    pub fn status_as_string(&self) -> String {
        let status = self.status();
        let statuses = [
            (status::NORMAL, "NORMAL"),
            (status::EOM, "EOM"),
            (status::IGNORE, "IGNORE"),
            (status::RESET_CONNECTION, "RESETCONNECTION"),
            (
                status::RESET_CONNECTION_SKIP_TRAN,
                "RESETCONNECTIONSKIPTRAN",
            ),
        ];
        statuses
            .iter()
            .map(|(value, name)| if status & value != 0 { *name } else { "" })
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_owned()
    }

    pub fn header_to_string(&self, indent: &str) -> String {
        let type_name = match self.packet_type() {
            packet_type::SQL_BATCH => "SQL_BATCH",
            packet_type::RPC_REQUEST => "RPC_REQUEST",
            packet_type::TABULAR_RESULT => "TABULAR_RESULT",
            packet_type::ATTENTION => "ATTENTION",
            packet_type::BULK_LOAD => "BULK_LOAD",
            packet_type::FEDAUTH_TOKEN => "FEDAUTH_TOKEN",
            packet_type::TRANSACTION_MANAGER => "TRANSACTION_MANAGER",
            packet_type::LOGIN7 => "LOGIN7",
            packet_type::NTLMAUTH_PKT => "NTLMAUTH_PKT",
            packet_type::PRELOGIN => "PRELOGIN",
            _ => "undefined",
        };
        let spid = self
            .buffer
            .get(4..6)
            .and_then(|value| value.try_into().ok())
            .map(u16::from_be_bytes)
            .unwrap_or(0);
        format!(
            "{indent}type:0x{:02X}({type_name}), status:0x{:02X}({}), length:0x{:04X}, spid:0x{:04X}, packetId:0x{:02X}, window:0x{:02X}",
            self.packet_type(),
            self.status(),
            self.status_as_string(),
            self.length(),
            spid,
            self.buffer.get(6).copied().unwrap_or(0),
            self.buffer.get(7).copied().unwrap_or(0)
        )
    }

    pub fn data_to_string(&self, indent: &str) -> String {
        let mut data_dump = String::new();
        let mut chars = String::new();
        for (offset, byte) in self.data().iter().enumerate() {
            if offset % 0x20 == 0 {
                data_dump.push_str(indent);
                data_dump.push_str(&format!("{offset:04X}  "));
            }
            if *byte < 0x20 || *byte > 0x7E {
                chars.push('.');
                if (offset + 1) % 8 == 0 && (offset + 1) % 0x20 != 0 {
                    chars.push(' ');
                }
            } else {
                chars.push(*byte as char);
            }
            data_dump.push_str(&format!("{byte:02X}"));
            if (offset + 1) % 4 == 0 && (offset + 1) % 0x20 != 0 {
                data_dump.push(' ');
            }
            if (offset + 1) % 0x20 == 0 {
                data_dump.push_str("  ");
                data_dump.push_str(&chars);
                chars.clear();
                if offset + 1 < self.data().len() {
                    data_dump.push('\n');
                }
            }
        }
        if !chars.is_empty() {
            data_dump.push_str("  ");
            data_dump.push_str(&chars);
        }
        data_dump
    }

    pub fn to_string(&self, indent: &str) -> String {
        format!(
            "{}\n{}",
            self.header_to_string(indent),
            self.data_to_string(&format!("{indent}{indent}"))
        )
    }

    pub fn to_string_with_indent(&self, indent: &str) -> String {
        self.to_string(indent)
    }

    pub fn payload_string(&self) -> String {
        String::new()
    }
}

pub fn is_packet_complete(buffer: &[u8]) -> bool {
    if buffer.len() < HEADER_LENGTH {
        return false;
    }
    let length = u16::from_be_bytes([buffer[2], buffer[3]]) as usize;
    buffer.len() >= length
}
pub fn packet_length(buffer: &[u8]) -> u16 {
    buffer
        .get(2..4)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_be_bytes)
        .unwrap_or(0)
}
