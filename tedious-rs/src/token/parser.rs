use crate::errors::{NotEnoughDataError, ParseResult};
use crate::metadata_parser::{Metadata, read_metadata};
use crate::ntlm::{NtlmChallenge, parse_challenge};
use crate::token::helpers::{
    read_b_var_byte, read_b_var_char, read_i32_le, read_u8, read_u16_le, read_u32_le, read_u64_le,
    read_us_var_byte, read_us_var_char,
};
use crate::token::rows::{ColumnMetadata, Row, RowColumn, TableName};
use crate::value_codec::Value;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DoneToken {
    pub token: u8,
    pub more: bool,
    pub sql_error: bool,
    pub attention: bool,
    pub server_error: bool,
    pub row_count: Option<u64>,
    pub cur_cmd: u16,
}
#[derive(Clone, PartialEq, Eq)]
pub struct MessageToken {
    pub token: u8,
    pub number: u32,
    pub state: u8,
    pub class: u8,
    pub message: String,
    pub server_name: String,
    pub proc_name: String,
    pub line_number: u32,
}
impl fmt::Debug for MessageToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MessageToken")
            .field("token", &self.token)
            .field("number", &self.number)
            .field("state", &self.state)
            .field("class", &self.class)
            .field(
                "message",
                &format_args!("[REDACTED; {} bytes]", self.message.len()),
            )
            .field(
                "server_name",
                &format_args!("[REDACTED; {} bytes]", self.server_name.len()),
            )
            .field(
                "proc_name",
                &format_args!("[REDACTED; {} bytes]", self.proc_name.len()),
            )
            .field("line_number", &self.line_number)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnStatusToken {
    pub value: i32,
}
#[derive(Clone, PartialEq)]
pub struct ReturnValueToken {
    pub ordinal: u16,
    pub name: String,
    pub status: u8,
    pub metadata: Metadata,
    pub value: Value,
}
impl fmt::Debug for ReturnValueToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReturnValueToken")
            .field("ordinal", &self.ordinal)
            .field(
                "name",
                &format_args!("[REDACTED; {} bytes]", self.name.len()),
            )
            .field("status", &self.status)
            .field("metadata", &"[REDACTED]")
            .field("value", &self.value)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginAckToken {
    pub interface: Option<String>,
    pub tds_version: Option<String>,
    pub prog_name: String,
    pub prog_version: [u8; 4],
}
#[derive(Clone, PartialEq, Eq)]
pub struct SspiToken {
    pub ntlm_packet: NtlmChallenge,
    pub buffer: Vec<u8>,
}
impl fmt::Debug for SspiToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SspiToken")
            .field("ntlm_packet", &self.ntlm_packet)
            .field(
                "buffer",
                &format_args!("[REDACTED; {} bytes]", self.buffer.len()),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct FeatureExtAckToken {
    pub fed_auth: Option<Vec<u8>>,
    pub utf8_support: Option<bool>,
}
impl fmt::Debug for FeatureExtAckToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("FeatureExtAckToken")
            .field(
                "fed_auth",
                &self
                    .fed_auth
                    .as_ref()
                    .map(|value| format!("[REDACTED; {} bytes]", value.len())),
            )
            .field("utf8_support", &self.utf8_support)
            .finish()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct FedAuthInfoToken {
    pub spn: Option<String>,
    pub sts_url: Option<String>,
}
impl fmt::Debug for FedAuthInfoToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("FedAuthInfoToken");
        match &self.spn {
            Some(value) => debug.field("spn", &format_args!("[REDACTED; {} bytes]", value.len())),
            None => debug.field("spn", &None::<String>),
        };
        match &self.sts_url {
            Some(value) => debug.field(
                "sts_url",
                &format_args!("[REDACTED; {} bytes]", value.len()),
            ),
            None => debug.field("sts_url", &None::<String>),
        };
        debug.finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RoutingChange {
    pub protocol: u8,
    pub port: u16,
    pub server: String,
    pub old_value: Vec<u8>,
}

impl fmt::Debug for RoutingChange {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutingChange")
            .field("protocol", &self.protocol)
            .field("port", &self.port)
            .field(
                "server",
                &format_args!("[REDACTED; {} bytes]", self.server.len()),
            )
            .field(
                "old_value",
                &format_args!("[REDACTED; {} bytes]", self.old_value.len()),
            )
            .finish()
    }
}

impl RoutingChange {
    pub fn server_and_instance(&self) -> (String, Option<String>) {
        self.server
            .split_once('\\')
            .map_or((self.server.clone(), None), |(server, instance)| {
                (server.to_owned(), Some(instance.to_owned()))
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OffsetToken {
    pub identifier: u16,
    pub offset: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabNameToken {
    pub tables: Vec<TableName>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnInfo {
    pub column_number: u8,
    pub table_number: u8,
    pub status: u8,
    pub column_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColInfoToken {
    pub columns: Vec<ColumnInfo>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionStateEntry {
    pub state_id: u8,
    pub value: Vec<u8>,
}

impl fmt::Debug for SessionStateEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionStateEntry")
            .field("state_id", &self.state_id)
            .field(
                "value",
                &format_args!("[REDACTED; {} bytes]", self.value.len()),
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionStateToken {
    pub sequence_number: u32,
    pub recoverable: bool,
    pub entries: Vec<SessionStateEntry>,
}

impl fmt::Debug for SessionStateToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SessionStateToken")
            .field("sequence_number", &self.sequence_number)
            .field("recoverable", &self.recoverable)
            .field("entries", &self.entries)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SensitivityLabel {
    pub name: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InformationType {
    pub name: String,
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SensitivityProperty {
    pub label_index: u16,
    pub information_type_index: u16,
    pub rank: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataClassificationToken {
    pub labels: Vec<SensitivityLabel>,
    pub information_types: Vec<InformationType>,
    pub rank: Option<i32>,
    pub columns: Vec<Vec<SensitivityProperty>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AltMetadataColumn {
    pub operator: u8,
    pub operand: u16,
    pub metadata: crate::metadata_parser::Metadata,
    pub column_name: String,
    pub table_name: Option<TableName>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AltMetadataToken {
    pub id: u16,
    pub by_columns: Vec<u16>,
    pub columns: Vec<AltMetadataColumn>,
}

#[derive(Clone, PartialEq)]
pub struct AltRowToken {
    pub id: u16,
    pub row: Row,
}

impl fmt::Debug for AltRowToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AltRowToken")
            .field("id", &self.id)
            .field("row", &self.row)
            .finish()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub enum EnvChangeKind {
    Text {
        new_value: String,
        old_value: String,
    },
    PacketSize {
        new_value: u32,
        old_value: u32,
    },
    Binary {
        new_value: Vec<u8>,
        old_value: Vec<u8>,
    },
    Collation {
        new_value: Option<crate::collation::Collation>,
        old_value: Option<crate::collation::Collation>,
    },
    BeginTransaction {
        new_value: Vec<u8>,
        old_value: Vec<u8>,
    },
    CommitTransaction {
        new_value: Vec<u8>,
        old_value: Vec<u8>,
    },
    RollbackTransaction {
        new_value: Vec<u8>,
        old_value: Vec<u8>,
    },
    ResetConnection {
        new_value: Vec<u8>,
        old_value: Vec<u8>,
    },
    Unknown {
        new_value: Vec<u8>,
        old_value: Vec<u8>,
    },
    Routing(RoutingChange),
}
impl fmt::Debug for EnvChangeKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Text {
                new_value,
                old_value,
            } => formatter
                .debug_struct("Text")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::PacketSize {
                new_value,
                old_value,
            } => formatter
                .debug_struct("PacketSize")
                .field("new_value", new_value)
                .field("old_value", old_value)
                .finish(),
            Self::Binary {
                new_value,
                old_value,
            } => formatter
                .debug_struct("Binary")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::Collation {
                new_value,
                old_value,
            } => formatter
                .debug_struct("Collation")
                .field("new_value", new_value)
                .field("old_value", old_value)
                .finish(),
            Self::BeginTransaction {
                new_value,
                old_value,
            } => formatter
                .debug_struct("BeginTransaction")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::CommitTransaction {
                new_value,
                old_value,
            } => formatter
                .debug_struct("CommitTransaction")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::RollbackTransaction {
                new_value,
                old_value,
            } => formatter
                .debug_struct("RollbackTransaction")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::ResetConnection {
                new_value,
                old_value,
            } => formatter
                .debug_struct("ResetConnection")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::Unknown {
                new_value,
                old_value,
            } => formatter
                .debug_struct("Unknown")
                .field(
                    "new_value",
                    &format_args!("[REDACTED; {} bytes]", new_value.len()),
                )
                .field(
                    "old_value",
                    &format_args!("[REDACTED; {} bytes]", old_value.len()),
                )
                .finish(),
            Self::Routing(route) => formatter.debug_tuple("Routing").field(route).finish(),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct EnvChangeToken {
    pub change_type: u8,
    pub kind: EnvChangeKind,
}
impl fmt::Debug for EnvChangeToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EnvChangeToken")
            .field("change_type", &self.change_type)
            .field("kind", &self.kind)
            .finish()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Done(DoneToken),
    Info(MessageToken),
    Error(MessageToken),
    ReturnStatus(ReturnStatusToken),
    ReturnValue(Box<ReturnValueToken>),
    LoginAck(LoginAckToken),
    Sspi(SspiToken),
    FeatureExtAck(FeatureExtAckToken),
    FedAuthInfo(FedAuthInfoToken),
    Order(Vec<u16>),
    EnvChange(EnvChangeToken),
    Offset(OffsetToken),
    TabName(TabNameToken),
    ColInfo(ColInfoToken),
    DataClassification(DataClassificationToken),
    AltMetadata(AltMetadataToken),
    AltRow(AltRowToken),
    SessionState(SessionStateToken),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenParseError {
    NotEnoughData(NotEnoughDataError),
    Unsupported(u8),
    Invalid(String),
}
impl From<NotEnoughDataError> for TokenParseError {
    fn from(value: NotEnoughDataError) -> Self {
        Self::NotEnoughData(value)
    }
}

fn done(
    buf: &[u8],
    offset: usize,
    token: u8,
    tds_version: &str,
) -> Result<ParseResult<Token>, TokenParseError> {
    let status = read_u16_le(buf, offset)?;
    let cur_cmd = read_u16_le(buf, status.offset)?;
    let count = if tds_version < "7_2" {
        let value = read_u32_le(buf, cur_cmd.offset)?;
        (u64::from(value.value), value.offset)
    } else {
        let value = read_u64_le(buf, cur_cmd.offset)?;
        (value.value, value.offset)
    };
    Ok(ParseResult::new(
        Token::Done(DoneToken {
            token,
            more: status.value & 1 != 0,
            sql_error: status.value & 2 != 0,
            attention: status.value & 0x20 != 0,
            server_error: status.value & 0x100 != 0,
            row_count: (status.value & 0x10 != 0).then_some(count.0),
            cur_cmd: cur_cmd.value,
        }),
        count.1,
    ))
}

fn message(
    buf: &[u8],
    offset: usize,
    token: u8,
    tds_version: &str,
) -> Result<ParseResult<Token>, TokenParseError> {
    let length = read_u16_le(buf, offset)?;
    if buf.len() < length.offset.saturating_add(length.value as usize) {
        return Err(NotEnoughDataError::new(length.offset + length.value as usize).into());
    }
    let number = read_u32_le(buf, length.offset)?;
    let state = read_u8(buf, number.offset)?;
    let class = read_u8(buf, state.offset)?;
    let text = read_us_var_char(buf, class.offset)?;
    let server = read_b_var_char(buf, text.offset)?;
    let procedure = read_b_var_char(buf, server.offset)?;
    let line = if tds_version < "7_2" {
        u32::from(read_u16_le(buf, procedure.offset)?.value)
    } else {
        read_u32_le(buf, procedure.offset)?.value
    };
    Ok(ParseResult::new(
        Token::Info(MessageToken {
            token,
            number: number.value,
            state: state.value,
            class: class.value,
            message: text.value,
            server_name: server.value,
            proc_name: procedure.value,
            line_number: line,
        }),
        length.offset + length.value as usize,
    ))
}

fn length_prefixed(
    buf: &[u8],
    offset: usize,
    width: usize,
) -> Result<ParseResult<Vec<u8>>, TokenParseError> {
    let length = if width == 2 {
        read_u16_le(buf, offset)?.value as usize
    } else {
        read_u32_le(buf, offset)?.value as usize
    };
    let start = offset + width;
    let end = start
        .checked_add(length)
        .ok_or_else(|| TokenParseError::Invalid("token length overflow".to_owned()))?;
    if end > buf.len() {
        return Err(NotEnoughDataError::new(end).into());
    }
    Ok(ParseResult::new(buf[start..end].to_vec(), end))
}

fn ucs2(data: &[u8]) -> String {
    String::from_utf16_lossy(
        &data
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>(),
    )
}

fn login_ack(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 2)?;
    if payload.value.len() < 6 {
        return Err(TokenParseError::Invalid("truncated LOGINACK".to_owned()));
    }
    let interface = match payload.value[0] {
        0 => Some("SQL_DFLT".to_owned()),
        1 => Some("SQL_TSQL".to_owned()),
        _ => None,
    };
    let version = u32::from_be_bytes([
        payload.value[1],
        payload.value[2],
        payload.value[3],
        payload.value[4],
    ]);
    let tds_version = crate::tds_versions::versions_by_value()
        .get(&version)
        .map(|value| (*value).to_owned());
    let name_len = payload.value[5] as usize * 2;
    if payload.value.len() < 6 + name_len + 4 {
        return Err(TokenParseError::Invalid(
            "truncated LOGINACK program name".to_owned(),
        ));
    }
    let prog_name = ucs2(&payload.value[6..6 + name_len]);
    let version_start = 6 + name_len;
    Ok(ParseResult::new(
        Token::LoginAck(LoginAckToken {
            interface,
            tds_version,
            prog_name,
            prog_version: [
                payload.value[version_start],
                payload.value[version_start + 1],
                payload.value[version_start + 2],
                payload.value[version_start + 3],
            ],
        }),
        payload.offset,
    ))
}

fn sspi(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 2)?;
    let challenge =
        parse_challenge(&payload.value).map_err(|error| TokenParseError::Invalid(error.0))?;
    Ok(ParseResult::new(
        Token::Sspi(SspiToken {
            ntlm_packet: challenge,
            buffer: payload.value,
        }),
        payload.offset,
    ))
}

fn return_value(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
    use_utc: bool,
    lower_case_guids: bool,
) -> Result<ParseResult<Token>, TokenParseError> {
    let ordinal = read_u16_le(buf, offset)?;
    let name = read_b_var_char(buf, ordinal.offset)?;
    let status = read_u8(buf, name.offset)?;
    let metadata = read_metadata(buf, status.offset, tds_version).map_err(|error| match error {
        crate::metadata_parser::MetadataError::NotEnoughData(value) => {
            TokenParseError::NotEnoughData(value)
        }
        crate::metadata_parser::MetadataError::Invalid(value) => TokenParseError::Invalid(value.0),
    })?;
    let value = crate::value_parser::read_value_with_options(
        buf,
        metadata.offset,
        &metadata.value,
        use_utc,
    )
    .map_err(|error| match error {
        crate::value_parser::ValueParserError::NotEnoughData(value) => {
            TokenParseError::NotEnoughData(value)
        }
        crate::value_parser::ValueParserError::Unsupported(value) => {
            TokenParseError::Invalid(value)
        }
    })?;
    let name = name
        .value
        .strip_prefix('@')
        .unwrap_or(&name.value)
        .to_owned();
    let value_offset = value.offset;
    let value = if lower_case_guids {
        normalize_guid_value(value.value)
    } else {
        value.value
    };
    Ok(ParseResult::new(
        Token::ReturnValue(Box::new(ReturnValueToken {
            ordinal: ordinal.value,
            name,
            status: status.value,
            metadata: metadata.value,
            value,
        })),
        value_offset,
    ))
}

fn normalize_guid_value(value: Value) -> Value {
    match value {
        Value::Guid(value) => Value::Guid(value.to_ascii_lowercase()),
        Value::DateTime { date, time } => Value::DateTime {
            date: Box::new(normalize_guid_value(*date)),
            time: Box::new(normalize_guid_value(*time)),
        },
        Value::LocalDateTime { date, time } => Value::LocalDateTime {
            date: Box::new(normalize_guid_value(*date)),
            time: Box::new(normalize_guid_value(*time)),
        },
        Value::DateTimeOffset {
            date,
            time,
            offset_minutes,
        } => Value::DateTimeOffset {
            date: Box::new(normalize_guid_value(*date)),
            time: Box::new(normalize_guid_value(*time)),
            offset_minutes,
        },
        value => value,
    }
}

fn feature_ack(buf: &[u8], mut offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let mut fed_auth = None;
    let mut utf8_support = None;
    loop {
        let id = read_u8(buf, offset)?;
        offset = id.offset;
        if id.value == 0xFF {
            return Ok(ParseResult::new(
                Token::FeatureExtAck(FeatureExtAckToken {
                    fed_auth,
                    utf8_support,
                }),
                offset,
            ));
        }
        let data = length_prefixed(buf, offset, 4)?;
        offset = data.offset;
        match id.value {
            0x02 => fed_auth = Some(data.value),
            0x0A => utf8_support = Some(data.value.first().is_some_and(|value| *value != 0)),
            _ => {}
        }
    }
}

fn fed_auth_info(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 4)?;
    if payload.value.len() < 4 {
        return Err(TokenParseError::Invalid("truncated FEDAUTHINFO".to_owned()));
    }
    let count = u32::from_le_bytes([
        payload.value[0],
        payload.value[1],
        payload.value[2],
        payload.value[3],
    ]) as usize;
    let mut cursor = 4;
    let mut spn = None;
    let mut sts_url = None;
    for _ in 0..count {
        if payload.value.len() < cursor + 9 {
            return Err(TokenParseError::Invalid(
                "truncated FEDAUTHINFO entry".to_owned(),
            ));
        }
        let id = payload.value[cursor];
        let length = u32::from_le_bytes([
            payload.value[cursor + 1],
            payload.value[cursor + 2],
            payload.value[cursor + 3],
            payload.value[cursor + 4],
        ]) as usize;
        let data_offset = u32::from_le_bytes([
            payload.value[cursor + 5],
            payload.value[cursor + 6],
            payload.value[cursor + 7],
            payload.value[cursor + 8],
        ]) as usize;
        let end = data_offset
            .checked_add(length)
            .ok_or_else(|| TokenParseError::Invalid("FEDAUTHINFO range overflow".to_owned()))?;
        if end > payload.value.len() {
            return Err(TokenParseError::Invalid(
                "truncated FEDAUTHINFO data".to_owned(),
            ));
        }
        match id {
            1 => sts_url = Some(ucs2(&payload.value[data_offset..end])),
            2 => spn = Some(ucs2(&payload.value[data_offset..end])),
            _ => {}
        }
        cursor += 9;
    }
    Ok(ParseResult::new(
        Token::FedAuthInfo(FedAuthInfoToken { spn, sts_url }),
        payload.offset,
    ))
}

fn order(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 2)?;
    if payload.value.len() % 2 != 0 {
        return Err(TokenParseError::Invalid(
            "ORDER length is not even".to_owned(),
        ));
    }
    let values = payload
        .value
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    Ok(ParseResult::new(Token::Order(values), payload.offset))
}

fn offset_token(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let identifier = read_u16_le(buf, offset)?;
    let value = read_u16_le(buf, identifier.offset)?;
    Ok(ParseResult::new(
        Token::Offset(OffsetToken {
            identifier: identifier.value,
            offset: value.value,
        }),
        value.offset,
    ))
}

fn table_name_for_metadata(
    buf: &[u8],
    offset: usize,
    metadata: &Metadata,
    tds_version: &str,
) -> Result<ParseResult<Option<TableName>>, TokenParseError> {
    if !metadata.data_type.has_table_name {
        return Ok(ParseResult::new(None, offset));
    }
    if tds_version < "7_2" {
        let value = read_us_var_char(buf, offset)?;
        return Ok(ParseResult::new(
            Some(TableName::Legacy(value.value)),
            value.offset,
        ));
    }
    let count = read_u8(buf, offset)?;
    let mut cursor = count.offset;
    let mut parts = Vec::with_capacity(count.value as usize);
    for _ in 0..count.value {
        let value = read_us_var_char(buf, cursor)?;
        parts.push(value.value);
        cursor = value.offset;
    }
    Ok(ParseResult::new(Some(TableName::Parts(parts)), cursor))
}

fn alt_metadata(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
) -> Result<ParseResult<Token>, TokenParseError> {
    let count = read_u16_le(buf, offset)?;
    let id = read_u16_le(buf, count.offset)?;
    let by_columns = read_u8(buf, id.offset)?;
    let mut cursor = by_columns.offset;
    let mut grouping_columns = Vec::with_capacity(by_columns.value as usize);
    for _ in 0..by_columns.value {
        let column = read_u16_le(buf, cursor)?;
        grouping_columns.push(column.value);
        cursor = column.offset;
    }
    let mut columns = Vec::with_capacity(count.value as usize);
    for _ in 0..count.value {
        let operator = read_u8(buf, cursor)?;
        let operand = read_u16_le(buf, operator.offset)?;
        let metadata = crate::metadata_parser::read_metadata(buf, operand.offset, tds_version)
            .map_err(|error| match error {
                crate::metadata_parser::MetadataError::NotEnoughData(value) => {
                    TokenParseError::NotEnoughData(value)
                }
                crate::metadata_parser::MetadataError::Invalid(value) => {
                    TokenParseError::Invalid(value.0)
                }
            })?;
        let table_name =
            table_name_for_metadata(buf, metadata.offset, &metadata.value, tds_version)?;
        let column_name = read_b_var_char(buf, table_name.offset)?;
        columns.push(AltMetadataColumn {
            operator: operator.value,
            operand: operand.value,
            metadata: metadata.value,
            column_name: column_name.value,
            table_name: table_name.value,
        });
        cursor = column_name.offset;
    }
    Ok(ParseResult::new(
        Token::AltMetadata(AltMetadataToken {
            id: id.value,
            by_columns: grouping_columns,
            columns,
        }),
        cursor,
    ))
}

fn col_info(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 2)?;
    let mut cursor = 0;
    let mut columns = Vec::new();
    while cursor < payload.value.len() {
        if payload.value.len() < cursor + 3 {
            return Err(TokenParseError::Invalid(
                "truncated COLINFO entry".to_owned(),
            ));
        }
        let column_number = payload.value[cursor];
        let table_number = payload.value[cursor + 1];
        let status = payload.value[cursor + 2];
        cursor += 3;
        let column_name = if status & 0x20 != 0 {
            let value = read_b_var_char(&payload.value, cursor)?;
            cursor = value.offset;
            Some(value.value)
        } else {
            None
        };
        columns.push(ColumnInfo {
            column_number,
            table_number,
            status,
            column_name,
        });
    }
    Ok(ParseResult::new(
        Token::ColInfo(ColInfoToken { columns }),
        payload.offset,
    ))
}

fn tab_name(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 2)?;
    let mut cursor = 0;
    let mut tables = Vec::new();
    while cursor < payload.value.len() {
        if tds_version < "7_2" {
            let table = read_us_var_char(&payload.value, cursor)?;
            tables.push(TableName::Legacy(table.value));
            cursor = table.offset;
        } else {
            let count = read_u8(&payload.value, cursor)?;
            cursor = count.offset;
            let mut parts = Vec::with_capacity(count.value as usize);
            for _ in 0..count.value {
                let part = read_us_var_char(&payload.value, cursor)?;
                parts.push(part.value);
                cursor = part.offset;
            }
            tables.push(TableName::Parts(parts));
        }
    }
    Ok(ParseResult::new(
        Token::TabName(TabNameToken { tables }),
        payload.offset,
    ))
}

fn session_state(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 4)?;
    if payload.value.len() < 5 {
        return Err(TokenParseError::Invalid(
            "truncated SESSIONSTATE token".to_owned(),
        ));
    }
    let sequence_number = u32::from_le_bytes(payload.value[0..4].try_into().unwrap());
    let recoverable = payload.value[4] & 1 != 0;
    let mut cursor = 5;
    let mut entries = Vec::new();
    while cursor < payload.value.len() {
        let state_id = payload.value[cursor];
        cursor += 1;
        let length = *payload
            .value
            .get(cursor)
            .ok_or_else(|| TokenParseError::Invalid("truncated SESSIONSTATE length".to_owned()))?;
        cursor += 1;
        let length = if length == 0xFF {
            let end = cursor.checked_add(4).ok_or_else(|| {
                TokenParseError::Invalid("SESSIONSTATE length overflow".to_owned())
            })?;
            if end > payload.value.len() {
                return Err(TokenParseError::Invalid(
                    "truncated SESSIONSTATE length".to_owned(),
                ));
            }
            let value = u32::from_le_bytes(payload.value[cursor..end].try_into().unwrap());
            cursor = end;
            usize::try_from(value)
                .map_err(|_| TokenParseError::Invalid("SESSIONSTATE length overflow".to_owned()))?
        } else {
            usize::from(length)
        };
        let end = cursor
            .checked_add(length)
            .ok_or_else(|| TokenParseError::Invalid("SESSIONSTATE length overflow".to_owned()))?;
        if end > payload.value.len() {
            return Err(TokenParseError::Invalid(
                "truncated SESSIONSTATE value".to_owned(),
            ));
        }
        entries.push(SessionStateEntry {
            state_id,
            value: payload.value[cursor..end].to_vec(),
        });
        cursor = end;
    }
    Ok(ParseResult::new(
        Token::SessionState(SessionStateToken {
            sequence_number,
            recoverable,
            entries,
        }),
        payload.offset,
    ))
}

fn classification_layout(
    buf: &[u8],
    offset: usize,
    expected_columns: Option<usize>,
    rank_present: bool,
) -> Result<ParseResult<ClassificationLayout>, TokenParseError> {
    let mut cursor = offset;
    let rank = if rank_present {
        let value = read_i32_le(buf, cursor)?;
        cursor = value.offset;
        Some(value.value)
    } else {
        None
    };
    let count = read_u16_le(buf, cursor)?;
    cursor = count.offset;
    if expected_columns.is_some_and(|expected| {
        usize::from(count.value) != 0 && usize::from(count.value) != expected
    }) {
        return Err(TokenParseError::Invalid(
            "DATACLASSIFICATION column count mismatch".to_owned(),
        ));
    }
    let mut columns = Vec::with_capacity(count.value as usize);
    for _ in 0..count.value {
        let property_count = read_u16_le(buf, cursor)?;
        cursor = property_count.offset;
        let mut properties = Vec::with_capacity(property_count.value as usize);
        for _ in 0..property_count.value {
            let label = read_u16_le(buf, cursor)?;
            let information_type = read_u16_le(buf, label.offset)?;
            cursor = information_type.offset;
            let property_rank = if rank_present {
                let value = read_i32_le(buf, cursor)?;
                cursor = value.offset;
                Some(value.value)
            } else {
                None
            };
            properties.push(SensitivityProperty {
                label_index: label.value,
                information_type_index: information_type.value,
                rank: property_rank,
            });
        }
        columns.push(properties);
    }
    Ok(ParseResult::new(
        ClassificationLayout { rank, columns },
        cursor,
    ))
}

struct ClassificationLayout {
    rank: Option<i32>,
    columns: Vec<Vec<SensitivityProperty>>,
}

fn data_classification(
    buf: &[u8],
    offset: usize,
    expected_columns: Option<usize>,
) -> Result<ParseResult<Token>, TokenParseError> {
    let label_count = read_u16_le(buf, offset)?;
    let mut cursor = label_count.offset;
    let mut labels = Vec::with_capacity(label_count.value as usize);
    for _ in 0..label_count.value {
        let name = read_b_var_char(buf, cursor)?;
        let id = read_b_var_char(buf, name.offset)?;
        cursor = id.offset;
        labels.push(SensitivityLabel {
            name: name.value,
            id: id.value,
        });
    }
    let information_count = read_u16_le(buf, cursor)?;
    cursor = information_count.offset;
    let mut information_types = Vec::with_capacity(information_count.value as usize);
    for _ in 0..information_count.value {
        let name = read_b_var_char(buf, cursor)?;
        let id = read_b_var_char(buf, name.offset)?;
        cursor = id.offset;
        information_types.push(InformationType {
            name: name.value,
            id: id.value,
        });
    }
    let no_rank = classification_layout(buf, cursor, expected_columns, false);
    let with_rank = classification_layout(buf, cursor, expected_columns, true);
    let (rank, columns, end) = match (no_rank, with_rank) {
        (Ok(no_rank), Ok(with_rank)) => {
            let no_rank_next_is_token = buf.get(no_rank.offset).is_some_and(is_known_token_marker);
            if no_rank_next_is_token {
                (no_rank.value.rank, no_rank.value.columns, no_rank.offset)
            } else {
                (
                    with_rank.value.rank,
                    with_rank.value.columns,
                    with_rank.offset,
                )
            }
        }
        (Ok(value), Err(_)) | (Err(_), Ok(value)) => {
            (value.value.rank, value.value.columns, value.offset)
        }
        (Err(error), Err(_)) => return Err(error),
    };
    Ok(ParseResult::new(
        Token::DataClassification(DataClassificationToken {
            labels,
            information_types,
            rank,
            columns,
        }),
        end,
    ))
}

pub fn parse_data_classification_with_options(
    buf: &[u8],
    offset: usize,
    expected_columns: Option<usize>,
) -> Result<ParseResult<Token>, TokenParseError> {
    let token = read_u8(buf, offset)?;
    if token.value != 0xA3 {
        return Err(TokenParseError::Invalid(
            "expected DATACLASSIFICATION token".to_owned(),
        ));
    }
    data_classification(buf, token.offset, expected_columns)
}

fn is_known_token_marker(value: &u8) -> bool {
    matches!(
        *value,
        0x01 | 0x78
            | 0x79
            | 0x81
            | 0x88
            | 0xA3
            | 0xA4
            | 0xA5
            | 0xA9
            | 0xAA
            | 0xAB
            | 0xAC
            | 0xAD
            | 0xAE
            | 0xD1
            | 0xD2
            | 0xD3
            | 0xE3
            | 0xE4
            | 0xED
            | 0xEE
            | 0xFD..=0xFF
    )
}

fn read_alt_value(
    buf: &[u8],
    offset: usize,
    metadata: &crate::metadata_parser::Metadata,
    use_utc: bool,
) -> Result<ParseResult<Value>, TokenParseError> {
    let fixed = matches!(
        metadata.data_type.id,
        0x30 | 0x32 | 0x34 | 0x38 | 0x3A | 0x3B | 0x3C | 0x3D | 0x3E | 0x7A | 0x7F
    );
    if !fixed {
        return crate::value_parser::read_value_with_options(buf, offset, metadata, use_utc)
            .map_err(|error| match error {
                crate::value_parser::ValueParserError::NotEnoughData(value) => {
                    TokenParseError::NotEnoughData(value)
                }
                crate::value_parser::ValueParserError::Unsupported(value) => {
                    TokenParseError::Invalid(value)
                }
            });
    }
    let length = read_u8(buf, offset)?;
    if length.value == 0 {
        return Ok(ParseResult::new(Value::Null, length.offset));
    }
    let value = crate::value_parser::read_value_with_options(buf, length.offset, metadata, use_utc)
        .map_err(|error| match error {
            crate::value_parser::ValueParserError::NotEnoughData(value) => {
                TokenParseError::NotEnoughData(value)
            }
            crate::value_parser::ValueParserError::Unsupported(value) => {
                TokenParseError::Invalid(value)
            }
        })?;
    let expected_end = length.offset + usize::from(length.value);
    if value.offset != expected_end {
        return Err(TokenParseError::Invalid(
            "ALTROW value length does not match metadata".to_owned(),
        ));
    }
    Ok(ParseResult::new(value.value, expected_end))
}

pub fn parse_alt_row_with_options(
    buf: &[u8],
    offset: usize,
    columns: &[AltMetadataColumn],
    use_utc: bool,
) -> Result<ParseResult<Token>, TokenParseError> {
    let id = read_u16_le(buf, offset)?;
    let mut cursor = id.offset;
    let mut row_columns = Vec::with_capacity(columns.len());
    for column in columns {
        let value = read_alt_value(buf, cursor, &column.metadata, use_utc)?;
        cursor = value.offset;
        row_columns.push(RowColumn {
            value: value.value,
            metadata: ColumnMetadata {
                metadata: column.metadata.clone(),
                col_name: column.column_name.clone(),
                table_name: column.table_name.clone(),
                crypto_metadata: None,
            },
        });
    }
    Ok(ParseResult::new(
        Token::AltRow(AltRowToken {
            id: id.value,
            row: Row {
                columns: row_columns,
            },
        }),
        cursor,
    ))
}

fn env_change(buf: &[u8], offset: usize) -> Result<ParseResult<Token>, TokenParseError> {
    let payload = length_prefixed(buf, offset, 2)?;
    if payload.value.is_empty() {
        return Err(TokenParseError::Invalid("empty ENVCHANGE".to_owned()));
    }
    let change_type = payload.value[0];
    let data = &payload.value[1..];
    let kind = match change_type {
        1 | 2 | 3 | 13 => {
            let new_value = read_b_var_char(data, 0)?;
            let old_value = read_b_var_char(data, new_value.offset)?;
            EnvChangeKind::Text {
                new_value: new_value.value,
                old_value: old_value.value,
            }
        }
        4 => {
            let new_value = read_b_var_char(data, 0)?;
            let old_value = read_b_var_char(data, new_value.offset)?;
            EnvChangeKind::PacketSize {
                new_value: new_value
                    .value
                    .parse()
                    .map_err(|_| TokenParseError::Invalid("invalid packet size".to_owned()))?,
                old_value: old_value
                    .value
                    .parse()
                    .map_err(|_| TokenParseError::Invalid("invalid packet size".to_owned()))?,
            }
        }
        7 => {
            let new_value = read_b_var_byte(data, 0)?;
            let old_value = read_b_var_byte(data, new_value.offset)?;
            EnvChangeKind::Collation {
                new_value: (!new_value.value.is_empty())
                    .then(|| crate::collation::Collation::from_buffer(&new_value.value, 0))
                    .flatten(),
                old_value: (!old_value.value.is_empty())
                    .then(|| crate::collation::Collation::from_buffer(&old_value.value, 0))
                    .flatten(),
            }
        }
        8 => {
            let new_value = read_b_var_byte(data, 0)?;
            let old_value = read_b_var_byte(data, new_value.offset)?;
            EnvChangeKind::BeginTransaction {
                new_value: new_value.value,
                old_value: old_value.value,
            }
        }
        9 => {
            let new_value = read_b_var_byte(data, 0)?;
            let old_value = read_b_var_byte(data, new_value.offset)?;
            EnvChangeKind::CommitTransaction {
                new_value: new_value.value,
                old_value: old_value.value,
            }
        }
        10 => {
            let new_value = read_b_var_byte(data, 0)?;
            let old_value = read_b_var_byte(data, new_value.offset)?;
            EnvChangeKind::RollbackTransaction {
                new_value: new_value.value,
                old_value: old_value.value,
            }
        }
        18 => {
            let new_value = read_b_var_byte(data, 0)?;
            let old_value = read_b_var_byte(data, new_value.offset)?;
            EnvChangeKind::ResetConnection {
                new_value: new_value.value,
                old_value: old_value.value,
            }
        }
        20 => {
            let route = read_us_var_byte(data, 0)?;
            let old_value = read_us_var_byte(data, route.offset)?;
            if route.value.len() < 5 {
                return Err(TokenParseError::Invalid(
                    "truncated routing change".to_owned(),
                ));
            }
            let protocol = route.value[0];
            if protocol != 0 {
                return Err(TokenParseError::Invalid(
                    "unknown routing protocol".to_owned(),
                ));
            }
            let port = u16::from_le_bytes([route.value[1], route.value[2]]);
            let length = u16::from_le_bytes([route.value[3], route.value[4]]) as usize;
            if route.value.len() < 5 + length * 2 {
                return Err(TokenParseError::Invalid(
                    "truncated routing server".to_owned(),
                ));
            }
            EnvChangeKind::Routing(RoutingChange {
                protocol,
                port,
                server: ucs2(&route.value[5..5 + length * 2]),
                old_value: old_value.value,
            })
        }
        _ => EnvChangeKind::Unknown {
            new_value: data.to_vec(),
            old_value: Vec::new(),
        },
    };
    Ok(ParseResult::new(
        Token::EnvChange(EnvChangeToken { change_type, kind }),
        payload.offset,
    ))
}

pub fn parse_token(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
) -> Result<ParseResult<Token>, TokenParseError> {
    parse_token_with_options(buf, offset, tds_version, true, false)
}

pub fn parse_token_with_options(
    buf: &[u8],
    offset: usize,
    tds_version: &str,
    use_utc: bool,
    lower_case_guids: bool,
) -> Result<ParseResult<Token>, TokenParseError> {
    let token = read_u8(buf, offset)?;
    match token.value {
        0xFD..=0xFF => done(buf, token.offset, token.value, tds_version),
        0xAB | 0xAA => {
            let value = message(buf, token.offset, token.value, tds_version)?;
            if token.value == 0xAA {
                Ok(ParseResult::new(
                    match value.value {
                        Token::Info(mut item) => {
                            item.token = token.value;
                            Token::Error(item)
                        }
                        item => item,
                    },
                    value.offset,
                ))
            } else {
                Ok(value)
            }
        }
        0x79 => {
            let value = read_i32_le(buf, token.offset)?;
            Ok(ParseResult::new(
                Token::ReturnStatus(ReturnStatusToken { value: value.value }),
                value.offset,
            ))
        }
        0xAC => return_value(buf, token.offset, tds_version, use_utc, lower_case_guids),
        0xAD => login_ack(buf, token.offset),
        0xED => sspi(buf, token.offset),
        0xAE => feature_ack(buf, token.offset),
        0xEE => fed_auth_info(buf, token.offset),
        0xA9 => order(buf, token.offset),
        0x78 => offset_token(buf, token.offset),
        0xA4 => tab_name(buf, token.offset, tds_version),
        0xA5 => col_info(buf, token.offset),
        0xA3 => data_classification(buf, token.offset, None),
        0x88 => alt_metadata(buf, token.offset, tds_version),
        0xE4 => session_state(buf, token.offset),
        0xD3 => Err(TokenParseError::Invalid(
            "ALTROW requires preceding ALTMETADATA".to_owned(),
        )),
        0xE3 => env_change(buf, token.offset),
        other => Err(TokenParseError::Unsupported(other)),
    }
}
