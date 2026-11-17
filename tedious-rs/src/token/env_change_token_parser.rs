use super::parser::{EnvChangeKind, EnvChangeToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(data: &[u8]) -> Result<ParseResult<EnvChangeToken>, TokenParseError> {
    match parse_token(data, 0, "7_4")? {
        ParseResult {
            value: Token::EnvChange(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid(
            "expected ENVCHANGE token".to_owned(),
        )),
    }
}

pub fn parse_optional(data: &[u8]) -> Result<ParseResult<Option<EnvChangeToken>>, TokenParseError> {
    let parsed = parse_token(data, 0, "7_4")?;
    match parsed.value {
        Token::EnvChange(token) if matches!(token.kind, EnvChangeKind::Unknown { .. }) => {
            Ok(ParseResult::new(None, parsed.offset))
        }
        Token::EnvChange(token) => Ok(ParseResult::new(Some(token), parsed.offset)),
        _ => Err(TokenParseError::Invalid(
            "expected ENVCHANGE token".to_owned(),
        )),
    }
}
