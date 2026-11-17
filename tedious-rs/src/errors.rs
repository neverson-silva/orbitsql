use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::connector::ConnectorError;
use crate::ntlm::NtlmError;
use crate::token::handler::UnexpectedTokenError;
use crate::transport::TransportError;
use crate::value_codec::CodecError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCause {
    pub kind: String,
    pub message: String,
    pub code: Option<String>,
    pub cause: Option<Box<ErrorCause>>,
}

impl ErrorCause {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            kind: "Error".to_owned(),
            message: message.into(),
            code: None,
            cause: None,
        }
    }

    pub fn typed(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            code: None,
            cause: None,
        }
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_cause(mut self, cause: ErrorCause) -> Self {
        self.cause = Some(Box::new(cause));
        self
    }

    pub fn from_error(error: &(dyn Error + 'static)) -> Self {
        let mut cause = if let Some(error) = error.downcast_ref::<ConnectionError>() {
            Self::typed("ConnectionError", error.message.clone())
                .with_optional_code(error.code.clone())
        } else if let Some(error) = error.downcast_ref::<RequestError>() {
            Self::typed("RequestError", error.message.clone())
                .with_optional_code(error.code.clone())
        } else if let Some(error) = error.downcast_ref::<InputError>() {
            Self::typed("TypeError", error.0.clone())
        } else if let Some(error) = error.downcast_ref::<UnexpectedTokenError>() {
            Self::typed("UnexpectedTokenError", error.to_string())
        } else if let Some(error) = error.downcast_ref::<NotEnoughDataError>() {
            Self::typed("NotEnoughDataError", error.to_string())
        } else if let Some(error) = error.downcast_ref::<CodecError>() {
            Self::typed("CodecError", error.0.clone())
        } else if let Some(error) = error.downcast_ref::<ConnectorError>() {
            Self::typed("ConnectorError", error.to_string())
        } else if let Some(error) = error.downcast_ref::<TransportError>() {
            Self::typed("TransportError", error.to_string())
        } else if let Some(error) = error.downcast_ref::<NtlmError>() {
            Self::typed("NtlmError", error.0.clone())
        } else if let Some(error) = error.downcast_ref::<std::io::Error>() {
            Self::typed("IoError", error.to_string())
        } else {
            Self::new(error.to_string())
        };
        if let Some(source) = error.source() {
            cause.cause = Some(Box::new(Self::from_error(source)));
        }
        cause
    }

    fn with_optional_code(mut self, code: Option<String>) -> Self {
        self.code = code;
        self
    }
}

impl Display for ErrorCause {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ErrorCause {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionError {
    pub message: String,
    pub code: Option<String>,
    pub is_transient: Option<bool>,
    pub isTransient: Option<bool>,
    pub cause: Option<Box<ErrorCause>>,
}

impl ConnectionError {
    pub fn new(message: impl Into<String>, code: Option<String>) -> Self {
        Self {
            message: message.into(),
            code,
            is_transient: None,
            isTransient: None,
            cause: None,
        }
    }

    pub fn with_cause(
        message: impl Into<String>,
        code: Option<String>,
        cause: impl Error + 'static,
    ) -> Self {
        let mut error = Self::new(message, code);
        error.cause = Some(Box::new(ErrorCause::from_error(&cause)));
        error
    }

    pub fn with_cause_value(
        message: impl Into<String>,
        code: Option<String>,
        cause: ErrorCause,
    ) -> Self {
        let mut error = Self::new(message, code);
        error.cause = Some(Box::new(cause));
        error
    }

    pub fn cause(&self) -> Option<&ErrorCause> {
        self.cause.as_deref()
    }
}

impl Display for ConnectionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ConnectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestError {
    pub message: String,
    pub code: Option<String>,
    pub number: Option<i32>,
    pub state: Option<u8>,
    pub class: Option<u8>,
    pub server_name: Option<String>,
    pub proc_name: Option<String>,
    pub line_number: Option<i32>,
    pub serverName: Option<String>,
    pub procName: Option<String>,
    pub lineNumber: Option<i32>,
    pub is_transient: Option<bool>,
    pub cause: Option<Box<ErrorCause>>,
    pub errors: Vec<RequestError>,
}

impl RequestError {
    pub fn new(message: impl Into<String>, code: Option<String>) -> Self {
        Self {
            message: message.into(),
            code,
            number: None,
            state: None,
            class: None,
            server_name: None,
            proc_name: None,
            line_number: None,
            serverName: None,
            procName: None,
            lineNumber: None,
            is_transient: None,
            cause: None,
            errors: Vec::new(),
        }
    }

    pub fn aggregate(errors: Vec<Self>) -> Self {
        Self {
            message: String::new(),
            code: None,
            number: None,
            state: None,
            class: None,
            server_name: None,
            proc_name: None,
            line_number: None,
            serverName: None,
            procName: None,
            lineNumber: None,
            is_transient: None,
            cause: None,
            errors,
        }
    }

    pub fn with_cause(
        message: impl Into<String>,
        code: Option<String>,
        cause: impl Error + 'static,
    ) -> Self {
        let mut error = Self::new(message, code);
        error.cause = Some(Box::new(ErrorCause::from_error(&cause)));
        error
    }

    pub fn with_cause_value(
        message: impl Into<String>,
        code: Option<String>,
        cause: ErrorCause,
    ) -> Self {
        let mut error = Self::new(message, code);
        error.cause = Some(Box::new(cause));
        error
    }

    pub fn cause(&self) -> Option<&ErrorCause> {
        self.cause.as_deref()
    }
}

impl Display for RequestError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for RequestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|cause| cause as &(dyn Error + 'static))
    }
}

impl Display for ConnectorError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resolve(error) | Self::SocketOptions(error) => Display::fmt(error, f),
            Self::NoAddresses => f.write_str("no addresses"),
            Self::AllFailed(errors) => f.write_str(&errors.join(", ")),
            Self::Cancelled => f.write_str("operation cancelled"),
        }
    }
}

impl Error for ConnectorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resolve(error) | Self::SocketOptions(error) => Some(error),
            Self::NoAddresses | Self::AllFailed(_) | Self::Cancelled => None,
        }
    }
}

impl Display for TransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => Display::fmt(error, f),
            Self::Timeout => f.write_str("transport timeout"),
            Self::InvalidPacket(message) => f.write_str(message),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Timeout | Self::InvalidPacket(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError(pub String);

impl Display for InputError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for InputError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseResult<T> {
    pub value: T,
    pub offset: usize,
}

impl<T> ParseResult<T> {
    pub fn new(value: T, offset: usize) -> Self {
        Self { value, offset }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotEnoughDataError {
    pub byte_count: usize,
}

impl NotEnoughDataError {
    pub fn new(byte_count: usize) -> Self {
        Self { byte_count }
    }
}

impl Display for NotEnoughDataError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "need {} bytes", self.byte_count)
    }
}

impl Error for NotEnoughDataError {}
