use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
};

use thiserror::Error;

use crate::{Codec, Envelope, ProtocolError};

pub struct MemoryEndpoint {
    codec: Codec,
    sender: SyncSender<Vec<u8>>,
    receiver: Receiver<Vec<u8>>,
    outgoing_bytes: Arc<AtomicUsize>,
    incoming_bytes: Arc<AtomicUsize>,
    maximum_queued_bytes: usize,
}

impl MemoryEndpoint {
    /// Encodes and queues one envelope without blocking.
    ///
    /// # Errors
    /// Returns an error when encoding fails, a queue limit is reached, or the peer disconnected.
    pub fn send(&self, envelope: &Envelope) -> Result<(), TransportError> {
        let mut frame = Vec::new();
        self.codec.encode(&mut frame, envelope)?;
        reserve_bytes(&self.outgoing_bytes, frame.len(), self.maximum_queued_bytes)?;
        match self.sender.try_send(frame) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(frame)) => {
                self.outgoing_bytes.fetch_sub(frame.len(), Ordering::AcqRel);
                Err(TransportError::QueueFull)
            }
            Err(TrySendError::Disconnected(frame)) => {
                self.outgoing_bytes.fetch_sub(frame.len(), Ordering::AcqRel);
                Err(TransportError::Disconnected)
            }
        }
    }

    /// Decodes the next queued envelope without blocking.
    ///
    /// # Errors
    /// Returns an error when decoding fails or the peer disconnected.
    pub fn try_receive(&self) -> Result<Option<Envelope>, TransportError> {
        let frame = match self.receiver.try_recv() {
            Ok(frame) => frame,
            Err(TryRecvError::Empty) => return Ok(None),
            Err(TryRecvError::Disconnected) => return Err(TransportError::Disconnected),
        };
        self.incoming_bytes.fetch_sub(frame.len(), Ordering::AcqRel);
        Ok(Some(self.codec.decode(frame.as_slice())?))
    }

    #[must_use]
    pub fn outgoing_queued_bytes(&self) -> usize {
        self.outgoing_bytes.load(Ordering::Acquire)
    }
}

#[must_use]
pub fn memory_transport(
    maximum_queued_frames: usize,
    maximum_queued_bytes: usize,
    codec: Codec,
) -> (MemoryEndpoint, MemoryEndpoint) {
    let (left_sender, right_receiver) = sync_channel(maximum_queued_frames);
    let (right_sender, left_receiver) = sync_channel(maximum_queued_frames);
    let left_to_right_bytes = Arc::new(AtomicUsize::new(0));
    let right_to_left_bytes = Arc::new(AtomicUsize::new(0));
    (
        MemoryEndpoint {
            codec: codec.clone(),
            sender: left_sender,
            receiver: left_receiver,
            outgoing_bytes: Arc::clone(&left_to_right_bytes),
            incoming_bytes: Arc::clone(&right_to_left_bytes),
            maximum_queued_bytes,
        },
        MemoryEndpoint {
            codec,
            sender: right_sender,
            receiver: right_receiver,
            outgoing_bytes: right_to_left_bytes,
            incoming_bytes: left_to_right_bytes,
            maximum_queued_bytes,
        },
    )
}

fn reserve_bytes(
    queued: &AtomicUsize,
    frame_bytes: usize,
    maximum: usize,
) -> Result<(), TransportError> {
    let mut current = queued.load(Ordering::Acquire);
    loop {
        let Some(next) = current.checked_add(frame_bytes) else {
            return Err(TransportError::ByteLimit { maximum });
        };
        if next > maximum {
            return Err(TransportError::ByteLimit { maximum });
        }
        match queued.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return Ok(()),
            Err(actual) => current = actual,
        }
    }
}

#[derive(Debug, Error)]
pub enum TransportError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("memory transport frame queue is full")]
    QueueFull,
    #[error("memory transport byte queue exceeds its {maximum}-byte limit")]
    ByteLimit { maximum: usize },
    #[error("memory transport peer disconnected")]
    Disconnected,
}
