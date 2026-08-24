use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

static LOGGER: OnceLock<LocalLogger> = OnceLock::new();

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Level {
    Info,
    Warn,
    Error,
}

#[derive(Serialize)]
struct Record<'a> {
    timestamp_ms: u128,
    session_id: &'a str,
    level: Level,
    event: &'a str,
    fields: BTreeMap<&'a str, Value>,
}

struct LocalLogger {
    file: Mutex<File>,
    session_id: String,
}

impl LocalLogger {
    fn open(path: &Path) -> io::Result<Self> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            file: Mutex::new(file),
            session_id: Uuid::now_v7().to_string(),
        })
    }

    fn event<const N: usize>(
        &self,
        level: Level,
        name: &'static str,
        fields: [(&'static str, Value); N],
    ) {
        self.event_map(level, name, fields.into_iter().collect());
    }

    fn event_map(&self, level: Level, name: &'static str, fields: BTreeMap<&'static str, Value>) {
        let record = Record {
            timestamp_ms: timestamp_ms(),
            session_id: &self.session_id,
            level,
            event: name,
            fields,
        };
        if let Ok(mut file) = self.file.lock() {
            let _ = write_record(&mut *file, &record);
        }
    }
}

pub(crate) fn init() -> io::Result<PathBuf> {
    init_in(&data_dir())
}

fn init_in(base: &Path) -> io::Result<PathBuf> {
    let directory = base.join("logs");
    fs::create_dir_all(&directory)?;
    let path = directory.join("archetype.jsonl");
    LOGGER
        .set(LocalLogger::open(&path)?)
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "logger already initialized"))?;
    Ok(path)
}

pub(crate) fn data_dir() -> PathBuf {
    resolve_data_dir(std::env::var_os("ARCHETYPE_DATA_DIR"))
}

fn resolve_data_dir(override_path: Option<OsString>) -> PathBuf {
    override_path.map_or_else(
        || {
            ProjectDirs::from("org", "Archetype", "Archetype")
                .map_or_else(std::env::temp_dir, |dirs| {
                    dirs.data_local_dir().to_path_buf()
                })
        },
        PathBuf::from,
    )
}

pub(crate) fn event<const N: usize>(
    level: Level,
    name: &'static str,
    fields: [(&'static str, Value); N],
) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    logger.event(level, name, fields);
}

fn event_map(level: Level, name: &'static str, fields: BTreeMap<&'static str, Value>) {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    logger.event_map(level, name, fields);
}

pub(crate) fn application_started(inspect_mode: bool) {
    event(
        Level::Info,
        "application_started",
        [(
            "mode",
            serde_json::json!(if inspect_mode { "inspect" } else { "desktop" }),
        )],
    );
}

pub(crate) fn application_failed(error: &str) {
    event(
        Level::Error,
        "application_failed",
        [("error", serde_json::json!(error))],
    );
}

pub(crate) fn inspection_completed(
    url: &str,
    title: &str,
    display_command_count: usize,
    diagnostic_count: usize,
) {
    event(
        Level::Info,
        "inspection_completed",
        [
            ("url", serde_json::json!(url)),
            ("title", serde_json::json!(title)),
            (
                "display_command_count",
                serde_json::json!(display_command_count),
            ),
            ("diagnostic_count", serde_json::json!(diagnostic_count)),
        ],
    );
}

pub(crate) fn render_diagnostic(page_id: Option<&str>, message: &str) {
    let mut fields = BTreeMap::from([("message", serde_json::json!(message))]);
    if let Some(page_id) = page_id {
        fields.insert("page_id", serde_json::json!(page_id));
    }
    event_map(Level::Warn, "render_diagnostic", fields);
}

pub(crate) fn profile_fallback(path: &Path, error: &str) {
    event(
        Level::Error,
        "profile_fallback",
        [
            ("path", serde_json::json!(path)),
            ("error", serde_json::json!(error)),
        ],
    );
}

pub(crate) fn navigation_started(page_id: &str, url: &str) {
    event(
        Level::Info,
        "navigation_started",
        [
            ("page_id", serde_json::json!(page_id)),
            ("url", serde_json::json!(url)),
        ],
    );
}

pub(crate) fn navigation_failed(page_id: &str, url: &str, error: &str) {
    event(
        Level::Error,
        "navigation_failed",
        [
            ("page_id", serde_json::json!(page_id)),
            ("url", serde_json::json!(url)),
            ("error", serde_json::json!(error)),
        ],
    );
}

pub(crate) fn history_navigation_failed(page_id: &str, direction: &str, error: &str) {
    event(
        Level::Error,
        "history_navigation_failed",
        [
            ("page_id", serde_json::json!(page_id)),
            ("direction", serde_json::json!(direction)),
            ("error", serde_json::json!(error)),
        ],
    );
}

pub(crate) fn navigation_completed(
    page_id: &str,
    url: &str,
    title: &str,
    display_command_count: usize,
    diagnostic_count: usize,
) {
    event(
        Level::Info,
        "navigation_completed",
        [
            ("page_id", serde_json::json!(page_id)),
            ("url", serde_json::json!(url)),
            ("title", serde_json::json!(title)),
            (
                "display_command_count",
                serde_json::json!(display_command_count),
            ),
            ("diagnostic_count", serde_json::json!(diagnostic_count)),
        ],
    );
}

fn timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis())
}

fn write_record(writer: &mut impl Write, record: &Record<'_>) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, record).map_err(io::Error::other)?;
    writer.write_all(b"\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_one_structured_json_record_per_line() {
        let record = Record {
            timestamp_ms: 1_700_000_000_123,
            session_id: "session-1",
            level: Level::Warn,
            event: "render_diagnostic",
            fields: BTreeMap::from([
                ("diagnostic_count", serde_json::json!(2)),
                ("url", serde_json::json!("file:///fixture.html")),
            ]),
        };
        let mut output = Vec::new();
        write_record(&mut output, &record).unwrap();
        assert_eq!(output.last(), Some(&b'\n'));
        let value: Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(value["timestamp_ms"], 1_700_000_000_123_u64);
        assert_eq!(value["session_id"], "session-1");
        assert_eq!(value["level"], "warn");
        assert_eq!(value["event"], "render_diagnostic");
        assert_eq!(value["fields"]["diagnostic_count"], 2);
    }

    #[test]
    fn persists_all_runtime_events_as_json_lines() {
        let directory = std::env::temp_dir().join(format!("archetype-logging-{}", Uuid::now_v7()));
        let path = init_in(&directory).unwrap();

        application_started(true);
        application_failed("startup failed");
        inspection_completed("file:///fixture.html", "Fixture", 4, 1);
        render_diagnostic(None, "unsupported declaration");
        render_diagnostic(Some("page-1"), "unsupported property");
        profile_fallback(Path::new("profile.db"), "invalid database");
        navigation_started("page-1", "https://example.com");
        navigation_failed("page-1", "https://example.com", "timed out");
        history_navigation_failed("page-1", "back", "missing history entry");
        navigation_completed("page-1", "https://example.com", "Example", 8, 0);

        let records = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(records.len(), 10);
        assert_eq!(records[0]["event"], "application_started");
        assert_eq!(records[0]["fields"]["mode"], "inspect");
        assert_eq!(records[4]["fields"]["page_id"], "page-1");
        assert_eq!(records[9]["event"], "navigation_completed");
        assert!(records.iter().all(|record| record["timestamp_ms"].is_u64()));
        assert!(
            records
                .windows(2)
                .all(|pair| pair[0]["session_id"] == pair[1]["session_id"])
        );

        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn resolves_explicit_data_directory() {
        assert_eq!(
            resolve_data_dir(Some(OsString::from("/tmp/archetype-data"))),
            PathBuf::from("/tmp/archetype-data")
        );
        assert!(resolve_data_dir(None).is_absolute());
    }
}
