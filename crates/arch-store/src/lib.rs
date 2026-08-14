use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, ErrorCode, OptionalExtension, params};
use thiserror::Error;
use uuid::Uuid;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS schema_migrations (
  version INTEGER PRIMARY KEY,
  applied_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS spaces (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  position INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS pages (
  id TEXT PRIMARY KEY,
  url TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  position INTEGER NOT NULL,
  last_visited_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS bookmarks (
  id TEXT PRIMARY KEY,
  space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE,
  parent_id TEXT REFERENCES bookmarks(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN ('bookmark', 'folder')),
  title TEXT NOT NULL,
  url TEXT,
  position INTEGER NOT NULL,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  CHECK((kind = 'folder' AND url IS NULL) OR (kind = 'bookmark' AND url IS NOT NULL))
);
CREATE TABLE IF NOT EXISTS app_state (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT OR IGNORE INTO schema_migrations(version, applied_at)
VALUES
  (1, CAST(unixepoch('subsec') * 1000 AS INTEGER)),
  (2, CAST(unixepoch('subsec') * 1000 AS INTEGER));
";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Space {
    pub id: String,
    pub name: String,
    pub position: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Page {
    pub id: String,
    pub url: String,
    pub title: String,
    pub position: i64,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("database recovery I/O error: {0}")]
    RecoveryIo(#[from] std::io::Error),
}

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Opens or creates a schema-v2 database.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot open, configure, or migrate the database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        match Self::open_once(path) {
            Ok(store) => Ok(store),
            Err(error) if is_corruption(&error) && path.exists() => {
                let backup = corrupt_backup_path(path);
                fs::rename(path, &backup)?;
                move_sidecar(path, &backup, "-wal")?;
                move_sidecar(path, &backup, "-shm")?;
                Self::open_once(path)
            }
            Err(error) => Err(error),
        }
    }

    fn open_once(path: &Path) -> Result<Self, StoreError> {
        let connection = Connection::open(path)?;
        Self::configure(&connection)?;
        connection.execute_batch(SCHEMA)?;
        Self::migrate_legacy_pages(&connection)?;
        Ok(Self { connection })
    }

    /// Creates an isolated schema-v2 database for tests and transient use.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot create, configure, or migrate the database.
    pub fn in_memory() -> Result<Self, StoreError> {
        let connection = Connection::open_in_memory()?;
        Self::configure(&connection)?;
        connection.execute_batch(SCHEMA)?;
        Self::migrate_legacy_pages(&connection)?;
        Ok(Self { connection })
    }

    fn configure(connection: &Connection) -> Result<(), rusqlite::Error> {
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        Ok(())
    }

    fn migrate_legacy_pages(connection: &Connection) -> Result<(), rusqlite::Error> {
        let has_space_id = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('pages') WHERE name = 'space_id')",
            [],
            |row| row.get::<_, bool>(0),
        )?;
        if !has_space_id {
            return Ok(());
        }

        connection.execute_batch(
            "BEGIN IMMEDIATE;
             CREATE TABLE pages_v2 (
               id TEXT PRIMARY KEY,
               url TEXT NOT NULL,
               title TEXT NOT NULL DEFAULT '',
               position INTEGER NOT NULL,
               last_visited_at INTEGER NOT NULL
             );
             INSERT INTO pages_v2(id, url, title, position, last_visited_at)
             SELECT p.id, p.url, p.title,
                    ROW_NUMBER() OVER (ORDER BY s.position, p.position, p.id) - 1,
                    p.last_visited_at
             FROM pages p
             JOIN spaces s ON s.id = p.space_id;
             DROP TABLE pages;
             ALTER TABLE pages_v2 RENAME TO pages;
             INSERT OR IGNORE INTO schema_migrations(version, applied_at)
             VALUES (2, CAST(unixepoch('subsec') * 1000 AS INTEGER));
             COMMIT;",
        )?;
        Ok(())
    }

    /// Inserts a Space at the end of the current ordering.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the transaction cannot be queried or committed.
    pub fn create_space(&mut self, name: &str) -> Result<Space, StoreError> {
        let transaction = self.connection.transaction()?;
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM spaces",
            [],
            |row| row.get(0),
        )?;
        let now = now_ms();
        let space = Space {
            id: Uuid::now_v7().to_string(),
            name: name.to_owned(),
            position,
        };
        transaction.execute(
            "INSERT INTO spaces(id, name, position, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
            params![space.id, space.name, space.position, now, now],
        )?;
        transaction.commit()?;
        Ok(space)
    }

    /// Renames an existing Space and reports whether a row changed.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot execute the update.
    pub fn rename_space(&self, id: &str, name: &str) -> Result<bool, StoreError> {
        Ok(self.connection.execute(
            "UPDATE spaces SET name = ?, updated_at = ? WHERE id = ?",
            params![name, now_ms(), id],
        )? == 1)
    }

    /// Deletes a Space, cascades its bookmarks, and compacts Space positions.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the transaction cannot be executed or committed.
    pub fn delete_space(&mut self, id: &str) -> Result<bool, StoreError> {
        let transaction = self.connection.transaction()?;
        let changed = transaction.execute("DELETE FROM spaces WHERE id = ?", [id])? == 1;
        transaction.execute(
            "WITH ranked AS (SELECT id, ROW_NUMBER() OVER (ORDER BY position, id) - 1 AS next FROM spaces) UPDATE spaces SET position = (SELECT next FROM ranked WHERE ranked.id = spaces.id)",
            [],
        )?;
        transaction.commit()?;
        Ok(changed)
    }

    /// Lists Spaces in their stable UI order.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot prepare or execute the query.
    pub fn spaces(&self) -> Result<Vec<Space>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, name, position FROM spaces ORDER BY position, id")?;
        Ok(statement
            .query_map([], |row| {
                Ok(Space {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    position: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    /// Inserts a page at the end of the global tab ordering.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the transaction fails.
    pub fn create_page(&mut self, url: &str) -> Result<Page, StoreError> {
        let transaction = self.connection.transaction()?;
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM pages",
            [],
            |row| row.get(0),
        )?;
        let page = Page {
            id: Uuid::now_v7().to_string(),
            url: url.to_owned(),
            title: String::new(),
            position,
        };
        transaction.execute(
            "INSERT INTO pages(id, url, title, position, last_visited_at) VALUES (?, ?, '', ?, ?)",
            params![page.id, page.url, page.position, now_ms()],
        )?;
        transaction.commit()?;
        Ok(page)
    }

    /// Stores the final URL and title for a completed page navigation.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot execute the update.
    pub fn update_page_navigation(
        &self,
        id: &str,
        url: &str,
        title: &str,
    ) -> Result<bool, StoreError> {
        Ok(self.connection.execute(
            "UPDATE pages SET url = ?, title = ?, last_visited_at = ? WHERE id = ?",
            params![url, title, now_ms(), id],
        )? == 1)
    }

    /// Deletes a page and compacts positions within its Space.
    ///
    /// # Errors
    /// Returns [`StoreError`] when the transaction cannot be executed or committed.
    pub fn delete_page(&mut self, id: &str) -> Result<bool, StoreError> {
        let transaction = self.connection.transaction()?;
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM pages WHERE id = ?)",
            [id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(false);
        }
        transaction.execute("DELETE FROM pages WHERE id = ?", [id])?;
        transaction.execute(
            "WITH ranked AS (SELECT id, ROW_NUMBER() OVER (ORDER BY position, id) - 1 AS next FROM pages) UPDATE pages SET position = (SELECT next FROM ranked WHERE ranked.id = pages.id)",
            [],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Lists global tabs in their stable UI order.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot prepare or execute the query.
    pub fn pages(&self) -> Result<Vec<Page>, StoreError> {
        let mut statement = self
            .connection
            .prepare("SELECT id, url, title, position FROM pages ORDER BY position, id")?;
        Ok(statement
            .query_map([], |row| {
                Ok(Page {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    title: row.get(2)?,
                    position: row.get(3)?,
                })
            })?
            .collect::<Result<_, _>>()?)
    }

    /// Inserts or replaces a named application-state value.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot execute the write.
    pub fn set_state(&self, key: &str, value: &str) -> Result<(), StoreError> {
        self.connection.execute("INSERT INTO app_state(key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value", params![key, value])?;
        Ok(())
    }

    /// Reads a named application-state value.
    ///
    /// # Errors
    /// Returns [`StoreError`] when `SQLite` cannot execute the query.
    pub fn state(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .connection
            .query_row("SELECT value FROM app_state WHERE key = ?", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }
}

fn is_corruption(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::Database(rusqlite::Error::SqliteFailure(error, _))
            if matches!(error.code, ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}

fn corrupt_backup_path(path: &Path) -> PathBuf {
    let mut backup = path.as_os_str().to_owned();
    backup.push(format!(".corrupt-{}", Uuid::now_v7()));
    PathBuf::from(backup)
}

fn move_sidecar(path: &Path, backup: &Path, suffix: &str) -> Result<(), std::io::Error> {
    let mut source = path.as_os_str().to_owned();
    source.push(suffix);
    let source = PathBuf::from(source);
    if source.exists() {
        let mut target = backup.as_os_str().to_owned();
        target.push(suffix);
        fs::rename(source, PathBuf::from(target))?;
    }
    Ok(())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
    )
    .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn space_deletion_does_not_delete_global_tabs() {
        let mut store = Store::in_memory().unwrap();
        let space = store.create_space("Research").unwrap();
        store.create_page("file:///fixture.html").unwrap();
        assert_eq!(store.pages().unwrap().len(), 1);
        store.delete_space(&space.id).unwrap();
        assert_eq!(store.pages().unwrap().len(), 1);
    }

    #[test]
    fn saves_selected_state() {
        let store = Store::in_memory().unwrap();
        store.set_state("selected_space_id", "space-1").unwrap();
        assert_eq!(
            store.state("selected_space_id").unwrap().as_deref(),
            Some("space-1")
        );
    }

    #[test]
    fn renames_space_persistently() {
        let mut store = Store::in_memory().unwrap();
        let space = store.create_space("Initial").unwrap();
        assert!(store.rename_space(&space.id, "Renamed").unwrap());
        assert_eq!(store.spaces().unwrap()[0].name, "Renamed");
        assert!(!store.rename_space("missing", "No change").unwrap());
    }

    #[test]
    fn preserves_corrupt_database_and_creates_replacement() {
        let directory = std::env::temp_dir().join(format!("archetype-store-{}", Uuid::now_v7()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("profile.db");
        let corrupt_bytes = b"this is not a SQLite database";
        fs::write(&path, corrupt_bytes).unwrap();

        let mut store = Store::open(&path).unwrap();
        store.create_space("Recovered").unwrap();
        drop(store);
        assert_eq!(
            Store::open(&path).unwrap().spaces().unwrap()[0].name,
            "Recovered"
        );

        let backup = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|entry| {
                entry
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("profile.db.corrupt-"))
            })
            .expect("corrupt database backup");
        assert_eq!(fs::read(backup).unwrap(), corrupt_bytes);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn deleting_page_compacts_positions() {
        let mut store = Store::in_memory().unwrap();
        let first = store.create_page("file:///one.html").unwrap();
        store.create_page("file:///two.html").unwrap();
        assert!(store.delete_page(&first.id).unwrap());
        let pages = store.pages().unwrap();
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].position, 0);
    }

    #[test]
    fn migrates_space_pages_to_global_tabs_in_stable_order() {
        let directory = std::env::temp_dir().join(format!("archetype-store-{}", Uuid::now_v7()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("profile.db");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at INTEGER NOT NULL);
                 CREATE TABLE spaces (id TEXT PRIMARY KEY, name TEXT NOT NULL, position INTEGER NOT NULL, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL);
                 CREATE TABLE pages (id TEXT PRIMARY KEY, space_id TEXT NOT NULL REFERENCES spaces(id) ON DELETE CASCADE, url TEXT NOT NULL, title TEXT NOT NULL DEFAULT '', position INTEGER NOT NULL, last_visited_at INTEGER NOT NULL);
                 CREATE TABLE app_state (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                 INSERT INTO spaces VALUES ('later', 'Later', 1, 0, 0), ('first', 'First', 0, 0, 0);
                 INSERT INTO pages VALUES ('b', 'later', 'https://b.example', 'B', 0, 0), ('a', 'first', 'https://a.example', 'A', 0, 0);",
            )
            .unwrap();
        drop(connection);

        let mut store = Store::open(&path).unwrap();
        let pages = store.pages().unwrap();
        assert_eq!(
            pages
                .iter()
                .map(|page| page.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        store.delete_space("first").unwrap();
        assert_eq!(store.pages().unwrap().len(), 2);

        drop(store);
        fs::remove_dir_all(directory).unwrap();
    }
}
