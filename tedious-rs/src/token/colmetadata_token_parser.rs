use super::rows::{ColumnMetadata, RowParseError, parse_colmetadata};
use crate::errors::ParseResult;

pub fn parse(
    data: &[u8],
    tds_version: &str,
    camel_case_columns: bool,
) -> Result<ParseResult<Vec<ColumnMetadata>>, RowParseError> {
    parse_colmetadata(data, tds_version, camel_case_columns)
}
