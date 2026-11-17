pub fn is_transient_error(number: u32) -> bool {
    matches!(number, 4060 | 10928 | 10929 | 40197 | 40501 | 40613)
}
