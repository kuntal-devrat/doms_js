//! Inter-isolate message passing and channels.

use std::sync::mpsc::{Receiver, Sender, channel};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("Message send failed: receiver disconnected")]
    SendError,
    #[error("Message receive failed: channel empty or disconnected")]
    RecvError,
}

#[derive(Debug, Clone)]
pub struct MessageData {
    pub payload: Vec<u8>,
}

pub struct MessagePort {
    sender: Sender<MessageData>,
    receiver: Receiver<MessageData>,
}

impl MessagePort {
    pub fn new(sender: Sender<MessageData>, receiver: Receiver<MessageData>) -> Self {
        Self { sender, receiver }
    }

    pub fn post_message(&self, data: Vec<u8>) -> Result<(), ChannelError> {
        self.sender.send(MessageData { payload: data }).map_err(|_| ChannelError::SendError)
    }

    pub fn try_recv(&self) -> Result<MessageData, ChannelError> {
        self.receiver.try_recv().map_err(|_| ChannelError::RecvError)
    }
}

pub struct MessageChannel {
    pub port1: MessagePort,
    pub port2: MessagePort,
}

impl MessageChannel {
    pub fn new() -> Self {
        let (tx1, rx1) = channel();
        let (tx2, rx2) = channel();

        Self { port1: MessagePort::new(tx2, rx1), port2: MessagePort::new(tx1, rx2) }
    }
}

impl Default for MessageChannel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_channel_roundtrip() {
        let channel = MessageChannel::new();
        channel.port1.post_message(vec![1, 2, 3]).unwrap();
        let msg = channel.port2.try_recv().unwrap();
        assert_eq!(msg.payload, vec![1, 2, 3]);
    }
}
