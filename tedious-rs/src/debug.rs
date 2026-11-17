use crate::packet::Packet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DebugOptions {
    pub data: bool,
    pub payload: bool,
    pub packet: bool,
    pub token: bool,
}

pub struct Debug {
    pub options: DebugOptions,
    listeners: Vec<DebugListener>,
}

type DebugListener = Box<dyn Fn(&str) + Send + Sync>;

impl Debug {
    pub fn new(options: DebugOptions) -> Self {
        Self {
            options,
            listeners: Vec::new(),
        }
    }

    pub fn on(&mut self, listener: impl Fn(&str) + Send + Sync + 'static) {
        self.listeners.push(Box::new(listener));
    }

    pub fn have_listeners(&self) -> bool {
        !self.listeners.is_empty()
    }

    pub fn log(&self, text: impl AsRef<str>) {
        for listener in &self.listeners {
            listener(text.as_ref());
        }
    }

    pub fn packet(&self, direction: &str, packet: &Packet) {
        if self.options.packet && self.have_listeners() {
            self.log(format!("\n{direction}\n{}", packet.header_to_string("  ")));
        }
    }

    pub fn data(&self, packet: &Packet) {
        if self.options.data && self.have_listeners() {
            if packet_data_is_sensitive(packet) {
                self.log("  [REDACTED PACKET DATA]");
            } else {
                self.log(packet.data_to_string("  "));
            }
        }
    }

    pub fn payload(&self, generate: impl FnOnce() -> String) {
        if self.options.payload && self.have_listeners() {
            self.log(generate());
        }
    }

    pub fn token(&self, token: impl std::fmt::Debug) {
        if self.options.token && self.have_listeners() {
            self.log(format!("{token:?}"));
        }
    }
}

fn packet_data_is_sensitive(packet: &Packet) -> bool {
    matches!(
        packet.packet_type(),
        crate::packet::packet_type::SQL_BATCH
            | crate::packet::packet_type::RPC_REQUEST
            | crate::packet::packet_type::TABULAR_RESULT
            | crate::packet::packet_type::BULK_LOAD
            | crate::packet::packet_type::LOGIN7
            | crate::packet::packet_type::NTLMAUTH_PKT
            | crate::packet::packet_type::FEDAUTH_TOKEN
    )
}
