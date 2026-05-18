use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use crate::worldline::types::{Worldline, WorldlineId};
use crate::TianJiError;

/// Storage abstraction used by simulation code when forking worldlines.
pub trait WorldlineStore {
    fn next_id(&self) -> Result<WorldlineId, TianJiError>;
    fn save(&self, worldline: &Worldline) -> Result<(), TianJiError>;
}

/// In-memory worldline store for tests and non-persistent simulations.
pub struct MemoryStore {
    counter: AtomicU64,
    worldlines: Mutex<Vec<Worldline>>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new(1)
    }
}

impl MemoryStore {
    pub fn new(start_id: WorldlineId) -> Self {
        Self {
            counter: AtomicU64::new(start_id),
            worldlines: Mutex::new(Vec::new()),
        }
    }

    pub fn saved_worldlines(&self) -> Result<Vec<Worldline>, TianJiError> {
        self.worldlines
            .lock()
            .map(|worldlines| worldlines.clone())
            .map_err(|_| TianJiError::DataIntegrity("memory worldline store poisoned".to_string()))
    }
}

impl WorldlineStore for MemoryStore {
    fn next_id(&self) -> Result<WorldlineId, TianJiError> {
        Ok(self.counter.fetch_add(1, Ordering::Relaxed))
    }

    fn save(&self, worldline: &Worldline) -> Result<(), TianJiError> {
        self.worldlines
            .lock()
            .map_err(|_| TianJiError::DataIntegrity("memory worldline store poisoned".to_string()))?
            .push(worldline.clone());
        Ok(())
    }
}

/// SQLite-backed worldline store using the existing storage module.
pub struct SqliteStore {
    conn: rusqlite::Connection,
}

impl SqliteStore {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn connection(&self) -> &rusqlite::Connection {
        &self.conn
    }
}

impl WorldlineStore for SqliteStore {
    fn next_id(&self) -> Result<WorldlineId, TianJiError> {
        crate::storage::next_worldline_id(&self.conn)
    }

    fn save(&self, worldline: &Worldline) -> Result<(), TianJiError> {
        crate::storage::save_worldline(&self.conn, worldline).map(|_| ())
    }
}
