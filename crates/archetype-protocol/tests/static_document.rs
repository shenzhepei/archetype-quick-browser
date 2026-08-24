use arch_paint::DisplayList;
use archetype_protocol::{
    BrokeredResource, Codec, Envelope, Message, Request, ResourceBytes, ResourceKind, Response,
};
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
            viewport_height_px: 720,
            resources: vec![BrokeredResource {
                requested_url: "https://example.test/image.png".parse().unwrap(),
                final_url: "https://example.test/image.png".parse().unwrap(),
                kind: ResourceKind::Image,
                body: ResourceBytes::new(vec![0, 1, 2, 254, 255]),
            }],
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

#[test]
fn legacy_static_document_defaults_the_viewport_height() {
    let request = serde_json::json!({
        "type": "render_document",
        "page_id": PageId::new(),
        "navigation_id": 1,
        "url": "https://example.test/legacy",
        "html": "<p>legacy</p>",
        "viewport_width_px": 800,
        "resources": []
    });
    let decoded: Request = serde_json::from_value(request).unwrap();
    assert!(matches!(
        decoded,
        Request::RenderDocument {
            viewport_height_px: 900,
            ..
        }
    ));
}

#[test]
fn resource_bytes_reject_invalid_base64() {
    assert!(serde_json::from_str::<ResourceBytes>("\"not base64!\"").is_err());
}
