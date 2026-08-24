use std::collections::BTreeSet;

use archetype_protocol::{
    Capability, ClientHello, Codec, Envelope, Message, ProtocolLimits, ServerHandshake,
};

#[test]
fn client_and_server_complete_a_framed_handshake() {
    let client_capabilities: BTreeSet<_> =
        [Capability::static_document(), Capability::display_list_v1()]
            .into_iter()
            .collect();
    let client_request = Envelope::v4(
        1,
        Message::ClientHello(ClientHello {
            minimum_protocol_minor: 0,
            maximum_protocol_minor: 0,
            capabilities: client_capabilities,
        }),
    );

    let codec = Codec::default();
    let mut client_to_server = Vec::new();
    codec
        .encode(&mut client_to_server, &client_request)
        .unwrap();
    let server_request = codec.decode(client_to_server.as_slice()).unwrap();

    let server = ServerHandshake::new(
        0,
        [Capability::static_document()],
        ProtocolLimits::default(),
    );
    let server_response = server.handle(&server_request).unwrap();
    let mut server_to_client = Vec::new();
    codec
        .encode(&mut server_to_client, &server_response)
        .unwrap();
    let client_response = codec.decode(server_to_client.as_slice()).unwrap();

    assert_eq!(client_response.request_id(), client_request.request_id());
    let Message::ServerHello(hello) = client_response.message() else {
        panic!("server should accept the compatible client");
    };
    assert_eq!(
        hello.capabilities,
        [Capability::static_document()].into_iter().collect()
    );
}
