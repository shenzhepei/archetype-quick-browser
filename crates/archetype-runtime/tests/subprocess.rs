use std::{
    thread,
    time::{Duration, Instant},
};

use arch_browser::runtime_broker::{BrokerRequest, load_static_document};
use arch_net::Loader;
use archetype_sdk::runtime_client::{
    RuntimeLimits, RuntimeProcessError, RuntimeSupervisor, StaticDocument,
};
use archetype_types::{NavigationId, PageId};
use url::Url;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(6);

fn document() -> StaticDocument {
    StaticDocument {
        page_id: PageId::new(),
        navigation_id: NavigationId::zero().saturating_next(),
        url: "https://example.test/runtime".parse().unwrap(),
        html: "<!doctype html><title>Subprocess</title><style>p { color: blue }</style><p>Hello from Runtime</p>"
            .to_owned(),
        viewport_width_px: 1024,
        resources: Vec::new(),
        broker_diagnostics: Vec::new(),
    }
}

fn start() -> RuntimeSupervisor {
    start_with_limits(RuntimeLimits::default())
}

fn start_with_limits(limits: RuntimeLimits) -> RuntimeSupervisor {
    let (supervisor, ready) =
        RuntimeSupervisor::spawn_with_limits(env!("CARGO_BIN_EXE_archetype-runtime"), limits)
            .expect("supervisor thread should start");
    ready
        .recv_timeout(PROCESS_TIMEOUT)
        .expect("runtime handshake should complete")
        .expect("runtime handshake should succeed");
    supervisor
}

#[test]
fn encoded_requests_respect_the_in_flight_byte_limit() {
    let supervisor = start_with_limits(RuntimeLimits {
        maximum_in_flight_bytes: 1,
        ..RuntimeLimits::default()
    });

    assert!(matches!(
        supervisor
            .render_document(document())
            .recv_timeout(PROCESS_TIMEOUT),
        Ok(Err(RuntimeProcessError::Backpressure))
    ));
}

#[test]
fn rss_limit_terminates_runtime_with_a_structured_reason() {
    let supervisor = start_with_limits(RuntimeLimits {
        maximum_rss_bytes: 1,
        ..RuntimeLimits::default()
    });
    let deadline = Instant::now() + PROCESS_TIMEOUT;

    loop {
        let result = supervisor
            .render_document(document())
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("render should return a structured result");
        if result
            == Err(RuntimeProcessError::ResourceLimit {
                resource: "RSS".to_owned(),
            })
        {
            break;
        }
        assert!(result.is_ok(), "unexpected runtime result: {result:?}");
        assert!(Instant::now() < deadline, "RSS limit was not enforced");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn browser_supervisor_renders_through_the_real_subprocess() {
    let supervisor = start();
    let expected = document();

    let rendered = supervisor
        .render_document(expected.clone())
        .recv_timeout(PROCESS_TIMEOUT)
        .expect("render should complete")
        .expect("render should succeed");

    assert_eq!(rendered.page_id, expected.page_id);
    assert_eq!(rendered.navigation_id, expected.navigation_id);
    assert_eq!(rendered.final_url, expected.url);
    assert_eq!(rendered.title, "Subprocess");
    assert!(!rendered.display_list.commands.is_empty());
    supervisor
        .shutdown()
        .recv_timeout(PROCESS_TIMEOUT)
        .expect("shutdown should complete")
        .expect("shutdown should succeed");
    assert!(matches!(
        supervisor
            .render_document(document())
            .recv_timeout(PROCESS_TIMEOUT),
        Ok(Err(RuntimeProcessError::RuntimeDisconnected))
    ));
}

#[test]
fn one_hundred_forced_runtime_restarts_keep_the_supervisor_alive() {
    for _ in 0..100 {
        let supervisor = start();
        supervisor
            .force_restart()
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("forced termination should complete")
            .expect("supervisor should accept forced termination");
        supervisor
            .render_document(document())
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("render after restart should complete")
            .expect("render after restart should succeed");
        supervisor
            .shutdown()
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("shutdown should complete")
            .expect("shutdown should reap the runtime");
    }
}

#[test]
fn brokered_fixture_resources_render_through_the_real_subprocess() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/pages/05-image/index.html");
    let document = load_static_document(
        &Loader::default(),
        &BrokerRequest {
            page_id: PageId::new(),
            navigation_id: NavigationId::zero().saturating_next(),
            url: Url::from_file_path(fixture).unwrap(),
            viewport_width_px: 1280,
        },
    )
    .unwrap();
    let supervisor = start();

    let rendered = supervisor
        .render_document(document)
        .recv_timeout(PROCESS_TIMEOUT)
        .expect("render should complete")
        .expect("render should succeed");

    assert!(
        rendered
            .display_list
            .commands
            .iter()
            .any(|command| matches!(
                command,
                arch_paint::DisplayCommand::Image { loaded: true, .. }
            ))
    );
    assert_eq!(rendered.image_resources.len(), 1);
    supervisor
        .shutdown()
        .recv_timeout(PROCESS_TIMEOUT)
        .expect("shutdown should complete")
        .expect("shutdown should succeed");
}
