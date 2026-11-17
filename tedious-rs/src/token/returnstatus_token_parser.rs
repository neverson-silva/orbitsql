use super::parser::{ReturnStatusToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(data: &[u8]) -> Result<ParseResult<ReturnStatusToken>, TokenParseError> {
    match parse_token(data, 0, "7_4")? {
        ParseResult {
            value: Token::ReturnStatus(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid(
            "expected RETURNSTATUS token".to_owned(),
        )),
    }
}
