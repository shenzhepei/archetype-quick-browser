use std::time::Duration;

use arch_browser::runtime_supervisor::{RuntimeProcessError, RuntimeSupervisor, StaticDocument};
use archetype_types::{NavigationId, PageId};

const PROCESS_TIMEOUT: Duration = Duration::from_secs(6);

fn document() -> StaticDocument {
    StaticDocument {
        page_id: PageId::new(),
        navigation_id: NavigationId::zero().saturating_next(),
        url: "https://example.test/runtime".parse().unwrap(),
        html: "<!doctype html><title>Subprocess</title><style>p { color: blue }</style><p>Hello from Runtime</p>"
            .to_owned(),
        viewport_width_px: 1024,
    }
}

fn start() -> RuntimeSupervisor {
    let (supervisor, ready) = RuntimeSupervisor::spawn(env!("CARGO_BIN_EXE_archetype-runtime"))
        .expect("supervisor thread should start");
    ready
        .recv_timeout(PROCESS_TIMEOUT)
        .expect("runtime handshake should complete")
        .expect("runtime handshake should succeed");
    supervisor
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
fn repeated_runtime_termination_keeps_the_supervisor_process_alive() {
    for _ in 0..20 {
        let supervisor = start();
        supervisor
            .shutdown()
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("shutdown should complete")
            .expect("shutdown should reap the runtime");
    }
}
