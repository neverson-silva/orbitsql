use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Cp437,
    Cp850,
    Cp874,
    Cp932,
    Cp936,
    Cp949,
    Cp950,
    Cp1250,
    Cp1251,
    Cp1252,
    Cp1253,
    Cp1254,
    Cp1255,
    Cp1256,
    Cp1257,
    Cp1258,
}

impl Display for Encoding {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Cp437 => "CP437",
            Encoding::Cp850 => "CP850",
            Encoding::Cp874 => "CP874",
            Encoding::Cp932 => "CP932",
            Encoding::Cp936 => "CP936",
            Encoding::Cp949 => "CP949",
            Encoding::Cp950 => "CP950",
            Encoding::Cp1250 => "CP1250",
            Encoding::Cp1251 => "CP1251",
            Encoding::Cp1252 => "CP1252",
            Encoding::Cp1253 => "CP1253",
            Encoding::Cp1254 => "CP1254",
            Encoding::Cp1255 => "CP1255",
            Encoding::Cp1256 => "CP1256",
            Encoding::Cp1257 => "CP1257",
            Encoding::Cp1258 => "CP1258",
        })
    }
}

pub const IGNORE_CASE: u32 = 1;
pub const IGNORE_ACCENT: u32 = 2;
pub const IGNORE_KANA: u32 = 4;
pub const IGNORE_WIDTH: u32 = 8;
pub const BINARY: u32 = 16;
pub const BINARY2: u32 = 32;
pub const UTF8: u32 = 64;

fn language_encoding(value: u32) -> Option<Encoding> {
    match value {
        0x0401 => Some(Encoding::Cp1256),
        0x0404 => Some(Encoding::Cp950),
        0x0405 => Some(Encoding::Cp1250),
        0x0406 => Some(Encoding::Cp1252),
        0x0408 => Some(Encoding::Cp1253),
        0x0409 => Some(Encoding::Cp1252),
        0x040A => Some(Encoding::Cp1252),
        0x040B => Some(Encoding::Cp1252),
        0x040C => Some(Encoding::Cp1252),
        0x040D => Some(Encoding::Cp1255),
        0x040E => Some(Encoding::Cp1250),
        0x040F => Some(Encoding::Cp1252),
        0x0411 => Some(Encoding::Cp932),
        0x0412 => Some(Encoding::Cp949),
        0x0414 => Some(Encoding::Cp1252),
        0x0415 => Some(Encoding::Cp1250),
        0x0417 => Some(Encoding::Cp1252),
        0x0418 => Some(Encoding::Cp1250),
        0x0419 => Some(Encoding::Cp1251),
        0x041A => Some(Encoding::Cp1250),
        0x041B => Some(Encoding::Cp1250),
        0x041C => Some(Encoding::Cp1250),
        0x041E => Some(Encoding::Cp874),
        0x041F => Some(Encoding::Cp1254),
        0x0420 => Some(Encoding::Cp1256),
        0x0422 => Some(Encoding::Cp1251),
        0x0424 => Some(Encoding::Cp1250),
        0x0425 => Some(Encoding::Cp1257),
        0x0426 => Some(Encoding::Cp1257),
        0x0427 => Some(Encoding::Cp1257),
        0x0429 => Some(Encoding::Cp1256),
        0x042A => Some(Encoding::Cp1258),
        0x042C => Some(Encoding::Cp1254),
        0x042E => Some(Encoding::Cp1252),
        0x042F => Some(Encoding::Cp1251),
        0x043B => Some(Encoding::Cp1252),
        0x043F => Some(Encoding::Cp1251),
        0x0442 => Some(Encoding::Cp1250),
        0x0443 => Some(Encoding::Cp1254),
        0x0444 => Some(Encoding::Cp1251),
        0x0452 => Some(Encoding::Cp1252),
        0x0462 => Some(Encoding::Cp1252),
        0x046D => Some(Encoding::Cp1251),
        0x047A => Some(Encoding::Cp1252),
        0x047C => Some(Encoding::Cp1252),
        0x047E => Some(Encoding::Cp1252),
        0x0480 => Some(Encoding::Cp1256),
        0x0483 => Some(Encoding::Cp1252),
        0x0485 => Some(Encoding::Cp1251),
        0x048C => Some(Encoding::Cp1256),
        0x0804 => Some(Encoding::Cp936),
        0x081A => Some(Encoding::Cp1250),
        0x082C => Some(Encoding::Cp1251),
        0x083B => Some(Encoding::Cp1252),
        0x085F => Some(Encoding::Cp1252),
        0x0C04 => Some(Encoding::Cp950),
        0x0C0A => Some(Encoding::Cp1252),
        0x0C1A => Some(Encoding::Cp1251),
        0x1404 => Some(Encoding::Cp950),
        0x141A => Some(Encoding::Cp1250),
        0x201A => Some(Encoding::Cp1251),
        0x0407 => Some(Encoding::Cp1252),
        0x0437 => Some(Encoding::Cp1252),
        _ => None,
    }
}

fn sort_encoding(value: u8) -> Option<Encoding> {
    match value {
        30 => Some(Encoding::Cp437),
        31 => Some(Encoding::Cp437),
        32 => Some(Encoding::Cp437),
        33 => Some(Encoding::Cp437),
        34 => Some(Encoding::Cp437),
        40 => Some(Encoding::Cp850),
        41 => Some(Encoding::Cp850),
        42 => Some(Encoding::Cp850),
        43 => Some(Encoding::Cp850),
        44 => Some(Encoding::Cp850),
        49 => Some(Encoding::Cp850),
        51 => Some(Encoding::Cp1252),
        52 => Some(Encoding::Cp1252),
        53 => Some(Encoding::Cp1252),
        54 => Some(Encoding::Cp1252),
        55 => Some(Encoding::Cp850),
        56 => Some(Encoding::Cp850),
        57 => Some(Encoding::Cp850),
        58 => Some(Encoding::Cp850),
        59 => Some(Encoding::Cp850),
        60 => Some(Encoding::Cp850),
        61 => Some(Encoding::Cp850),
        80 => Some(Encoding::Cp1250),
        81 => Some(Encoding::Cp1250),
        82 => Some(Encoding::Cp1250),
        83 => Some(Encoding::Cp1250),
        84 => Some(Encoding::Cp1250),
        85 => Some(Encoding::Cp1250),
        86 => Some(Encoding::Cp1250),
        87 => Some(Encoding::Cp1250),
        88 => Some(Encoding::Cp1250),
        89 => Some(Encoding::Cp1250),
        90 => Some(Encoding::Cp1250),
        91 => Some(Encoding::Cp1250),
        92 => Some(Encoding::Cp1250),
        93 => Some(Encoding::Cp1250),
        94 => Some(Encoding::Cp1250),
        95 => Some(Encoding::Cp1250),
        96 => Some(Encoding::Cp1250),
        104 => Some(Encoding::Cp1251),
        105 => Some(Encoding::Cp1251),
        106 => Some(Encoding::Cp1251),
        107 => Some(Encoding::Cp1251),
        108 => Some(Encoding::Cp1251),
        112 => Some(Encoding::Cp1253),
        113 => Some(Encoding::Cp1253),
        114 => Some(Encoding::Cp1253),
        120 => Some(Encoding::Cp1253),
        121 => Some(Encoding::Cp1253),
        122 => Some(Encoding::Cp1253),
        124 => Some(Encoding::Cp1253),
        128 => Some(Encoding::Cp1254),
        129 => Some(Encoding::Cp1254),
        130 => Some(Encoding::Cp1254),
        136 => Some(Encoding::Cp1255),
        137 => Some(Encoding::Cp1255),
        138 => Some(Encoding::Cp1255),
        144 => Some(Encoding::Cp1256),
        145 => Some(Encoding::Cp1256),
        146 => Some(Encoding::Cp1256),
        152 => Some(Encoding::Cp1257),
        153 => Some(Encoding::Cp1257),
        154 => Some(Encoding::Cp1257),
        155 => Some(Encoding::Cp1257),
        156 => Some(Encoding::Cp1257),
        157 => Some(Encoding::Cp1257),
        158 => Some(Encoding::Cp1257),
        159 => Some(Encoding::Cp1257),
        160 => Some(Encoding::Cp1257),
        183 => Some(Encoding::Cp1252),
        184 => Some(Encoding::Cp1252),
        185 => Some(Encoding::Cp1252),
        186 => Some(Encoding::Cp1252),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collation {
    pub lcid: u32,
    pub flags: u32,
    pub version: u8,
    pub sort_id: u8,
    pub codepage: Option<Encoding>,
    buffer: Option<[u8; 5]>,
}

impl Collation {
    pub fn from_buffer(buffer: &[u8], offset: usize) -> Option<Self> {
        if buffer.len() < offset.saturating_add(5) {
            return None;
        }
        let lcid = u32::from(buffer[offset])
            | (u32::from(buffer[offset + 1]) << 8)
            | (u32::from(buffer[offset + 2] & 0x0f) << 16);
        let flags =
            u32::from(buffer[offset + 2] >> 4) | (u32::from(buffer[offset + 3] & 0x0f) << 4);
        let version = buffer[offset + 3] >> 4;
        let sort_id = buffer[offset + 4];
        Some(Self::new(lcid, flags, version, sort_id))
    }
    pub fn new(lcid: u32, flags: u32, version: u8, sort_id: u8) -> Self {
        let codepage = if flags & UTF8 != 0 {
            Some(Encoding::Utf8)
        } else if sort_id != 0 {
            sort_encoding(sort_id)
        } else {
            language_encoding(lcid & 0xffff)
        };
        Self {
            lcid,
            flags,
            version,
            sort_id,
            codepage,
            buffer: None,
        }
    }
    pub fn to_buffer(&mut self) -> [u8; 5] {
        if let Some(buffer) = self.buffer {
            return buffer;
        }
        let buffer = [
            (self.lcid & 0xff) as u8,
            ((self.lcid >> 8) & 0xff) as u8,
            ((self.lcid >> 16) as u8 & 0x0f) | ((self.flags as u8 & 0x0f) << 4),
            ((self.flags as u8 >> 4) & 0x0f) | ((self.version & 0x0f) << 4),
            self.sort_id,
        ];
        self.buffer = Some(buffer);
        buffer
    }
}
