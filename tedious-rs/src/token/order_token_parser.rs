use super::parser::{Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(data: &[u8]) -> Result<ParseResult<Vec<u16>>, TokenParseError> {
    match parse_token(data, 0, "7_4")? {
        ParseResult {
            value: Token::Order(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid("expected ORDER token".to_owned())),
    }
}
