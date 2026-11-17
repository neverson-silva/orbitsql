use crate::message::{IncomingMessageStream as MessageAssembler, Message};

pub use crate::message::{MessageStreamControl, MessageStreamError, MessageStreamEvent};

#[derive(Default)]
pub struct IncomingMessageStream {
    assembler: MessageAssembler,
}

impl IncomingMessageStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, chunk: &[u8]) -> Result<Vec<Message>, MessageStreamError> {
        let mut messages = self.assembler.feed(chunk)?;
        for message in &mut messages {
            message.reset_connection = false;
            message.ignore = false;
        }
        Ok(messages)
    }

    pub fn feed_incremental(
        &mut self,
        chunk: &[u8],
    ) -> Result<Vec<MessageStreamEvent>, MessageStreamError> {
        self.assembler.feed_incremental(chunk)
    }

    pub fn pause(&mut self) -> &mut Self {
        self.assembler.pause();
        self
    }

    pub fn resume(&mut self) -> Result<Vec<Message>, MessageStreamError> {
        let mut messages = self.assembler.resume()?;
        for message in &mut messages {
            message.reset_connection = false;
            message.ignore = false;
        }
        Ok(messages)
    }

    pub fn resume_incremental(&mut self) -> Result<Vec<MessageStreamEvent>, MessageStreamError> {
        self.assembler.resume_incremental()
    }

    pub fn feed_incremental_with_sink<F>(
        &mut self,
        chunk: &[u8],
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.assembler.feed_incremental_with_sink(chunk, sink)
    }

    pub fn process_buffered_data<F>(
        &mut self,
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.assembler.resume_incremental_with_sink(sink)
    }

    pub fn drain_incremental_with_sink<F>(
        &mut self,
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.assembler.resume_incremental_with_sink(sink)
    }

    pub fn resume_incremental_with_sink<F>(
        &mut self,
        sink: &mut F,
    ) -> Result<MessageStreamControl, MessageStreamError>
    where
        F: FnMut(MessageStreamEvent) -> MessageStreamControl,
    {
        self.assembler.resume_incremental_with_sink(sink)
    }

    pub fn is_paused(&self) -> bool {
        self.assembler.is_paused()
    }

    pub fn buffered_len(&self) -> usize {
        self.assembler.buffered_len()
    }

    pub fn take_pending_messages(&mut self) -> Vec<Message> {
        self.assembler.take_pending_messages()
    }

    pub fn take_pending_events(&mut self) -> Vec<MessageStreamEvent> {
        self.assembler.take_pending_events()
    }

    pub fn finish(&mut self) -> Result<(), MessageStreamError> {
        self.assembler.finish()
    }
}
