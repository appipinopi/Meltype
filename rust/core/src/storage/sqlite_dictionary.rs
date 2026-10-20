// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! The dictionary is authoritative in SQLite; backup/import formats remain logical TXT/JSON.
use rusqlite::{params, Connection, OpenFlags};
use std::{collections::BTreeMap, io, path::Path, time::Duration};
const FILE: &str = "userdict.sqlite3";
const APPLICATION_ID: i64 = 0x4d545544;
fn error(error: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("{FILE}: {error}"))
}
fn version(connection: &Connection) -> io::Result<i64> {
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(error)?;
    let app: i64 = connection
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(error)?;
    if !(0..=1).contains(&version) || (app != APPLICATION_ID && !(version == 0 && app == 0)) {
        return Err(error("unsupported database schema/application"));
    }
    if version == 0 {
        let tables: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
                [],
                |row| row.get(0),
            )
            .map_err(error)?;
        if tables != 0 {
            return Err(error("unrecognized unversioned database"));
        }
    }
    Ok(version)
}
pub(super) fn read(directory: &Path) -> io::Result<Option<BTreeMap<String, Vec<String>>>> {
    let path = directory.join(FILE);
    if !path.try_exists()? {
        return Ok(None);
    }
    let connection =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    connection
        .busy_timeout(Duration::from_secs(5))
        .map_err(error)?;
    // Both SELECTs must observe the same SQLite snapshot during an edit.
    connection.execute_batch("BEGIN DEFERRED").map_err(error)?;
    if version(&connection)? == 0 {
        return Ok(None);
    }
    let mut statement=connection.prepare("SELECT reading, word FROM dictionary_entries ORDER BY reading COLLATE BINARY, position").map_err(error)?;
    let entries = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(error)?;
    let mut words: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut readings = connection
        .prepare("SELECT reading FROM dictionary_readings ORDER BY reading COLLATE BINARY")
        .map_err(error)?;
    for reading in readings
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(error)?
    {
        words.insert(reading.map_err(error)?, Vec::new());
    }
    for entry in entries {
        let (reading, word) = entry.map_err(error)?;
        words
            .get_mut(&reading)
            .ok_or_else(|| error("orphan dictionary entry"))?
            .push(word);
    }
    Ok(Some(words))
}
pub(super) struct Write {
    connection: Connection,
    committed: bool,
}
impl Drop for Write {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.connection.execute_batch("ROLLBACK");
        }
    }
}
impl Write {
    pub(super) fn prepare(
        directory: &Path,
        words: &BTreeMap<String, Vec<String>>,
    ) -> io::Result<Self> {
        let connection = Connection::open(directory.join(FILE)).map_err(error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(error)?;
        version(&connection)?;
        connection.execute_batch("BEGIN IMMEDIATE").map_err(error)?;
        let writer = Self {
            connection,
            committed: false,
        };
        writer.connection.execute_batch("CREATE TABLE IF NOT EXISTS dictionary_readings (reading TEXT PRIMARY KEY NOT NULL) WITHOUT ROWID; CREATE TABLE IF NOT EXISTS dictionary_entries (reading TEXT NOT NULL, position INTEGER NOT NULL CHECK(position>=0), word TEXT NOT NULL, PRIMARY KEY(reading,position)) WITHOUT ROWID; PRAGMA application_id=1297372484; PRAGMA user_version=1; DELETE FROM dictionary_entries; DELETE FROM dictionary_readings;").map_err(error)?;
        {
            let mut statement = writer
                .connection
                .prepare("INSERT INTO dictionary_entries(reading,position,word) VALUES(?1,?2,?3)")
                .map_err(error)?;
            for (reading, entries) in words {
                writer
                    .connection
                    .execute(
                        "INSERT INTO dictionary_readings(reading) VALUES(?1)",
                        [reading],
                    )
                    .map_err(error)?;
                for (position, word) in entries.iter().enumerate() {
                    statement
                        .execute(params![reading, position as i64, word])
                        .map_err(error)?;
                }
            }
        }
        Ok(writer)
    }
    pub(super) fn commit(mut self) -> io::Result<()> {
        self.connection.execute_batch("COMMIT").map_err(error)?;
        self.committed = true;
        Ok(())
    }
}
