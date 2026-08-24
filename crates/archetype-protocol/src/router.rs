use std::{collections::BTreeMap, time::Duration};

use thiserror::Error;

use crate::{Envelope, Message, PROTOCOL_MAJOR, PROTOCOL_MINOR, Request, Response};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingKind {
    Request,
    Cancellation { target_request_id: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingRequest {
    deadline: Duration,
    kind: PendingKind,
}

#[derive(Clone, Debug)]
pub struct RequestRouter {
    maximum_pending: usize,
    next_request_id: Option<u64>,
    pending: BTreeMap<u64, PendingRequest>,
}

impl RequestRouter {
    #[must_use]
    pub fn new(maximum_pending: usize) -> Self {
        Self {
            maximum_pending,
            next_request_id: Some(1),
            pending: BTreeMap::new(),
        }
    }

    /// Registers a request and returns its protocol envelope.
    ///
    /// # Errors
    /// Returns an error when the pending limit, request ID space, or duration range is exhausted.
    pub fn begin(
        &mut self,
        request: Request,
        now: Duration,
        timeout: Duration,
    ) -> Result<Envelope, RouterError> {
        if self.pending.len() >= self.maximum_pending {
            return Err(RouterError::Backpressure {
                maximum: self.maximum_pending,
            });
        }
        let request_id = self.allocate_request_id()?;
        let deadline = now
            .checked_add(timeout)
            .ok_or(RouterError::DeadlineOverflow)?;
        self.pending.insert(
            request_id,
            PendingRequest {
                deadline,
                kind: PendingKind::Request,
            },
        );
        Ok(Envelope::v4(request_id, Message::Request(request)))
    }

    /// Replaces a pending request with a correlated cancellation request.
    ///
    /// # Errors
    /// Returns an error when the target is absent, already expired, or no request ID remains.
    pub fn cancel(
        &mut self,
        target_request_id: u64,
        now: Duration,
        timeout: Duration,
    ) -> Result<Envelope, RouterError> {
        let Some(target) = self.pending.get(&target_request_id).copied() else {
            return Err(RouterError::UnknownRequest(target_request_id));
        };
        if now >= target.deadline {
            self.pending.remove(&target_request_id);
            return Err(RouterError::ResponseTimedOut(target_request_id));
        }
        let cancellation_request_id = self.allocate_request_id()?;
        let deadline = now
            .checked_add(timeout)
            .ok_or(RouterError::DeadlineOverflow)?;
        self.pending.remove(&target_request_id);
        self.pending.insert(
            cancellation_request_id,
            PendingRequest {
                deadline,
                kind: PendingKind::Cancellation { target_request_id },
            },
        );
        Ok(Envelope::v4(
            cancellation_request_id,
            Message::Request(Request::Cancel { target_request_id }),
        ))
    }

    /// Routes one response to its pending request, regardless of arrival order.
    ///
    /// # Errors
    /// Returns an error for non-responses, unknown IDs, expired requests, or mismatched
    /// cancellation acknowledgements.
    pub fn route(
        &mut self,
        envelope: Envelope,
        now: Duration,
    ) -> Result<RoutedResponse, RouterError> {
        if envelope.protocol_major() != PROTOCOL_MAJOR
            || envelope.protocol_minor() != PROTOCOL_MINOR
        {
            return Err(RouterError::UnexpectedProtocolVersion {
                major: envelope.protocol_major(),
                minor: envelope.protocol_minor(),
            });
        }
        let request_id = envelope.request_id();
        let Message::Response(response) = envelope.message() else {
            return Err(RouterError::UnexpectedMessage);
        };
        let Some(pending) = self.pending.remove(&request_id) else {
            return Err(RouterError::UnknownRequest(request_id));
        };
        let original_request_id = match pending.kind {
            PendingKind::Request => request_id,
            PendingKind::Cancellation { target_request_id } => target_request_id,
        };
        if now >= pending.deadline {
            return Err(RouterError::ResponseTimedOut(original_request_id));
        }
        if let PendingKind::Cancellation { target_request_id } = pending.kind {
            if !matches!(response, Response::Cancelled { target_request_id: acknowledged } if *acknowledged == target_request_id)
            {
                return Err(RouterError::InvalidCancellationResponse {
                    expected: target_request_id,
                });
            }
        }
        let Message::Response(response) = envelope.into_message() else {
            unreachable!("message was checked above");
        };
        Ok(RoutedResponse {
            original_request_id,
            response_request_id: request_id,
            response,
        })
    }

    #[must_use]
    pub fn expire(&mut self, now: Duration) -> Vec<u64> {
        let expired: Vec<_> = self
            .pending
            .iter()
            .filter_map(|(request_id, pending)| {
                (now >= pending.deadline).then_some(match pending.kind {
                    PendingKind::Request => *request_id,
                    PendingKind::Cancellation { target_request_id } => target_request_id,
                })
            })
            .collect();
        self.pending.retain(|_, pending| now < pending.deadline);
        expired
    }

    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    fn allocate_request_id(&mut self) -> Result<u64, RouterError> {
        let request_id = self
            .next_request_id
            .ok_or(RouterError::RequestIdExhausted)?;
        self.next_request_id = request_id.checked_add(1);
        Ok(request_id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutedResponse {
    pub original_request_id: u64,
    pub response_request_id: u64,
    pub response: Response,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum RouterError {
    #[error("request router is full at {maximum} pending requests")]
    Backpressure { maximum: usize },
    #[error("request ID space is exhausted")]
    RequestIdExhausted,
    #[error("request deadline exceeds the supported duration range")]
    DeadlineOverflow,
    #[error("request {0} is not pending")]
    UnknownRequest(u64),
    #[error("request {0} timed out")]
    ResponseTimedOut(u64),
    #[error("received a non-response message for a pending request")]
    UnexpectedMessage,
    #[error("received protocol version {major}.{minor} for a V4 request")]
    UnexpectedProtocolVersion { major: u16, minor: u16 },
    #[error("cancellation response did not acknowledge request {expected}")]
    InvalidCancellationResponse { expected: u64 },
}
