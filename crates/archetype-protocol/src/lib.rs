use std::{
    collections::BTreeSet,
    io::{Read, Write},
};

use arch_paint::DisplayList;
use archetype_types::{ArchetypeUrl, NavigationId, PageId};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, value::RawValue};
use thiserror::Error;

pub const MAGIC: &str = "ARCH";
pub const PROTOCOL_MAJOR: u16 = 4;
pub const PROTOCOL_MINOR: u16 = 0;
pub const MAX_FRAME_BODY_BYTES: usize = 16 * 1024 * 1024;
const MAX_FRAME_BODY_BYTES_U32: u32 = 16 * 1024 * 1024;

mod router;
mod transport;

pub use router::{RequestRouter, RoutedResponse, RouterError};
pub use transport::{MemoryEndpoint, TransportError, memory_transport};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Capability(String);

impl Capability {
    #[must_use]
    pub fn static_document() -> Self {
        Self("static_document".to_owned())
    }

    #[must_use]
    pub fn display_list_v1() -> Self {
        Self("display_list_v1".to_owned())
    }

    #[must_use]
    pub fn cancellable_navigation() -> Self {
        Self("cancellable_navigation".to_owned())
    }

    #[must_use]
    pub fn resource_broker_v1() -> Self {
        Self("resource_broker_v1".to_owned())
    }

    #[must_use]
    pub fn renderer_restart_v1() -> Self {
        Self("renderer_restart_v1".to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProtocolLimits {
    pub max_frame_body_bytes: u32,
    pub max_in_flight_requests: u16,
    pub max_event_queue: u16,
    pub max_in_flight_bytes: u32,
}

impl Default for ProtocolLimits {
    fn default() -> Self {
        Self {
            max_frame_body_bytes: MAX_FRAME_BODY_BYTES_U32,
            max_in_flight_requests: 64,
            max_event_queue: 256,
            max_in_flight_bytes: 64 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientHello {
    pub minimum_protocol_minor: u16,
    pub maximum_protocol_minor: u16,
    pub capabilities: BTreeSet<Capability>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ServerHello {
    pub selected_protocol_minor: u16,
    pub capabilities: BTreeSet<Capability>,
    pub limits: ProtocolLimits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionCode {
    IncompatibleMajor,
    IncompatibleMinor,
    InvalidHello,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Rejected {
    pub code: RejectionCode,
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Stylesheet,
    Image,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceBytes(Vec<u8>);

impl ResourceBytes {
    #[must_use]
    pub const fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<u8> {
        self.0
    }
}

impl Serialize for ResourceBytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for ResourceBytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        STANDARD
            .decode(encoded)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BrokeredResource {
    pub requested_url: ArchetypeUrl,
    pub final_url: ArchetypeUrl,
    pub kind: ResourceKind,
    pub body: ResourceBytes,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Navigate {
        page_id: PageId,
        navigation_id: NavigationId,
        url: ArchetypeUrl,
    },
    RenderDocument {
        page_id: PageId,
        navigation_id: NavigationId,
        url: ArchetypeUrl,
        html: String,
        viewport_width_px: u32,
        resources: Vec<BrokeredResource>,
    },
    Cancel {
        target_request_id: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Accepted,
    Rendered {
        page_id: PageId,
        navigation_id: NavigationId,
        final_url: ArchetypeUrl,
        title: String,
        display_list: DisplayList,
        diagnostics: Vec<String>,
    },
    Cancelled {
        target_request_id: u64,
    },
    Failed {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    ClientHello(ClientHello),
    ServerHello(ServerHello),
    Rejected(Rejected),
    Request(Request),
    Response(Response),
}

impl Message {
    const fn kind(&self) -> &'static str {
        match self {
            Self::ClientHello(_) => "client_hello",
            Self::ServerHello(_) => "server_hello",
            Self::Rejected(_) => "rejected",
            Self::Request(_) => "request",
            Self::Response(_) => "response",
        }
    }

    fn payload(&self) -> Result<Value, ProtocolError> {
        match self {
            Self::ClientHello(payload) => Ok(serde_json::to_value(payload)?),
            Self::ServerHello(payload) => Ok(serde_json::to_value(payload)?),
            Self::Rejected(payload) => Ok(serde_json::to_value(payload)?),
            Self::Request(payload) => Ok(serde_json::to_value(payload)?),
            Self::Response(payload) => Ok(serde_json::to_value(payload)?),
        }
    }

    fn from_wire(kind: &str, payload: Value) -> Result<Self, ProtocolError> {
        match kind {
            "client_hello" => Ok(Self::ClientHello(serde_json::from_value(payload)?)),
            "server_hello" => Ok(Self::ServerHello(serde_json::from_value(payload)?)),
            "rejected" => Ok(Self::Rejected(serde_json::from_value(payload)?)),
            "request" => Ok(Self::Request(serde_json::from_value(payload)?)),
            "response" => Ok(Self::Response(serde_json::from_value(payload)?)),
            _ => Err(ProtocolError::UnknownMessageKind(kind.to_owned())),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Envelope {
    protocol_major: u16,
    protocol_minor: u16,
    request_id: u64,
    message: Message,
}

impl Envelope {
    #[must_use]
    pub const fn new(
        protocol_major: u16,
        protocol_minor: u16,
        request_id: u64,
        message: Message,
    ) -> Self {
        Self {
            protocol_major,
            protocol_minor,
            request_id,
            message,
        }
    }

    #[must_use]
    pub const fn v4(request_id: u64, message: Message) -> Self {
        Self::new(PROTOCOL_MAJOR, PROTOCOL_MINOR, request_id, message)
    }

    #[must_use]
    pub const fn protocol_major(&self) -> u16 {
        self.protocol_major
    }

    #[must_use]
    pub const fn protocol_minor(&self) -> u16 {
        self.protocol_minor
    }

    #[must_use]
    pub const fn request_id(&self) -> u64 {
        self.request_id
    }

    #[must_use]
    pub const fn message(&self) -> &Message {
        &self.message
    }

    #[must_use]
    pub fn into_message(self) -> Message {
        self.message
    }
}

#[derive(Clone, Debug)]
pub struct Codec {
    max_frame_body_bytes: usize,
}

impl Codec {
    #[must_use]
    pub const fn new(max_frame_body_bytes: usize) -> Self {
        Self {
            max_frame_body_bytes,
        }
    }

    /// Writes one length-prefixed envelope.
    ///
    /// # Errors
    /// Returns an error when serialization or writing fails, or when the body exceeds the limit.
    pub fn encode(&self, mut writer: impl Write, envelope: &Envelope) -> Result<(), ProtocolError> {
        let body = encode_body(envelope)?;
        if body.len() > self.effective_limit() {
            return Err(ProtocolError::FrameTooLarge {
                actual: body.len(),
                maximum: self.effective_limit(),
            });
        }
        let length = u32::try_from(body.len()).map_err(|_| ProtocolError::FrameTooLarge {
            actual: body.len(),
            maximum: self.effective_limit(),
        })?;
        writer.write_all(&length.to_be_bytes())?;
        writer.write_all(&body)?;
        Ok(())
    }

    /// Reads and validates one length-prefixed envelope.
    ///
    /// # Errors
    /// Returns an error for truncated input, invalid JSON, protocol invariant violations, or an
    /// advertised body length above the configured limit.
    pub fn decode(&self, mut reader: impl Read) -> Result<Envelope, ProtocolError> {
        let mut prefix = [0_u8; 4];
        reader.read_exact(&mut prefix)?;
        let length = u32::from_be_bytes(prefix) as usize;
        if length == 0 {
            return Err(ProtocolError::EmptyFrame);
        }
        if length > self.effective_limit() {
            return Err(ProtocolError::FrameTooLarge {
                actual: length,
                maximum: self.effective_limit(),
            });
        }
        let mut body = vec![0_u8; length];
        reader.read_exact(&mut body)?;
        decode_body(&body)
    }

    const fn effective_limit(&self) -> usize {
        if self.max_frame_body_bytes < MAX_FRAME_BODY_BYTES {
            self.max_frame_body_bytes
        } else {
            MAX_FRAME_BODY_BYTES
        }
    }
}

impl Default for Codec {
    fn default() -> Self {
        Self::new(MAX_FRAME_BODY_BYTES)
    }
}

#[derive(Clone, Debug)]
pub struct ServerHandshake {
    supported_protocol_minor: u16,
    capabilities: BTreeSet<Capability>,
    limits: ProtocolLimits,
}

impl ServerHandshake {
    #[must_use]
    pub fn new(
        supported_protocol_minor: u16,
        capabilities: impl IntoIterator<Item = Capability>,
        limits: ProtocolLimits,
    ) -> Self {
        Self {
            supported_protocol_minor,
            capabilities: capabilities.into_iter().collect(),
            limits,
        }
    }

    /// Negotiates one client hello and returns a correlated hello or rejection response.
    ///
    /// # Errors
    /// Returns an error when the request ID is reserved or the message is not a client hello.
    pub fn handle(&self, request: &Envelope) -> Result<Envelope, ProtocolError> {
        if request.request_id == 0 {
            return Err(ProtocolError::ReservedRequestId);
        }
        let Message::ClientHello(hello) = &request.message else {
            return Err(ProtocolError::UnexpectedHandshakeMessage(
                request.message.kind().to_owned(),
            ));
        };

        if request.protocol_major != PROTOCOL_MAJOR {
            return Ok(rejection(
                request.request_id,
                RejectionCode::IncompatibleMajor,
                "protocol major is incompatible",
            ));
        }
        if hello.minimum_protocol_minor > hello.maximum_protocol_minor {
            return Ok(rejection(
                request.request_id,
                RejectionCode::InvalidHello,
                "protocol minor range is invalid",
            ));
        }
        if !(hello.minimum_protocol_minor..=hello.maximum_protocol_minor)
            .contains(&self.supported_protocol_minor)
        {
            return Ok(rejection(
                request.request_id,
                RejectionCode::IncompatibleMinor,
                "protocol minor is incompatible",
            ));
        }

        let capabilities = hello
            .capabilities
            .intersection(&self.capabilities)
            .cloned()
            .collect();
        Ok(Envelope::new(
            PROTOCOL_MAJOR,
            self.supported_protocol_minor,
            request.request_id,
            Message::ServerHello(ServerHello {
                selected_protocol_minor: self.supported_protocol_minor,
                capabilities,
                limits: self.limits,
            }),
        ))
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("protocol I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("protocol JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("protocol frame body is empty")]
    EmptyFrame,
    #[error("protocol frame body is {actual} bytes, exceeding the {maximum}-byte limit")]
    FrameTooLarge { actual: usize, maximum: usize },
    #[error("invalid protocol magic {0:?}")]
    InvalidMagic(String),
    #[error("payload length declares {declared} bytes but contains {actual} bytes")]
    PayloadLengthMismatch { declared: u64, actual: usize },
    #[error("unknown protocol message kind {0:?}")]
    UnknownMessageKind(String),
    #[error("request ID 0 is reserved for lifecycle events")]
    ReservedRequestId,
    #[error("unexpected message during handshake: {0}")]
    UnexpectedHandshakeMessage(String),
}

#[derive(Serialize)]
struct WireEnvelopeRef<'a> {
    magic: &'static str,
    protocol_major: u16,
    protocol_minor: u16,
    kind: &'static str,
    request_id: u64,
    payload_length: u64,
    payload: &'a Value,
}

#[derive(Deserialize)]
struct WireEnvelope {
    magic: String,
    protocol_major: u16,
    protocol_minor: u16,
    kind: String,
    request_id: u64,
    payload_length: u64,
    payload: Box<RawValue>,
}

fn encode_body(envelope: &Envelope) -> Result<Vec<u8>, ProtocolError> {
    let payload = envelope.message.payload()?;
    let payload_length = serde_json::to_vec(&payload)?.len() as u64;
    Ok(serde_json::to_vec(&WireEnvelopeRef {
        magic: MAGIC,
        protocol_major: envelope.protocol_major,
        protocol_minor: envelope.protocol_minor,
        kind: envelope.message.kind(),
        request_id: envelope.request_id,
        payload_length,
        payload: &payload,
    })?)
}

fn decode_body(body: &[u8]) -> Result<Envelope, ProtocolError> {
    let wire: WireEnvelope = serde_json::from_slice(body)?;
    if wire.magic != MAGIC {
        return Err(ProtocolError::InvalidMagic(wire.magic));
    }
    let actual_payload_length = wire.payload.get().len();
    if wire.payload_length != actual_payload_length as u64 {
        return Err(ProtocolError::PayloadLengthMismatch {
            declared: wire.payload_length,
            actual: actual_payload_length,
        });
    }
    Ok(Envelope::new(
        wire.protocol_major,
        wire.protocol_minor,
        wire.request_id,
        Message::from_wire(&wire.kind, serde_json::from_str(wire.payload.get())?)?,
    ))
}

fn rejection(request_id: u64, code: RejectionCode, message: &str) -> Envelope {
    Envelope::v4(
        request_id,
        Message::Rejected(Rejected {
            code,
            message: message.to_owned(),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities() -> BTreeSet<Capability> {
        [
            Capability::static_document(),
            Capability::display_list_v1(),
            Capability::cancellable_navigation(),
        ]
        .into_iter()
        .collect()
    }

    fn client_hello() -> Envelope {
        Envelope::v4(
            1,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 0,
                maximum_protocol_minor: 1,
                capabilities: capabilities(),
            }),
        )
    }

    fn encoded(envelope: &Envelope) -> Vec<u8> {
        let mut bytes = Vec::new();
        Codec::default().encode(&mut bytes, envelope).unwrap();
        bytes
    }

    #[test]
    fn codec_round_trips_typed_envelopes() {
        let server = ServerHandshake::new(0, capabilities(), ProtocolLimits::default());
        let accepted = server.handle(&client_hello()).unwrap();
        let incompatible = Envelope::new(
            5,
            0,
            2,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 0,
                maximum_protocol_minor: 0,
                capabilities: capabilities(),
            }),
        );
        let rejected = server.handle(&incompatible).unwrap();

        for envelope in [client_hello(), accepted, rejected] {
            assert_eq!(
                Codec::default()
                    .decode(encoded(&envelope).as_slice())
                    .unwrap(),
                envelope
            );
        }
    }

    #[test]
    fn codec_validates_original_payload_bytes_with_layout_floats() {
        let envelope = Envelope::v4(
            9,
            Message::Response(Response::Rendered {
                page_id: PageId::new(),
                navigation_id: NavigationId::zero().saturating_next(),
                final_url: "https://example.test/floats".parse().unwrap(),
                title: "Rust UI neutral / 框架无关".to_owned(),
                display_list: DisplayList {
                    commands: Vec::new(),
                    content_height: 123.456_78,
                },
                diagnostics: Vec::new(),
            }),
        );

        assert_eq!(
            Codec::default()
                .decode(encoded(&envelope).as_slice())
                .unwrap(),
            envelope
        );
    }

    #[test]
    fn codec_enforces_the_frame_limit_before_reading_a_body() {
        let envelope = client_hello();
        let bytes = encoded(&envelope);
        let body_length = bytes.len() - 4;

        let mut exact = Vec::new();
        Codec::new(body_length)
            .encode(&mut exact, &envelope)
            .unwrap();
        assert_eq!(
            Codec::new(body_length).decode(exact.as_slice()).unwrap(),
            envelope
        );

        assert!(matches!(
            Codec::new(body_length - 1).encode(Vec::new(), &envelope),
            Err(ProtocolError::FrameTooLarge { .. })
        ));
        assert!(matches!(
            Codec::new(body_length - 1).decode(bytes.as_slice()),
            Err(ProtocolError::FrameTooLarge { .. })
        ));

        let oversized = u32::try_from(MAX_FRAME_BODY_BYTES + 1)
            .unwrap()
            .to_be_bytes();
        assert!(matches!(
            Codec::new(usize::MAX).decode(oversized.as_slice()),
            Err(ProtocolError::FrameTooLarge {
                maximum: MAX_FRAME_BODY_BYTES,
                ..
            })
        ));
    }

    #[test]
    fn codec_rejects_empty_truncated_and_malformed_frames() {
        assert!(matches!(
            Codec::default().decode([0, 0, 0, 0].as_slice()),
            Err(ProtocolError::EmptyFrame)
        ));
        assert!(matches!(
            Codec::default().decode([0, 0, 0, 4, b'{'].as_slice()),
            Err(ProtocolError::Io(_))
        ));
        assert!(matches!(
            Codec::default().decode([0, 0, 0, 1, b'{'].as_slice()),
            Err(ProtocolError::Json(_))
        ));
    }

    #[test]
    fn codec_ignores_optional_fields_and_rejects_unknown_kinds() {
        let bytes = encoded(&client_hello());
        let mut wire: Value = serde_json::from_slice(&bytes[4..]).unwrap();
        wire["future_optional_field"] = Value::Bool(true);
        wire["payload"]["future_optional_field"] = Value::String("supported".to_owned());
        wire["payload_length"] = Value::from(
            u64::try_from(serde_json::to_vec(&wire["payload"]).unwrap().len()).unwrap(),
        );
        let body = serde_json::to_vec(&wire).unwrap();
        let mut extended = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
        extended.extend(body);
        Codec::default().decode(extended.as_slice()).unwrap();

        wire["kind"] = Value::String("future_message".to_owned());
        let body = serde_json::to_vec(&wire).unwrap();
        let mut unknown = u32::try_from(body.len()).unwrap().to_be_bytes().to_vec();
        unknown.extend(body);
        assert!(matches!(
            Codec::default().decode(unknown.as_slice()),
            Err(ProtocolError::UnknownMessageKind(kind)) if kind == "future_message"
        ));
    }

    #[test]
    fn codec_rejects_invalid_magic_and_payload_length() {
        let bytes = encoded(&client_hello());
        let mut wire: Value = serde_json::from_slice(&bytes[4..]).unwrap();
        wire["magic"] = Value::String("NOPE".to_owned());
        let body = serde_json::to_vec(&wire).unwrap();
        assert!(matches!(
            decode_body(&body),
            Err(ProtocolError::InvalidMagic(magic)) if magic == "NOPE"
        ));

        wire["magic"] = Value::String(MAGIC.to_owned());
        wire["payload_length"] = Value::from(0);
        let body = serde_json::to_vec(&wire).unwrap();
        assert!(matches!(
            decode_body(&body),
            Err(ProtocolError::PayloadLengthMismatch { .. })
        ));
    }

    #[test]
    fn handshake_negotiates_minor_version_capabilities_and_limits() {
        let server = ServerHandshake::new(
            0,
            [
                Capability::static_document(),
                Capability::renderer_restart_v1(),
            ],
            ProtocolLimits::default(),
        );
        let response = server.handle(&client_hello()).unwrap();
        assert_eq!(response.request_id(), 1);
        let Message::ServerHello(hello) = response.message() else {
            panic!("handshake should succeed");
        };
        assert_eq!(hello.selected_protocol_minor, 0);
        assert_eq!(
            hello.capabilities,
            [Capability::static_document()].into_iter().collect()
        );
        assert_eq!(hello.limits, ProtocolLimits::default());
    }

    #[test]
    fn handshake_rejects_incompatible_and_invalid_versions() {
        let server = ServerHandshake::new(0, capabilities(), ProtocolLimits::default());
        let incompatible_major = Envelope::new(
            5,
            0,
            7,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 0,
                maximum_protocol_minor: 0,
                capabilities: capabilities(),
            }),
        );
        assert_rejected(
            &server.handle(&incompatible_major).unwrap(),
            RejectionCode::IncompatibleMajor,
        );

        let incompatible_minor = Envelope::v4(
            8,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 2,
                maximum_protocol_minor: 3,
                capabilities: capabilities(),
            }),
        );
        assert_rejected(
            &server.handle(&incompatible_minor).unwrap(),
            RejectionCode::IncompatibleMinor,
        );

        let invalid_range = Envelope::v4(
            9,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 3,
                maximum_protocol_minor: 2,
                capabilities: capabilities(),
            }),
        );
        assert_rejected(
            &server.handle(&invalid_range).unwrap(),
            RejectionCode::InvalidHello,
        );
    }

    #[test]
    fn handshake_rejects_reserved_ids_and_non_hello_messages() {
        let server = ServerHandshake::new(0, capabilities(), ProtocolLimits::default());
        let reserved = Envelope::v4(
            0,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 0,
                maximum_protocol_minor: 0,
                capabilities: capabilities(),
            }),
        );
        assert!(matches!(
            server.handle(&reserved),
            Err(ProtocolError::ReservedRequestId)
        ));

        let response = Envelope::v4(
            1,
            Message::Rejected(Rejected {
                code: RejectionCode::InvalidHello,
                message: "invalid".to_owned(),
            }),
        );
        assert!(matches!(
            server.handle(&response),
            Err(ProtocolError::UnexpectedHandshakeMessage(kind)) if kind == "rejected"
        ));
    }

    fn assert_rejected(envelope: &Envelope, expected: RejectionCode) {
        let Message::Rejected(rejected) = envelope.message() else {
            panic!("handshake should be rejected");
        };
        assert_eq!(rejected.code, expected);
    }
}
