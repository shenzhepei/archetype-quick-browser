use std::{
    collections::HashMap,
    io::{self, Read, Write},
};

use arch_dom::NodeKind;
use archetype_protocol::{
    Capability, Codec, Envelope, Message, PROTOCOL_MAJOR, PROTOCOL_MINOR, ProtocolError,
    ProtocolLimits, Request, Response, ServerHandshake,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Protocol(#[from] ProtocolError),
    #[error("runtime output flush failed: {0}")]
    Flush(#[source] io::Error),
}

/// Serves one renderer connection until shutdown or input EOF.
///
/// # Errors
/// Returns an error when the framed protocol or output stream fails.
pub fn serve(mut input: impl Read, mut output: impl Write) -> Result<(), RuntimeError> {
    let codec = Codec::default();
    let handshake_request = match codec.decode(&mut input) {
        Ok(envelope) => envelope,
        Err(ProtocolError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let handshake = ServerHandshake::new(
        PROTOCOL_MINOR,
        [
            Capability::static_document(),
            Capability::display_list_v1(),
            Capability::cancellable_navigation(),
        ],
        ProtocolLimits::default(),
    );
    let handshake_response = handshake.handle(&handshake_request)?;
    let accepted = matches!(handshake_response.message(), Message::ServerHello(_));
    write_envelope(&codec, &mut output, &handshake_response)?;
    if !accepted {
        return Ok(());
    }

    loop {
        let envelope = match codec.decode(&mut input) {
            Ok(envelope) => envelope,
            Err(ProtocolError::Io(error)) if error.kind() == io::ErrorKind::UnexpectedEof => {
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let response = handle_request(&envelope);
        write_envelope(
            &codec,
            &mut output,
            &Envelope::v4(envelope.request_id(), Message::Response(response)),
        )?;
    }
}

fn write_envelope(
    codec: &Codec,
    output: &mut impl Write,
    envelope: &Envelope,
) -> Result<(), RuntimeError> {
    codec.encode(&mut *output, envelope)?;
    output.flush().map_err(RuntimeError::Flush)
}

fn handle_request(envelope: &Envelope) -> Response {
    if envelope.protocol_major() != PROTOCOL_MAJOR || envelope.protocol_minor() != PROTOCOL_MINOR {
        return Response::Failed {
            code: "unsupported_protocol_version".to_owned(),
            message: "request protocol version does not match the negotiated version".to_owned(),
        };
    }
    match envelope.message() {
        Message::Request(Request::RenderDocument {
            page_id,
            navigation_id,
            url,
            html,
            viewport_width_px,
        }) if *viewport_width_px > 0 => {
            let Ok(viewport_width_px) = u16::try_from(*viewport_width_px) else {
                return Response::Failed {
                    code: "invalid_viewport".to_owned(),
                    message: "viewport width exceeds 65535 pixels".to_owned(),
                };
            };
            let document = arch_html::parse(html);
            let stylesheet = arch_css::parse(&inline_css(&document));
            let styled = arch_style::style_document(&document, &stylesheet);
            let mut diagnostics = stylesheet.diagnostics;
            diagnostics.extend(document_diagnostics(&document));
            let layout = arch_layout::layout(
                &document,
                &styled,
                f32::from(viewport_width_px),
                &HashMap::new(),
                &HashMap::new(),
            );
            Response::Rendered {
                page_id: page_id.clone(),
                navigation_id: *navigation_id,
                final_url: url.clone(),
                title: arch_html::title(&document).unwrap_or_else(|| url.as_str().to_owned()),
                display_list: arch_paint::paint(&layout),
                diagnostics,
            }
        }
        Message::Request(Request::RenderDocument { .. }) => Response::Failed {
            code: "invalid_viewport".to_owned(),
            message: "viewport width must be greater than zero".to_owned(),
        },
        Message::Request(Request::Navigate { .. }) => Response::Accepted,
        Message::Request(Request::Cancel { target_request_id }) => Response::Cancelled {
            target_request_id: *target_request_id,
        },
        _ => Response::Failed {
            code: "unexpected_message".to_owned(),
            message: "runtime accepts request messages after the handshake".to_owned(),
        },
    }
}

fn inline_css(document: &arch_dom::Document) -> String {
    let mut output = String::new();
    for node in document.descendants(document.root()) {
        if matches!(&node.kind, NodeKind::Element(element) if element.name == "style") {
            output.push_str(&document.text_content(node.id));
            output.push('\n');
        }
    }
    output
}

fn document_diagnostics(document: &arch_dom::Document) -> Vec<String> {
    let mut script_elements = 0_usize;
    let mut event_attributes = 0_usize;
    for node in document.descendants(document.root()) {
        let NodeKind::Element(element) = &node.kind else {
            continue;
        };
        script_elements += usize::from(element.name == "script");
        event_attributes += element
            .attributes
            .iter()
            .filter(|(name, _)| name.to_ascii_lowercase().starts_with("on"))
            .count();
    }
    let mut diagnostics = Vec::new();
    if script_elements > 0 {
        diagnostics.push(format!(
            "ignored {script_elements} script element(s); JavaScript is disabled"
        ));
    }
    if event_attributes > 0 {
        diagnostics.push(format!(
            "ignored {event_attributes} inline event attribute(s); JavaScript is disabled"
        ));
    }
    diagnostics
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use archetype_protocol::{ClientHello, Message};
    use archetype_types::{ArchetypeUrl, NavigationId, PageId};

    use super::*;

    fn encode(envelopes: &[Envelope]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for envelope in envelopes {
            Codec::default().encode(&mut bytes, envelope).unwrap();
        }
        bytes
    }

    fn hello() -> Envelope {
        Envelope::v4(
            1,
            Message::ClientHello(ClientHello {
                minimum_protocol_minor: 0,
                maximum_protocol_minor: 0,
                capabilities: BTreeSet::from([
                    Capability::static_document(),
                    Capability::display_list_v1(),
                ]),
            }),
        )
    }

    #[test]
    fn handshake_and_static_render_use_the_framed_stream() {
        let page_id = PageId::new();
        let navigation_id = NavigationId::zero().saturating_next();
        let url = "https://example.test/page".parse::<ArchetypeUrl>().unwrap();
        let render = Envelope::v4(
            2,
            Message::Request(Request::RenderDocument {
                page_id: page_id.clone(),
                navigation_id,
                url: url.clone(),
                html: "<title>Runtime</title><style>p { color: red }</style><p onclick='x()'>Hello</p>"
                    .to_owned(),
                viewport_width_px: 800,
            }),
        );
        let mut output = Vec::new();

        serve(encode(&[hello(), render]).as_slice(), &mut output).unwrap();

        let mut responses = output.as_slice();
        assert!(matches!(
            Codec::default().decode(&mut responses).unwrap().message(),
            Message::ServerHello(_)
        ));
        let rendered = Codec::default().decode(&mut responses).unwrap();
        let Message::Response(Response::Rendered {
            page_id: returned_page_id,
            navigation_id: returned_navigation_id,
            final_url,
            title,
            display_list,
            diagnostics,
        }) = rendered.message()
        else {
            panic!("runtime should return a rendered document");
        };
        assert_eq!(returned_page_id, &page_id);
        assert_eq!(*returned_navigation_id, navigation_id);
        assert_eq!(final_url, &url);
        assert_eq!(title, "Runtime");
        assert!(!display_list.commands.is_empty());
        assert!(
            diagnostics
                .iter()
                .any(|item| item.contains("event attribute"))
        );
    }

    #[test]
    fn invalid_viewport_returns_a_structured_failure() {
        let render = Envelope::v4(
            2,
            Message::Request(Request::RenderDocument {
                page_id: PageId::new(),
                navigation_id: NavigationId::zero(),
                url: "https://example.test/".parse().unwrap(),
                html: String::new(),
                viewport_width_px: 0,
            }),
        );
        let mut output = Vec::new();

        serve(encode(&[hello(), render]).as_slice(), &mut output).unwrap();

        let mut responses = output.as_slice();
        Codec::default().decode(&mut responses).unwrap();
        let failure = Codec::default().decode(&mut responses).unwrap();
        assert!(matches!(
            failure.message(),
            Message::Response(Response::Failed { code, .. }) if code == "invalid_viewport"
        ));
    }
}
