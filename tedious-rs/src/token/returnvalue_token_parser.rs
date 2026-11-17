use super::parser::{ReturnValueToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(
    data: &[u8],
    tds_version: &str,
) -> Result<ParseResult<Box<ReturnValueToken>>, TokenParseError> {
    match parse_token(data, 0, tds_version)? {
        ParseResult {
            value: Token::ReturnValue(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid(
            "expected RETURNVALUE token".to_owned(),
        )),
    }
}
