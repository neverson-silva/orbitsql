use crate::packet::{HEADER_LENGTH, Packet, packet_type};
use std::fmt;

#[derive(Clone, PartialEq, Eq)]
pub struct Message {
    pub message_type: u8,
    pub reset_connection: bool,
    pub ignore: bool,
    pub data: Vec<u8>,
}

impl fmt::Debug for Message {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Message")
            .field("message_type", &self.message_type)
            .field("reset_connection", &self.reset_connection)
            .field("ignore", &self.ignore)
            .field(
                "data",
                &format_args!("[REDACTED; {} bytes]", self.data.len()),
            )
            .finish()
    }
}

impl Message {
    pub fn new(message_type: u8) -> Self {
        Self {
            message_type,
            reset_connection: false,
            ignore: false,
            data: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageStreamError(pub String);

impl fmt::Display for MessageStreamError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for MessageStreamError {}

#[derive(Clone, PartialEq, Eq)]
pub enum MessageStreamEvent {
    MessageStarted(Message),
    Data { message_type: u8, data: Vec<u8> },
    MessageEnded { message_type: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageStreamControl {
    Continue,
    Pending,
}

impl fmt::Debug for MessageStreamEvent {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MessageStarted(message) => formatter
                .debug_tuple("MessageStarted")
                .field(message)
                .finish(),
            Self::Data { message_type, data } => formatter
                .debug_struct("Data")
                .field("message_type", message_type)
                .field("data", &format_args!("[REDACTED; {} bytes]", data.len()))
                .finish(),
            Self::MessageEnded { message_type } => formatter
                .debug_struct("MessageEnded")
                .field("message_type", message_type)
                .finish(),
        }
    }
}

#[derive(Default)]
pub struct IncomingMessageStream {
    buffer: Vec<u8>,
    current: Option<Message>,
    paused: bool,
    terminal_error: Option<MessageStreamError>,
    finished: bool,
    pending_messages: Vec<Message>,
    pending_events: Vec<MessageStreamEvent>,
}

impl IncomingMessageStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<Message>, MessageStreamError> {
        self.ensure_open()?;
        self.buffer.extend_from_slice(chunk);
        Ok(self.process_buffered()?.1)
    }

    pub fn feed_incremental(
        &mut self,
        chunk: &[u8],
    ) -> Result<Vec<MessageStreamEvent>, MessageStreamError> {
        self.ensure_open()?;
        self.buffer.extend_from_slice(chunk);
        Ok(self.process_buffered()?.0)
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) -> Result<Vec<Message>, MessageStreamError> {
        self.ensure_open()?;
        self.paused = false;
        Ok(self.process_buffered()?.1)
    }

    pub fn resume_incremental(&mut self) -> Result<Vec<MessageStreamEvent>, MessageStreamError> {
        self.ensure_open()?;
        self.paused = false;
        Ok(self.process_buffered()?.0)
    }

    pub fn feed_incremental_with_sink<F>(
        &mut self,
        chunk: &[u8],
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.ensure_open()?;
        self.buffer.extend_from_slice(chunk);
        self.process_buffered_with_sink(sink)
    }

    pub fn resume_incremental_with_sink<F>(
        &mut self,
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.ensure_open()?;
        self.paused = false;
        self.process_buffered_with_sink(sink)
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn take_pending_messages(&mut self) -> Vec<Message> {
        std::mem::take(&mut self.pending_messages)
    }

    pub fn take_pending_events(&mut self) -> Vec<MessageStreamEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn process_buffered(
        &mut self,
    ) -> Result<(Vec<MessageStreamEvent>, Vec<Message>), MessageStreamError> {
        self.ensure_open()?;
        if self.paused {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut events = Vec::new();
        let mut messages = Vec::new();
        loop {
            if self.buffer.len() < HEADER_LENGTH {
                break;
            }
            let length = u16::from_be_bytes([self.buffer[2], self.buffer[3]]) as usize;
            if length < HEADER_LENGTH {
                self.preserve_before_error(&events, &messages);
                return self.fail("Unable to process incoming packet");
            }
            if self.buffer.len() < length {
                break;
            }
            let packet_type = self.buffer[0];
            let packet_status = self.buffer[1];
            let packet = self.buffer.drain(..length).collect::<Vec<_>>();
            if self.current.is_none() {
                let message = Message::new(packet_type);
                events.push(MessageStreamEvent::MessageStarted(message.clone()));
                self.current = Some(message);
            }
            let data = packet[HEADER_LENGTH..].to_vec();
            let message_type = self
                .current
                .as_ref()
                .map_or(packet_type, |message| message.message_type);
            if !data.is_empty() {
                events.push(MessageStreamEvent::Data {
                    message_type,
                    data: data.clone(),
                });
            }
            if let Some(message) = &mut self.current {
                message.data.extend_from_slice(&data);
            }
            if packet_status & crate::packet::status::EOM != 0 {
                if let Some(message) = self.current.take() {
                    events.push(MessageStreamEvent::MessageEnded {
                        message_type: message.message_type,
                    });
                    messages.push(message);
                } else {
                    return self.fail("message ended without a payload");
                }
            }
        }
        Ok((events, messages))
    }

    fn process_buffered_with_sink<F>(
        &mut self,
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.ensure_open()?;
        if self.paused {
            return Ok(MessageStreamControl::Pending);
        }
        loop {
            if self.buffer.len() < HEADER_LENGTH {
                return Ok(MessageStreamControl::Continue);
            }
            let length = u16::from_be_bytes([self.buffer[2], self.buffer[3]]) as usize;
            if length < HEADER_LENGTH {
                return self.fail("Unable to process incoming packet");
            }
            if self.buffer.len() < length {
                return Ok(MessageStreamControl::Continue);
            }
            let packet_type = self.buffer[0];
            let packet_status = self.buffer[1];
            if self.current.is_none() {
                let message = Message::new(packet_type);
                self.current = Some(message.clone());
                if sink(MessageStreamEvent::MessageStarted(message))
                    == MessageStreamControl::Pending
                {
                    self.paused = true;
                    return Ok(MessageStreamControl::Pending);
                }
            }
            let packet = self.buffer.drain(..length).collect::<Vec<_>>();
            let data = packet[HEADER_LENGTH..].to_vec();
            let message_type = self
                .current
                .as_ref()
                .map_or(packet_type, |message| message.message_type);
            if !data.is_empty() {
                if let Some(message) = &mut self.current {
                    message.data.extend_from_slice(&data);
                }
                if sink(MessageStreamEvent::Data { message_type, data })
                    == MessageStreamControl::Pending
                {
                    self.paused = true;
                    return Ok(MessageStreamControl::Pending);
                }
            }
            if packet_status & crate::packet::status::EOM != 0 {
                if let Some(message) = self.current.take() {
                    if sink(MessageStreamEvent::MessageEnded {
                        message_type: message.message_type,
                    }) == MessageStreamControl::Pending
                    {
                        self.paused = true;
                        return Ok(MessageStreamControl::Pending);
                    }
                } else {
                    return self.fail("message ended without a payload");
                }
            }
        }
    }

    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn finish(&mut self) -> Result<(), MessageStreamError> {
        self.ensure_open()?;
        if self.buffer.is_empty() && self.current.is_none() {
            self.finished = true;
            Ok(())
        } else {
            self.fail("incoming stream ended before the message was complete")
        }
    }

    fn preserve_before_error(&mut self, events: &[MessageStreamEvent], messages: &[Message]) {
        self.pending_events.extend_from_slice(events);
        self.pending_messages.extend_from_slice(messages);
    }

    fn ensure_open(&self) -> Result<(), MessageStreamError> {
        if let Some(error) = &self.terminal_error {
            return Err(error.clone());
        }
        if self.finished {
            return Err(MessageStreamError(
                "incoming stream has finished".to_owned(),
            ));
        }
        Ok(())
    }

    fn fail<T>(&mut self, message: &str) -> Result<T, MessageStreamError> {
        let error = MessageStreamError(message.to_owned());
        self.terminal_error = Some(error.clone());
        self.buffer.clear();
        self.current = None;
        Err(error)
    }
}

pub fn packetize(message: &Message, packet_size: usize) -> Result<Vec<Packet>, MessageStreamError> {
    if packet_size <= HEADER_LENGTH || packet_size > u16::MAX as usize {
        return Err(MessageStreamError("invalid packet size".to_owned()));
    }
    if message.ignore {
        let mut packet = Packet::new(message.message_type);
        packet.reset_connection(message.reset_connection);
        packet.last(Some(true));
        packet.ignore(true);
        return Ok(vec![packet]);
    }
    let payload_size = packet_size - HEADER_LENGTH;
    let count = message.data.len().div_ceil(payload_size).max(1);
    let mut packets = Vec::with_capacity(count);
    for index in 0..count {
        let start = index * payload_size;
        let end = (start + payload_size).min(message.data.len());
        let mut packet = Packet::new(message.message_type);
        packet.buffer[6] = ((index + 1) % 256) as u8;
        packet.reset_connection(message.reset_connection);
        packet.last(Some(index + 1 == count));
        packet.ignore(message.ignore && index + 1 == count);
        packet
            .add_data(&message.data[start..end])
            .map_err(MessageStreamError)?;
        packets.push(packet);
    }
    Ok(packets)
}

pub fn message_type_name(value: u8) -> &'static str {
    match value {
        packet_type::SQL_BATCH => "SQL_BATCH",
        packet_type::RPC_REQUEST => "RPC_REQUEST",
        packet_type::TABULAR_RESULT => "TABULAR_RESULT",
        packet_type::ATTENTION => "ATTENTION",
        packet_type::BULK_LOAD => "BULK_LOAD",
        packet_type::FEDAUTH_TOKEN => "FEDAUTH_TOKEN",
        packet_type::TRANSACTION_MANAGER => "TRANSACTION_MANAGER",
        packet_type::LOGIN7 => "LOGIN7",
        packet_type::NTLMAUTH_PKT => "NTLMAUTH_PKT",
        packet_type::PRELOGIN => "PRELOGIN",
        _ => "UNKNOWN",
    }
}
