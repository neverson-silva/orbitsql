use super::parser::{FeatureExtAckToken, Token, TokenParseError, parse_token};
use crate::errors::ParseResult;

pub fn parse(data: &[u8]) -> Result<ParseResult<FeatureExtAckToken>, TokenParseError> {
    match parse_token(data, 0, "7_4")? {
        ParseResult {
            value: Token::FeatureExtAck(token),
            offset,
        } => Ok(ParseResult::new(token, offset)),
        _ => Err(TokenParseError::Invalid(
            "expected FEATUREEXTACK token".to_owned(),
        )),
    }
}
