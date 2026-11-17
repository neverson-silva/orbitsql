use crate::all_headers::write_to_tracking_buffer;
use crate::tracking_buffer::{Encoding, WritableTrackingBuffer};

pub const TM_GET_DTC_ADDRESS: u16 = 0x00;
pub const TM_PROPAGATE_XACT: u16 = 0x01;
pub const TM_BEGIN_XACT: u16 = 0x05;
pub const TM_PROMOTE_XACT: u16 = 0x06;
pub const TM_COMMIT_XACT: u16 = 0x07;
pub const TM_ROLLBACK_XACT: u16 = 0x08;
pub const TM_SAVE_XACT: u16 = 0x09;

pub const OPERATION_TYPE: [(&str, u16); 7] = [
    ("TM_GET_DTC_ADDRESS", TM_GET_DTC_ADDRESS),
    ("TM_PROPAGATE_XACT", TM_PROPAGATE_XACT),
    ("TM_BEGIN_XACT", TM_BEGIN_XACT),
    ("TM_PROMOTE_XACT", TM_PROMOTE_XACT),
    ("TM_COMMIT_XACT", TM_COMMIT_XACT),
    ("TM_ROLLBACK_XACT", TM_ROLLBACK_XACT),
    ("TM_SAVE_XACT", TM_SAVE_XACT),
];

pub const ISOLATION_LEVEL: [(&str, u8); 6] = [
    ("NO_CHANGE", 0),
    ("READ_UNCOMMITTED", 1),
    ("READ_COMMITTED", 2),
    ("REPEATABLE_READ", 3),
    ("SERIALIZABLE", 4),
    ("SNAPSHOT", 5),
];

pub fn isolation_level_by_value(value: u8) -> Option<&'static str> {
    ISOLATION_LEVEL
        .iter()
        .find_map(|(name, number)| (*number == value).then_some(*name))
}

pub fn assert_valid_isolation_level(value: i32, name: &str) -> Result<(), String> {
    if !(0..=5).contains(&value) {
        return Err(format!(
            "The value of \"{name}\" is out of range. It must be >= 0 && <= 5. Received: {value}"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IsolationLevel {
    #[default]
    NoChange = 0,
    ReadUncommitted = 1,
    ReadCommitted = 2,
    RepeatableRead = 3,
    Serializable = 4,
    Snapshot = 5,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionPayloadError(pub String);

pub struct Transaction {
    pub name: String,
    pub isolation_level: IsolationLevel,
    pub outstanding_request_count: u32,
}

impl Default for Transaction {
    fn default() -> Self {
        Self::with_default_isolation_level(String::new())
    }
}

impl Transaction {
    pub fn new(name: impl Into<String>, isolation_level: IsolationLevel) -> Self {
        Self {
            name: name.into(),
            isolation_level,
            outstanding_request_count: 1,
        }
    }

    pub fn with_default_isolation_level(name: impl Into<String>) -> Self {
        Self::new(name, IsolationLevel::default())
    }
    pub fn isolation_level_to_tsql(&self) -> &'static str {
        match self.isolation_level {
            IsolationLevel::ReadUncommitted => "READ UNCOMMITTED",
            IsolationLevel::ReadCommitted => "READ COMMITTED",
            IsolationLevel::RepeatableRead => "REPEATABLE READ",
            IsolationLevel::Serializable => "SERIALIZABLE",
            IsolationLevel::Snapshot => "SNAPSHOT",
            IsolationLevel::NoChange => "",
        }
    }

    pub fn description(&self, operation: u16) -> String {
        match operation {
            TM_BEGIN_XACT => format!(
                "Begin Transaction: name={}, isolationLevel={}",
                self.name,
                isolation_level_by_value(self.isolation_level as u8).unwrap_or("UNKNOWN")
            ),
            TM_COMMIT_XACT => format!("Commit Transaction: name={}", self.name),
            TM_ROLLBACK_XACT => format!("Rollback Transaction: name={}", self.name),
            TM_SAVE_XACT => format!("Save Transaction: name={}", self.name),
            _ => format!("Transaction operation: name={}", self.name),
        }
    }
    pub fn try_begin_payload(&self, descriptor: &[u8]) -> Result<Vec<u8>, TransactionPayloadError> {
        let name_length = self.name.encode_utf16().count();
        if name_length > usize::from(u8::MAX) / 2 {
            return Err(TransactionPayloadError(
                "transaction name exceeds the TDS one-byte length field".to_owned(),
            ));
        }
        let mut b = WritableTrackingBuffer::new(100, Some(Encoding::Ucs2), false);
        write_to_tracking_buffer(&mut b, descriptor, self.outstanding_request_count);
        b.write_u16_le(TM_BEGIN_XACT);
        b.write_u8(self.isolation_level as u8);
        b.write_u8((name_length * 2) as u8);
        let _ = b.write_string(&self.name, Some(Encoding::Ucs2));
        Ok(b.data())
    }
    pub fn try_commit_payload(
        &self,
        descriptor: &[u8],
    ) -> Result<Vec<u8>, TransactionPayloadError> {
        self.try_simple_payload(descriptor, TM_COMMIT_XACT, true)
    }
    pub fn try_rollback_payload(
        &self,
        descriptor: &[u8],
    ) -> Result<Vec<u8>, TransactionPayloadError> {
        self.try_simple_payload(descriptor, TM_ROLLBACK_XACT, true)
    }
    pub fn try_save_payload(&self, descriptor: &[u8]) -> Result<Vec<u8>, TransactionPayloadError> {
        self.try_simple_payload(descriptor, TM_SAVE_XACT, false)
    }

    fn try_simple_payload(
        &self,
        descriptor: &[u8],
        operation: u16,
        flag: bool,
    ) -> Result<Vec<u8>, TransactionPayloadError> {
        let name_length = self.name.encode_utf16().count();
        if name_length > usize::from(u8::MAX) / 2 {
            return Err(TransactionPayloadError(
                "transaction name exceeds the TDS one-byte length field".to_owned(),
            ));
        }
        let mut b = WritableTrackingBuffer::new(100, Some(Encoding::Ascii), false);
        write_to_tracking_buffer(&mut b, descriptor, self.outstanding_request_count);
        b.write_u16_le(operation);
        b.write_u8((name_length * 2) as u8);
        let _ = b.write_string(&self.name, Some(Encoding::Ucs2));
        if flag {
            b.write_u8(0)
        }
        Ok(b.data())
    }
}
