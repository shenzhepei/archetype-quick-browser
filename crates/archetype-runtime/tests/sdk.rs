use std::{fs, path::Path};

use archetype_sdk::{Engine, PageEvent, PageOptions, SdkError, StaticDocument};
use archetype_types::PageId;
use futures_executor::block_on;
use sha2::{Digest, Sha256};

fn engine() -> Engine {
    block_on(
        Engine::builder()
            .runtime_path(env!("CARGO_BIN_EXE_archetype-runtime"))
            .build(),
    )
    .expect("SDK should start the test Runtime")
}

#[test]
fn partner_sdk_renders_rgba_and_png_through_runtime() {
    let engine = engine();
    let page = block_on(engine.create_page(PageOptions::new(640, 360))).unwrap();
    let document = StaticDocument::new(
        "https://example.test/sdk",
        "<title>Partner SDK</title><style>body{background:#eef4ff;color:#173b72}main{display:flex;gap:16px}</style><main>Hello <strong>SDK</strong></main>",
    )
    .unwrap();

    let navigation = block_on(page.render(document)).unwrap();

    assert_eq!(navigation.title(), "Partner SDK");
    assert_eq!(navigation.frame().width_px(), 640);
    assert_eq!(navigation.frame().height_px(), 360);
    assert_eq!(navigation.frame().stride_bytes(), 640 * 4);
    assert_eq!(navigation.frame().rgba().len(), 640 * 360 * 4);
    assert!(
        navigation
            .frame()
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel != [255, 255, 255, 255])
    );
    assert!(matches!(
        block_on(page.next_event()).unwrap(),
        PageEvent::NavigationStarted { .. }
    ));
    assert!(matches!(
        block_on(page.next_event()).unwrap(),
        PageEvent::FrameReady { .. }
    ));
    let output = std::env::temp_dir().join(format!("archetype-sdk-{}.png", PageId::new()));
    navigation.frame().save_png(&output).unwrap();
    let decoded = image::open(&output).unwrap();
    assert_eq!(decoded.width(), 640);
    assert_eq!(decoded.height(), 360);
    fs::remove_file(output).unwrap();
    block_on(engine.shutdown()).unwrap();
}

#[test]
fn sdk_applies_media_queries_for_each_page_viewport() {
    let engine = engine();
    let narrow = block_on(engine.create_page(PageOptions::new(320, 240))).unwrap();
    let wide = block_on(engine.create_page(PageOptions::new(1280, 720))).unwrap();
    let document = StaticDocument::new(
        "https://example.test/responsive",
        "<style>main{width:300px;height:120px;background-color:red}\
         @media (min-width:768px){main{background-color:blue}}</style>\
         <main>responsive</main>",
    )
    .unwrap();

    let narrow_frame = block_on(narrow.render(document.clone())).unwrap();
    let wide_frame = block_on(wide.render(document)).unwrap();
    let color_count =
        |rgba: &[u8], color: [u8; 4]| rgba.chunks_exact(4).filter(|pixel| *pixel == color).count();
    assert!(
        color_count(narrow_frame.frame().rgba(), [255, 0, 0, 255])
            > color_count(narrow_frame.frame().rgba(), [0, 0, 255, 255])
    );
    assert!(
        color_count(wide_frame.frame().rgba(), [0, 0, 255, 255])
            > color_count(wide_frame.frame().rgba(), [255, 0, 0, 255])
    );
    block_on(engine.shutdown()).unwrap();
}

#[test]
fn sdk_rejects_a_runtime_with_the_wrong_digest_before_launch() {
    let error = block_on(
        Engine::builder()
            .runtime_path(env!("CARGO_BIN_EXE_archetype-runtime"))
            .expected_runtime_sha256(&"00".repeat(32))
            .unwrap()
            .build(),
    )
    .err()
    .expect("digest mismatch should fail");
    assert!(matches!(error, SdkError::Integrity(_)));
}

#[test]
fn sdk_starts_a_runtime_with_the_expected_digest() {
    let runtime = env!("CARGO_BIN_EXE_archetype-runtime");
    let digest = format!("{:x}", Sha256::digest(fs::read(runtime).unwrap()));
    let engine = block_on(
        Engine::builder()
            .runtime_path(runtime)
            .expected_runtime_sha256(&digest)
            .unwrap()
            .build(),
    )
    .unwrap();
    block_on(engine.shutdown()).unwrap();
}

#[test]
fn one_hundred_sdk_runtime_restarts_remain_renderable() {
    for cycle in 0..100 {
        let engine = engine();
        let page = block_on(engine.create_page(PageOptions::new(320, 180))).unwrap();
        block_on(engine.force_restart_for_testing()).unwrap();
        let document = StaticDocument::new(
            "https://example.test/restart",
            format!("<title>Cycle {cycle}</title><p>recovered</p>"),
        )
        .unwrap();
        let navigation = block_on(page.render(document)).unwrap();
        assert_eq!(navigation.title(), format!("Cycle {cycle}"));
        block_on(engine.shutdown()).unwrap();
    }
}

#[test]
fn sdk_discards_an_older_concurrent_navigation() {
    let engine = engine();
    let page = block_on(engine.create_page(PageOptions::new(320, 180))).unwrap();
    let slow = StaticDocument::new(
        "https://example.test/slow",
        format!("<title>Old</title><p>{}</p>", "old ".repeat(250_000)),
    )
    .unwrap();
    let current = StaticDocument::new(
        "https://example.test/current",
        "<title>Current</title><p>newest navigation</p>",
    )
    .unwrap();

    let older = page.render(slow);
    let newest = page.render(current);

    assert!(matches!(block_on(older), Err(SdkError::StaleNavigation)));
    assert_eq!(block_on(newest).unwrap().title(), "Current");
    block_on(engine.shutdown()).unwrap();
}

#[test]
fn stopped_engine_reports_disconnect_to_its_page() {
    let engine = engine();
    let page = block_on(engine.create_page(PageOptions::new(320, 180))).unwrap();
    block_on(engine.shutdown()).unwrap();
    let document = StaticDocument::new("https://example.test/stopped", "<p>stopped</p>").unwrap();

    assert!(matches!(
        block_on(page.render(document)),
        Err(SdkError::Disconnected)
    ));
    assert!(matches!(
        block_on(page.next_event()).unwrap(),
        PageEvent::RuntimeDisconnected
    ));
}

#[test]
fn public_sdk_source_does_not_name_internal_ui_or_render_types() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../archetype-sdk/src");
    for file in ["lib.rs", "api.rs", "future.rs"] {
        let source = fs::read_to_string(root.join(file)).unwrap();
        for forbidden in ["gpui", "arch_dom", "arch_layout", "DisplayList", "Envelope"] {
            assert!(
                !source.contains(forbidden),
                "public SDK source {file} contains forbidden type {forbidden}"
            );
        }
    }
}
