use std::time::Duration;

use archetype_protocol::{
    Codec, Envelope, MemoryEndpoint, Message, Request, RequestRouter, Response, RouterError,
    TransportError, memory_transport,
};
use archetype_types::{ArchetypeUrl, NavigationId, PageId};

struct FakeRuntime;

impl FakeRuntime {
    fn response(request: &Envelope) -> Envelope {
        let response = match request.message() {
            Message::Request(Request::Navigate { .. } | Request::RenderDocument { .. }) => {
                Response::Accepted
            }
            Message::Request(Request::Cancel { target_request_id }) => Response::Cancelled {
                target_request_id: *target_request_id,
            },
            _ => panic!("fake runtime only accepts requests"),
        };
        Envelope::v4(request.request_id(), Message::Response(response))
    }

    fn receive(endpoint: &MemoryEndpoint) -> Envelope {
        endpoint
            .try_receive()
            .unwrap()
            .expect("request should be queued")
    }
}

fn navigate(url: &str) -> Request {
    Request::Navigate {
        page_id: PageId::new(),
        navigation_id: NavigationId::zero().saturating_next(),
        url: url.parse::<ArchetypeUrl>().unwrap(),
    }
}

#[test]
fn router_correlates_out_of_order_fake_runtime_responses() {
    let (client, runtime) = memory_transport(4, 64 * 1024, Codec::default());
    let mut router = RequestRouter::new(2);
    let now = Duration::ZERO;
    let first = router
        .begin(
            navigate("https://example.test/one"),
            now,
            Duration::from_secs(5),
        )
        .unwrap();
    let second = router
        .begin(
            navigate("https://example.test/two"),
            now,
            Duration::from_secs(5),
        )
        .unwrap();
    client.send(&first).unwrap();
    client.send(&second).unwrap();

    let runtime_first = FakeRuntime::receive(&runtime);
    let runtime_second = FakeRuntime::receive(&runtime);
    runtime
        .send(&FakeRuntime::response(&runtime_second))
        .unwrap();
    runtime
        .send(&FakeRuntime::response(&runtime_first))
        .unwrap();

    let second_response = client.try_receive().unwrap().unwrap();
    let first_response = client.try_receive().unwrap().unwrap();
    assert_eq!(
        router
            .route(second_response, now)
            .unwrap()
            .original_request_id,
        second.request_id()
    );
    assert_eq!(
        router
            .route(first_response, now)
            .unwrap()
            .original_request_id,
        first.request_id()
    );
    assert_eq!(router.pending_count(), 0);
}

#[test]
fn cancellation_replaces_the_target_and_discards_its_late_response() {
    let (client, runtime) = memory_transport(4, 64 * 1024, Codec::default());
    let mut router = RequestRouter::new(1);
    let now = Duration::ZERO;
    let navigation = router
        .begin(
            navigate("https://example.test/slow"),
            now,
            Duration::from_secs(10),
        )
        .unwrap();
    client.send(&navigation).unwrap();
    let held_navigation = FakeRuntime::receive(&runtime);

    let cancellation = router
        .cancel(navigation.request_id(), now, Duration::from_secs(2))
        .unwrap();
    assert!(cancellation.request_id() > navigation.request_id());
    assert_eq!(router.pending_count(), 1);
    client.send(&cancellation).unwrap();
    let runtime_cancellation = FakeRuntime::receive(&runtime);
    runtime
        .send(&FakeRuntime::response(&runtime_cancellation))
        .unwrap();

    let completion = router
        .route(client.try_receive().unwrap().unwrap(), now)
        .unwrap();
    assert_eq!(completion.original_request_id, navigation.request_id());
    assert_eq!(completion.response_request_id, cancellation.request_id());
    assert!(matches!(completion.response, Response::Cancelled { .. }));

    runtime
        .send(&FakeRuntime::response(&held_navigation))
        .unwrap();
    assert!(matches!(
        router.route(client.try_receive().unwrap().unwrap(), now),
        Err(RouterError::UnknownRequest(request_id)) if request_id == navigation.request_id()
    ));
}

#[test]
fn router_expires_requests_without_sleeping() {
    let mut router = RequestRouter::new(2);
    let request = router
        .begin(
            navigate("https://example.test/timeout"),
            Duration::from_secs(10),
            Duration::from_secs(2),
        )
        .unwrap();
    assert_eq!(router.expire(Duration::from_secs(11)), Vec::<u64>::new());
    assert_eq!(
        router.expire(Duration::from_secs(12)),
        vec![request.request_id()]
    );
    assert_eq!(router.pending_count(), 0);
}

#[test]
fn router_and_transport_apply_backpressure() {
    let mut router = RequestRouter::new(1);
    let first = router
        .begin(
            navigate("https://example.test/one"),
            Duration::ZERO,
            Duration::from_secs(1),
        )
        .unwrap();
    assert!(matches!(
        router.begin(
            navigate("https://example.test/two"),
            Duration::ZERO,
            Duration::from_secs(1)
        ),
        Err(RouterError::Backpressure { maximum: 1 })
    ));

    let (sender, receiver) = memory_transport(1, 64 * 1024, Codec::default());
    sender.send(&first).unwrap();
    assert!(sender.outgoing_queued_bytes() > 0);
    assert!(matches!(
        sender.send(&first),
        Err(TransportError::QueueFull)
    ));
    FakeRuntime::receive(&receiver);
    assert_eq!(sender.outgoing_queued_bytes(), 0);

    let (byte_limited, _peer) = memory_transport(2, 1, Codec::default());
    assert!(matches!(
        byte_limited.send(&first),
        Err(TransportError::ByteLimit { maximum: 1 })
    ));
    assert_eq!(byte_limited.outgoing_queued_bytes(), 0);
}
