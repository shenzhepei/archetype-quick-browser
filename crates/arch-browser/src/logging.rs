use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::PathBuf,
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

pub(crate) fn init() -> io::Result<PathBuf> {
    let directory = data_dir().join("logs");
    fs::create_dir_all(&directory)?;
    let path = directory.join("archetype.jsonl");
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    LOGGER
        .set(LocalLogger {
            file: Mutex::new(file),
            session_id: Uuid::now_v7().to_string(),
        })
        .map_err(|_| io::Error::new(io::ErrorKind::AlreadyExists, "logger already initialized"))?;
    Ok(path)
}

pub(crate) fn data_dir() -> PathBuf {
    std::env::var_os("ARCHETYPE_DATA_DIR").map_or_else(
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
    let record = Record {
        timestamp_ms: timestamp_ms(),
        session_id: &logger.session_id,
        level,
        event: name,
        fields: fields.into_iter().collect(),
    };
    if let Ok(mut file) = logger.file.lock() {
        let _ = write_record(&mut *file, &record);
    }
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
}
