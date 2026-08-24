use arch_paint::DisplayList;
use archetype_protocol::{Codec, Envelope, Message, Request, Response};
use archetype_types::{ArchetypeUrl, NavigationId, PageId};

#[test]
fn static_document_messages_round_trip_with_a_display_list() {
    let page_id = PageId::new();
    let navigation_id = NavigationId::zero().saturating_next();
    let url = "https://example.test/document"
        .parse::<ArchetypeUrl>()
        .unwrap();
    let request = Envelope::v4(
        2,
        Message::Request(Request::RenderDocument {
            page_id: page_id.clone(),
            navigation_id,
            url: url.clone(),
            html: "<!doctype html><title>Example</title><p>Hello</p>".to_owned(),
            viewport_width_px: 1280,
        }),
    );
    let response = Envelope::v4(
        2,
        Message::Response(Response::Rendered {
            page_id,
            navigation_id,
            final_url: url,
            title: "Example".to_owned(),
            display_list: DisplayList {
                commands: Vec::new(),
                content_height: 24.0,
            },
            diagnostics: vec!["JavaScript is disabled".to_owned()],
        }),
    );

    for envelope in [request, response] {
        let mut frame = Vec::new();
        Codec::default().encode(&mut frame, &envelope).unwrap();
        assert_eq!(Codec::default().decode(frame.as_slice()).unwrap(), envelope);
    }
}
