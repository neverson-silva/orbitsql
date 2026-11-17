use crate::value_codec::EncodedParameter;

pub fn normalize_row(
    legacy_lob: &[bool],
    null_values: &[EncodedParameter],
    mut values: Vec<EncodedParameter>,
) -> Result<Vec<EncodedParameter>, String> {
    if legacy_lob.len() != null_values.len() {
        return Err("bulk row normalization metadata does not match column count".to_owned());
    }
    values.truncate(legacy_lob.len());
    while values.len() < legacy_lob.len() {
        let index = values.len();
        values.push(null_values[index].clone());
    }
    for (is_legacy_lob, value) in legacy_lob.iter().zip(&mut values) {
        if *is_legacy_lob && value.length == u32::MAX.to_le_bytes() {
            value.length.clear();
        }
    }
    Ok(values)
}
