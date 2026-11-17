use crate::always_encrypted::result_decryption::decrypt_result_column_with_provider;
use crate::always_encrypted::symmetric_key_cache::KeyStoreProvider;
use crate::always_encrypted::types::{CryptoMetadata, CryptoMetadataWithBaseTypeInfo};
use crate::metadata_parser::MetadataError;
use crate::token::parser::{
    Token, TokenParseError, parse_alt_row_with_options, parse_token_with_options,
};
use crate::token::rows::{
    ColumnMetadata, Row, RowParseError, parse_colmetadata_at, parse_nbc_row_with_options,
    parse_row_with_options,
};
use crate::value_parser::ValueParserError;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

pub type ColumnNameReplacer = Arc<dyn Fn(&str, usize, &ColumnMetadata) -> String + Send + Sync>;

#[allow(clippy::large_enum_variant)]
#[derive(Clone, PartialEq)]
pub enum StreamToken {
    Control(Token),
    ColMetadata(Vec<ColumnMetadata>),
    Row(Row),
    NbcRow(Row),
}

impl fmt::Debug for StreamToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Control(token) => formatter
                .debug_tuple("Control")
                .field(&format_args!("[REDACTED CONTROL: {}]", token_name(token)))
                .finish(),
            Self::ColMetadata(columns) => formatter
                .debug_tuple("ColMetadata")
                .field(&format_args!("[REDACTED; {} columns]", columns.len()))
                .finish(),
            Self::Row(row) => formatter
                .debug_tuple("Row")
                .field(&format_args!("[REDACTED; {} columns]", row.columns.len()))
                .finish(),
            Self::NbcRow(row) => formatter
                .debug_tuple("NbcRow")
                .field(&format_args!("[REDACTED; {} columns]", row.columns.len()))
                .finish(),
        }
    }
}

fn token_name(token: &Token) -> &'static str {
    match token {
        Token::Done(_) => "Done",
        Token::Info(_) => "Info",
        Token::Error(_) => "Error",
        Token::ReturnStatus(_) => "ReturnStatus",
        Token::ReturnValue(_) => "ReturnValue",
        Token::LoginAck(_) => "LoginAck",
        Token::Sspi(_) => "Sspi",
        Token::FeatureExtAck(_) => "FeatureExtAck",
        Token::FedAuthInfo(_) => "FedAuthInfo",
        Token::Order(_) => "Order",
        Token::EnvChange(_) => "EnvChange",
        Token::Offset(_) => "Offset",
        Token::TabName(_) => "TabName",
        Token::ColInfo(_) => "ColInfo",
        Token::DataClassification(_) => "DataClassification",
        Token::AltMetadata(_) => "AltMetadata",
        Token::AltRow(_) => "AltRow",
        Token::SessionState(_) => "SessionState",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenStreamState {
    Reading,
    Paused,
    Failed,
    Finished,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TokenStreamEvent {
    Paused,
    Resumed,
    Token(StreamToken),
    End,
    Failed(StreamParseError),
}
#[derive(Clone, Debug, PartialEq)]
pub enum StreamParseError {
    NotEnoughData,
    Token(TokenParseError),
    Row(RowParseError),
    Invalid(String),
}

pub struct TokenStreamParser {
    buffer: Vec<u8>,
    tds_version: String,
    camel_case_columns: bool,
    use_utc: bool,
    lower_case_guids: bool,
    columns: Vec<ColumnMetadata>,
    column_name_replacer: Option<ColumnNameReplacer>,
    paused: bool,
    pending_crypto_metadata: Option<Vec<CryptoMetadata>>,
    result_crypto_metadata: Vec<Option<CryptoMetadataWithBaseTypeInfo>>,
    return_crypto_metadata: HashMap<usize, CryptoMetadata>,
    alt_metadata: HashMap<u16, Vec<crate::token::parser::AltMetadataColumn>>,
    key_store_provider: Option<Arc<dyn KeyStoreProvider>>,
    trusted_server_name_ae: Option<String>,
    key_cache_ttl: Duration,
    state: TokenStreamState,
    events: Vec<TokenStreamEvent>,
    terminal_error: Option<StreamParseError>,
}

impl TokenStreamParser {
    pub fn new(tds_version: impl Into<String>, camel_case_columns: bool) -> Self {
        Self {
            buffer: Vec::new(),
            tds_version: tds_version.into(),
            camel_case_columns,
            use_utc: true,
            lower_case_guids: false,
            columns: Vec::new(),
            column_name_replacer: None,
            paused: false,
            pending_crypto_metadata: None,
            result_crypto_metadata: Vec::new(),
            return_crypto_metadata: HashMap::new(),
            alt_metadata: HashMap::new(),
            key_store_provider: None,
            trusted_server_name_ae: None,
            key_cache_ttl: Duration::ZERO,
            state: TokenStreamState::Reading,
            events: Vec::new(),
            terminal_error: None,
        }
    }

    pub fn new_with_options(
        tds_version: impl Into<String>,
        camel_case_columns: bool,
        use_utc: bool,
    ) -> Self {
        let mut parser = Self::new(tds_version, camel_case_columns);
        parser.use_utc = use_utc;
        parser
    }
    pub fn new_with_parser_options(
        tds_version: impl Into<String>,
        camel_case_columns: bool,
        use_utc: bool,
        lower_case_guids: bool,
    ) -> Self {
        let mut parser = Self::new_with_options(tds_version, camel_case_columns, use_utc);
        parser.lower_case_guids = lower_case_guids;
        parser
    }
    pub fn set_column_name_replacer(&mut self, replacer: Option<ColumnNameReplacer>) {
        self.column_name_replacer = replacer;
    }
    pub fn set_column_encryption(
        &mut self,
        metadata: Vec<CryptoMetadata>,
        provider: Option<Arc<dyn KeyStoreProvider>>,
        server_name: Option<String>,
        cache_ttl: Duration,
    ) {
        self.return_crypto_metadata = metadata
            .iter()
            .map(|value| {
                (
                    value.ordinal,
                    clone_crypto_metadata_without_algorithm(value),
                )
            })
            .collect();
        self.pending_crypto_metadata = Some(metadata);
        self.key_store_provider = provider;
        self.trusted_server_name_ae = server_name;
        self.key_cache_ttl = cache_ttl;
    }
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }
    pub fn pause(&mut self) {
        if self.is_terminal() {
            return;
        }
        self.paused = true;
        self.state = TokenStreamState::Paused;
        self.events.push(TokenStreamEvent::Paused);
    }
    pub fn resume(&mut self) -> Result<Vec<StreamToken>, StreamParseError> {
        if let Some(error) = self.terminal_error() {
            return Err(error);
        }
        self.paused = false;
        self.state = TokenStreamState::Reading;
        self.events.push(TokenStreamEvent::Resumed);
        self.parse_buffer()
    }
    pub fn is_paused(&self) -> bool {
        self.paused
    }
    pub fn finish(&mut self) -> Result<(), StreamParseError> {
        if self.state == TokenStreamState::Finished {
            return Err(StreamParseError::Invalid(
                "token stream is already finished".to_owned(),
            ));
        }
        if let Some(error) = self.terminal_error() {
            return Err(error);
        }
        if self.buffer.is_empty() {
            self.paused = false;
            self.state = TokenStreamState::Finished;
            self.events.push(TokenStreamEvent::End);
            Ok(())
        } else {
            let error = StreamParseError::NotEnoughData;
            self.fail(error.clone());
            Err(error)
        }
    }
    pub fn columns(&self) -> &[ColumnMetadata] {
        &self.columns
    }
    pub fn state(&self) -> &TokenStreamState {
        &self.state
    }
    pub fn events(&self) -> &[TokenStreamEvent] {
        &self.events
    }
    pub fn take_events(&mut self) -> Vec<TokenStreamEvent> {
        std::mem::take(&mut self.events)
    }
    pub fn feed(&mut self, data: &[u8]) -> Result<Vec<StreamToken>, StreamParseError> {
        if let Some(error) = self.terminal_error() {
            return Err(error);
        }
        self.buffer.extend_from_slice(data);
        if self.paused {
            return Ok(Vec::new());
        }
        self.parse_buffer()
    }
    pub fn feed_next(&mut self, data: &[u8]) -> Result<Option<StreamToken>, StreamParseError> {
        if let Some(error) = self.terminal_error() {
            return Err(error);
        }
        self.buffer.extend_from_slice(data);
        if self.paused {
            return Ok(None);
        }
        match self.parse_buffer_up_to(1) {
            Ok(tokens) => Ok(tokens.into_iter().next()),
            Err(error) if is_not_enough_data(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }
    fn parse_buffer(&mut self) -> Result<Vec<StreamToken>, StreamParseError> {
        self.parse_buffer_up_to(usize::MAX)
    }
    fn parse_buffer_up_to(&mut self, limit: usize) -> Result<Vec<StreamToken>, StreamParseError> {
        let result = self.parse_buffer_inner(limit);
        if let Err(error) = &result
            && !is_not_enough_data(error)
        {
            self.fail(error.clone());
        }
        result
    }

    fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            TokenStreamState::Failed | TokenStreamState::Finished
        )
    }

    fn terminal_error(&self) -> Option<StreamParseError> {
        self.terminal_error.clone().or_else(|| {
            if self.state == TokenStreamState::Finished {
                Some(StreamParseError::Invalid(
                    "token stream is already finished".to_owned(),
                ))
            } else {
                None
            }
        })
    }

    fn fail(&mut self, error: StreamParseError) {
        self.state = TokenStreamState::Failed;
        self.paused = false;
        self.terminal_error = Some(error.clone());
        self.events.push(TokenStreamEvent::Failed(error.clone()));
    }
    fn parse_buffer_inner(&mut self, limit: usize) -> Result<Vec<StreamToken>, StreamParseError> {
        let mut output = Vec::new();
        loop {
            if self.buffer.is_empty() {
                break;
            }
            let parsed = match self.buffer[0] {
                0x81 => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    let value = parse_colmetadata_at(
                        &self.buffer,
                        1,
                        &self.tds_version,
                        self.camel_case_columns && self.column_name_replacer.is_none(),
                    )
                    .map_err(|error| map_row_error(error, &self.buffer))?;
                    let mut columns = value.value;
                    if let Some(replacer) = &self.column_name_replacer {
                        let original = columns.clone();
                        for (index, column) in columns.iter_mut().enumerate() {
                            column.col_name = replacer(&column.col_name, index, &original[index]);
                        }
                    }
                    self.result_crypto_metadata = columns
                        .iter()
                        .map(|column| {
                            column.crypto_metadata.as_ref().and_then(|descriptor| {
                                let metadata = self.pending_crypto_metadata.as_ref()?.iter().find(
                                    |metadata| {
                                        metadata.ordinal == descriptor.ordinal
                                            && metadata.cipher_algorithm_id
                                                == descriptor.cipher_algorithm_id
                                            && metadata.encryption_type
                                                == descriptor.encryption_type
                                    },
                                )?;
                                Some(CryptoMetadataWithBaseTypeInfo::new(
                                    clone_crypto_metadata_without_algorithm(metadata),
                                    column.metadata.clone(),
                                ))
                            })
                        })
                        .collect();
                    if columns
                        .iter()
                        .zip(&self.result_crypto_metadata)
                        .any(|(column, metadata)| {
                            column.crypto_metadata.is_some() && metadata.is_none()
                        })
                    {
                        return Err(StreamParseError::Invalid(
                            "column encryption metadata is unavailable".to_owned(),
                        ));
                    }
                    self.pending_crypto_metadata = None;
                    self.columns = columns.clone();
                    self.alt_metadata.clear();
                    Ok((StreamToken::ColMetadata(columns), value.offset))
                })(),
                0xD1 => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    if self.columns.is_empty() {
                        Err(StreamParseError::Invalid(
                            "ROW received before COLMETADATA".to_owned(),
                        ))
                    } else {
                        let value =
                            parse_row_with_options(&self.buffer, 1, &self.columns, self.use_utc)
                                .map_err(|error| map_row_error(error, &self.buffer))?;
                        let mut row = value.value;
                        self.decrypt_row(&mut row)?;
                        if self.lower_case_guids {
                            normalize_row_guids(&mut row);
                        }
                        Ok((StreamToken::Row(row), value.offset))
                    }
                })(),
                0xD2 => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    if self.columns.is_empty() {
                        Err(StreamParseError::Invalid(
                            "NBCROW received before COLMETADATA".to_owned(),
                        ))
                    } else {
                        let value = parse_nbc_row_with_options(
                            &self.buffer,
                            1,
                            &self.columns,
                            self.use_utc,
                        )
                        .map_err(|error| map_row_error(error, &self.buffer))?;
                        let mut row = value.value;
                        self.decrypt_row(&mut row)?;
                        if self.lower_case_guids {
                            normalize_row_guids(&mut row);
                        }
                        Ok((StreamToken::NbcRow(row), value.offset))
                    }
                })(),
                0x88 => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    let value = parse_token_with_options(
                        &self.buffer,
                        0,
                        &self.tds_version,
                        self.use_utc,
                        self.lower_case_guids,
                    )
                    .map_err(StreamParseError::Token)?;
                    if let Token::AltMetadata(metadata) = &value.value {
                        self.alt_metadata
                            .insert(metadata.id, metadata.columns.clone());
                    }
                    Ok((StreamToken::Control(value.value), value.offset))
                })(),
                0xD3 => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    let id = if self.buffer.len() >= 3 {
                        u16::from_le_bytes([self.buffer[1], self.buffer[2]])
                    } else {
                        return Err(StreamParseError::NotEnoughData);
                    };
                    let columns = self.alt_metadata.get(&id).ok_or_else(|| {
                        StreamParseError::Invalid(
                            "ALTROW received without matching ALTMETADATA".to_owned(),
                        )
                    })?;
                    let value = parse_alt_row_with_options(&self.buffer, 1, columns, self.use_utc)
                        .map_err(StreamParseError::Token)?;
                    Ok((StreamToken::Control(value.value), value.offset))
                })(),
                0xA3 => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    let value = crate::token::parser::parse_data_classification_with_options(
                        &self.buffer,
                        0,
                        Some(self.columns.len()),
                    )
                    .map_err(StreamParseError::Token)?;
                    Ok((StreamToken::Control(value.value), value.offset))
                })(),
                _ => (|| -> Result<(StreamToken, usize), StreamParseError> {
                    let value = parse_token_with_options(
                        &self.buffer,
                        0,
                        &self.tds_version,
                        self.use_utc,
                        self.lower_case_guids,
                    )
                    .map_err(StreamParseError::Token)?;
                    let mut token = value.value;
                    self.decrypt_return_value(&mut token)?;
                    Ok((StreamToken::Control(token), value.offset))
                })(),
            };
            let (token, consumed) = match parsed {
                Ok(value) => value,
                Err(error @ StreamParseError::Token(TokenParseError::NotEnoughData(_)))
                | Err(error @ StreamParseError::Row(RowParseError::NotEnoughData(_))) => {
                    return if output.is_empty() {
                        Err(error)
                    } else {
                        Ok(output)
                    };
                }
                Err(error @ StreamParseError::NotEnoughData) => {
                    return if output.is_empty() {
                        Err(error)
                    } else {
                        Ok(output)
                    };
                }
                Err(error) => return Err(error),
            };
            if consumed == 0 || consumed > self.buffer.len() {
                return Err(StreamParseError::Invalid(
                    "token parser made invalid progress".to_owned(),
                ));
            }
            self.buffer.drain(..consumed);
            if let StreamToken::Control(Token::EnvChange(value)) = &token
                && !matches!(
                    value.change_type,
                    1 | 2 | 3 | 4 | 7 | 8 | 9 | 10 | 13 | 18 | 20
                )
            {
                continue;
            }
            self.events.push(TokenStreamEvent::Token(token.clone()));
            output.push(token);
            if output.len() >= limit {
                break;
            }
        }
        Ok(output)
    }
}

impl TokenStreamParser {
    fn decrypt_row(&mut self, row: &mut Row) -> Result<(), StreamParseError> {
        let encrypted = row.columns.iter().any(|column| {
            column.metadata.crypto_metadata.is_some()
                && !matches!(column.value, crate::value_codec::Value::Null)
        });
        let Some(provider) = self.key_store_provider.as_deref() else {
            if encrypted {
                return Err(StreamParseError::Invalid(
                    "column encryption provider is unavailable".to_owned(),
                ));
            }
            return Ok(());
        };
        let Some(server_name) = self.trusted_server_name_ae.as_deref() else {
            return Err(StreamParseError::Invalid(
                "Server name should not be null in decrypt result".to_owned(),
            ));
        };
        for (index, column) in row.columns.iter_mut().enumerate() {
            let Some(metadata) = self
                .result_crypto_metadata
                .get_mut(index)
                .and_then(Option::as_mut)
            else {
                continue;
            };
            column.value = decrypt_result_column_with_provider(
                &column.value,
                metadata,
                server_name,
                provider,
                self.key_cache_ttl,
            )
            .map_err(StreamParseError::Invalid)?;
        }
        Ok(())
    }

    fn decrypt_return_value(&mut self, token: &mut Token) -> Result<(), StreamParseError> {
        let Token::ReturnValue(value) = token else {
            return Ok(());
        };
        if value.metadata.flags & 0x0800 == 0 {
            return Ok(());
        }
        let Some(provider) = self.key_store_provider.as_deref() else {
            return Err(StreamParseError::Invalid(
                "column encryption provider is unavailable".to_owned(),
            ));
        };
        let Some(server_name) = self.trusted_server_name_ae.as_deref() else {
            return Err(StreamParseError::Invalid(
                "Server name should not be null in decrypt result".to_owned(),
            ));
        };
        let Some(metadata) = self.return_crypto_metadata.get(&(value.ordinal as usize)) else {
            return Err(StreamParseError::Invalid(
                "return value encryption metadata is unavailable".to_owned(),
            ));
        };
        let mut metadata = CryptoMetadataWithBaseTypeInfo::new(
            clone_crypto_metadata_without_algorithm(metadata),
            value.metadata.clone(),
        );
        value.value = decrypt_result_column_with_provider(
            &value.value,
            &mut metadata,
            server_name,
            provider,
            self.key_cache_ttl,
        )
        .map_err(StreamParseError::Invalid)?;
        Ok(())
    }
}

fn clone_crypto_metadata_without_algorithm(metadata: &CryptoMetadata) -> CryptoMetadata {
    CryptoMetadata {
        cek_entry: metadata.cek_entry.clone(),
        cipher_algorithm_id: metadata.cipher_algorithm_id,
        cipher_algorithm_name: metadata.cipher_algorithm_name.clone(),
        normalization_rule_version: metadata.normalization_rule_version.clone(),
        encryption_key_info: metadata.encryption_key_info.clone(),
        ordinal: metadata.ordinal,
        encryption_type: metadata.encryption_type,
        cipher_algorithm: None,
    }
}

fn map_row_error(error: RowParseError, _buffer: &[u8]) -> StreamParseError {
    match error {
        RowParseError::NotEnoughData(value) => {
            StreamParseError::Row(RowParseError::NotEnoughData(value))
        }
        RowParseError::Metadata(MetadataError::NotEnoughData(value)) => {
            StreamParseError::Row(RowParseError::NotEnoughData(value))
        }
        RowParseError::Value(ValueParserError::NotEnoughData(value)) => {
            StreamParseError::Row(RowParseError::NotEnoughData(value))
        }
        other => StreamParseError::Row(other),
    }
}

fn is_not_enough_data(error: &StreamParseError) -> bool {
    matches!(
        error,
        StreamParseError::NotEnoughData
            | StreamParseError::Token(TokenParseError::NotEnoughData(_))
            | StreamParseError::Row(RowParseError::NotEnoughData(_))
            | StreamParseError::Row(RowParseError::Metadata(MetadataError::NotEnoughData(_)))
            | StreamParseError::Row(RowParseError::Value(ValueParserError::NotEnoughData(_)))
    )
}

fn normalize_row_guids(row: &mut Row) {
    for column in &mut row.columns {
        normalize_value_guid(&mut column.value);
    }
}

fn normalize_value_guid(value: &mut crate::value_codec::Value) {
    match value {
        crate::value_codec::Value::Guid(value) => *value = value.to_ascii_lowercase(),
        crate::value_codec::Value::DateTime { date, time }
        | crate::value_codec::Value::LocalDateTime { date, time } => {
            normalize_value_guid(date);
            normalize_value_guid(time);
        }
        crate::value_codec::Value::DateTimeOffset { date, time, .. } => {
            normalize_value_guid(date);
            normalize_value_guid(time);
        }
        _ => {}
    }
}
