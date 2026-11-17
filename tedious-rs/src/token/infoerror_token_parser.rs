use super::parser::{MessageToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse_info(
    data: &[u8],
    tds_version: &str,
) -> Result<ParseResult<MessageToken>, TokenParseError> {
    match parse_token(data, 0, tds_version)? {
        ParseResult {
            value: Token::Info(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid("expected INFO token".to_owned())),
    }
}

pub fn parse_error(
    data: &[u8],
    tds_version: &str,
) -> Result<ParseResult<MessageToken>, TokenParseError> {
    match parse_token(data, 0, tds_version)? {
        ParseResult {
            value: Token::Error(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid("expected ERROR token".to_owned())),
    }
}
