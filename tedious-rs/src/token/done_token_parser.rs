use super::parser::{DoneToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(data: &[u8], tds_version: &str) -> Result<ParseResult<DoneToken>, TokenParseError> {
    match parse_token(data, 0, tds_version)? {
        ParseResult {
            value: Token::Done(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid("expected DONE token".to_owned())),
    }
}
