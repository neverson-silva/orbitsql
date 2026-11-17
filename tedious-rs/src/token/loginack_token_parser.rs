use super::parser::{LoginAckToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(data: &[u8]) -> Result<ParseResult<LoginAckToken>, TokenParseError> {
    match parse_token(data, 0, "7_4")? {
        ParseResult {
            value: Token::LoginAck(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid(
            "expected LOGINACK token".to_owned(),
        )),
    }
}
