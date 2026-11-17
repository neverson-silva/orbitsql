use super::rows::{ColumnMetadata, Row, RowParseError, parse_nbc_row};
use crate::errors::ParseResult;

pub fn parse(data: &[u8], columns: &[ColumnMetadata]) -> Result<ParseResult<Row>, RowParseError> {
    parse_nbc_row(data, 0, columns)
}
