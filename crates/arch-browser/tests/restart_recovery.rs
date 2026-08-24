use std::{
    io::{BufRead as _, BufReader},
    path::Path,
    process::{Command, Stdio},
};

use arch_browser::BrowserCore;
use arch_store::BookmarkKind;
use uuid::Uuid;

#[test]
fn forced_exit_restores_spaces_bookmarks_tabs_and_selection() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let directory = std::env::temp_dir().join(format!("archetype-restart-{}", Uuid::now_v7()));
    std::fs::create_dir(&directory).unwrap();
    let profile = directory.join("profile.db");
    let first = root.join("fixtures/pages/01-document/index.html");
    let second = root.join("fixtures/pages/02-cascade/index.html");
    let mut child = Command::new(env!("CARGO_BIN_EXE_arch-profile-probe"))
        .args([&profile, &first, &second])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut ready = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut ready)
        .unwrap();
    let ids = ready.trim().split('\t').collect::<Vec<_>>();
    assert_eq!(ids.first(), Some(&"READY"));
    assert_eq!(ids.len(), 7);
    child.kill().unwrap();
    assert!(!child.wait().unwrap().success());

    let mut restored = BrowserCore::open(&profile).unwrap();
    let spaces = restored.spaces().unwrap();
    assert_eq!(
        spaces
            .iter()
            .map(|space| space.name.as_str())
            .collect::<Vec<_>>(),
        ["Work", "Personal"]
    );
    assert_eq!(spaces[0].id, ids[1]);
    assert_eq!(spaces[1].id, ids[2]);

    let root_bookmarks = restored.bookmarks(&spaces[0].id, None).unwrap();
    assert_eq!(root_bookmarks.len(), 1);
    assert_eq!(root_bookmarks[0].id, ids[3]);
    assert_eq!(root_bookmarks[0].kind, BookmarkKind::Folder);
    let children = restored
        .bookmarks(&spaces[0].id, Some(&root_bookmarks[0].id))
        .unwrap();
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].id, ids[4]);
    assert_eq!(children[0].kind, BookmarkKind::Bookmark);

    let pages = restored.pages().unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].id, ids[5]);
    assert_eq!(pages[0].title, "Archetype V3 Fixture");
    assert_eq!(pages[1].id, ids[6]);
    assert_eq!(pages[1].title, "Cascade fixture");
    assert_eq!(
        restored.selection().unwrap(),
        (Some(ids[2].to_owned()), Some(ids[6].to_owned()))
    );

    restored.delete_space(&spaces[0].id).unwrap();
    assert_eq!(restored.pages().unwrap().len(), 2);
    drop(restored);
    std::fs::remove_dir_all(directory).unwrap();
}
