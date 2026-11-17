use std::collections::VecDeque;

use crate::message::{Message, MessageStreamControl, MessageStreamError, packetize};
use crate::packet::Packet;

pub struct OutgoingMessageStream {
    packet_size: usize,
    packets: VecDeque<Packet>,
    paused: bool,
    finished: bool,
    terminal_error: Option<MessageStreamError>,
}

impl OutgoingMessageStream {
    pub fn new(packet_size: usize) -> Result<Self, MessageStreamError> {
        packetize(&Message::new(0), packet_size)?;
        Ok(Self {
            packet_size,
            packets: VecDeque::new(),
            paused: false,
            finished: false,
            terminal_error: None,
        })
    }

    pub fn write_message(&mut self, message: &Message) -> Result<(), MessageStreamError> {
        self.ensure_open()?;
        let packets = match packetize(message, self.packet_size) {
            Ok(packets) => packets,
            Err(error) => return self.fail(error),
        };
        self.packets.extend(packets);
        Ok(())
    }

    pub fn write_message_parts<I>(
        &mut self,
        message_type: u8,
        reset_connection: bool,
        parts: I,
    ) -> Result<(), MessageStreamError>
    where
        I: IntoIterator<Item = Result<Vec<u8>, MessageStreamError>>,
    {
        self.ensure_open()?;
        let payload_size = self.packet_size - crate::packet::HEADER_LENGTH;
        let mut payload = Vec::with_capacity(payload_size);
        let mut packet_id = 1u8;
        let mut emitted = false;
        for part in parts {
            let part = match part {
                Ok(part) => part,
                Err(error) => return self.fail(error),
            };
            let mut offset = 0;
            while offset < part.len() {
                if payload.len() == payload_size {
                    let packet = match stream_packet(
                        message_type,
                        packet_id,
                        false,
                        reset_connection,
                        &payload,
                    ) {
                        Ok(packet) => packet,
                        Err(error) => return self.fail(error),
                    };
                    self.packets.push_back(packet);
                    packet_id = packet_id.wrapping_add(1);
                    payload.clear();
                    emitted = true;
                }
                let length = (payload_size - payload.len()).min(part.len() - offset);
                payload.extend_from_slice(&part[offset..offset + length]);
                offset += length;
            }
        }
        if !payload.is_empty() || !emitted {
            let packet =
                match stream_packet(message_type, packet_id, true, reset_connection, &payload) {
                    Ok(packet) => packet,
                    Err(error) => return self.fail(error),
                };
            self.packets.push_back(packet);
        }
        Ok(())
    }

    pub fn packet_size(&self) -> usize {
        self.packet_size
    }

    pub fn set_packet_size(&mut self, packet_size: usize) -> Result<(), MessageStreamError> {
        self.ensure_open()?;
        packetize(&Message::new(0), packet_size)?;
        self.packet_size = packet_size;
        Ok(())
    }

    pub fn read_packet(&mut self) -> Option<Packet> {
        if self.paused {
            None
        } else {
            self.packets.pop_front()
        }
    }

    pub fn read_packet_with_sink<F>(
        &mut self,
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(Packet) -> MessageStreamControl,
    {
        if self.paused {
            return Ok(MessageStreamControl::Pending);
        }

        let Some(packet) = self.packets.pop_front() else {
            return Ok(MessageStreamControl::Continue);
        };

        if sink(packet) == MessageStreamControl::Pending {
            self.paused = true;
            Ok(MessageStreamControl::Pending)
        } else {
            Ok(MessageStreamControl::Continue)
        }
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn resume(&mut self) {
        self.paused = false;
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn pending_packets(&self) -> usize {
        self.packets.len()
    }

    pub fn finish(&mut self) {
        self.finished = true;
    }
}

fn stream_packet(
    message_type: u8,
    packet_id: u8,
    last: bool,
    reset_connection: bool,
    data: &[u8],
) -> Result<Packet, MessageStreamError> {
    let mut packet = Packet::new(message_type);
    packet.buffer[6] = packet_id;
    packet.last(Some(last));
    packet.reset_connection(reset_connection);
    packet.add_data(data).map_err(MessageStreamError)?;
    Ok(packet)
}

impl OutgoingMessageStream {
    fn ensure_open(&self) -> Result<(), MessageStreamError> {
        if let Some(error) = &self.terminal_error {
            return Err(error.clone());
        }
        if self.finished {
            return Err(MessageStreamError("stream has finished".to_owned()));
        }
        Ok(())
    }

    fn fail<T>(&mut self, error: MessageStreamError) -> Result<T, MessageStreamError> {
        self.packets.clear();
        self.terminal_error = Some(error.clone());
        Err(error)
    }
}
