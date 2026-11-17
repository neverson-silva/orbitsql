use crate::tracking_buffer::WritableTrackingBuffer;

pub const TYPE_TRANSACTION_DESCRIPTOR: u16 = 0x0002;
pub const TXNDESCRIPTOR_HEADER_DATA_LEN: usize = 12;
pub const TXNDESCRIPTOR_HEADER_LEN: usize = 18;

pub fn write_to_tracking_buffer<'a>(
    buffer: &'a mut WritableTrackingBuffer,
    txn_descriptor: &[u8],
    outstanding_request_count: u32,
) -> &'a mut WritableTrackingBuffer {
    buffer.write_u32_le(0);
    buffer.write_u32_le(TXNDESCRIPTOR_HEADER_LEN as u32);
    buffer.write_u16_le(TYPE_TRANSACTION_DESCRIPTOR);
    buffer.write_buffer(txn_descriptor);
    buffer.write_u32_le(outstanding_request_count);
    buffer.patch_u32_le(0, buffer.length() as u32);
    buffer
}
