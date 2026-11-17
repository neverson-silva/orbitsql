use std::collections::HashMap;

pub const VERSIONS: [(&str, u32); 6] = [
    ("7_1", 0x71000001),
    ("7_2", 0x72090002),
    ("7_3_A", 0x730A0003),
    ("7_3_B", 0x730B0003),
    ("7_4", 0x74000004),
    ("8_0", 0x08000000),
];

pub const TDS_VERSION: [(&str, u32); 6] = VERSIONS;

pub fn versions() -> HashMap<&'static str, u32> {
    VERSIONS.into_iter().collect()
}

pub fn versions_by_value() -> HashMap<u32, &'static str> {
    VERSIONS
        .into_iter()
        .map(|(name, value)| (value, name))
        .collect()
}

pub fn version_value(name: &str) -> Option<u32> {
    VERSIONS
        .iter()
        .find_map(|(version_name, value)| (*version_name == name).then_some(*value))
}

pub fn version_name(value: u32) -> Option<&'static str> {
    VERSIONS
        .iter()
        .find_map(|(name, version_value)| (*version_value == value).then_some(*name))
}

pub fn is_tds_8_0(name: &str) -> bool {
    name == "8_0"
}

pub fn strict_tls_version_is_valid(name: &str) -> bool {
    is_tds_8_0(name) && version_value(name) == Some(0x08000000)
}
