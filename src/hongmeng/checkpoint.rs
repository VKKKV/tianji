use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rusqlite::params;
use serde::{Deserialize, Serialize};

use crate::worldline::types::{ActorId, Worldline};
use crate::TianJiError;

use super::agent::AgentStatus;
use super::board::BoardMessage;

/// A checkpoint of the simulation state, persisted to SQLite for crash recovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HongmengCheckpoint {
    pub simulation_id: String,
    pub tick: u64,
    pub worldline_snapshot: Worldline,
    pub agent_states: BTreeMap<ActorId, AgentStatus>,
    pub board_snapshot: Vec<BoardMessage>,
    pub created_at: DateTime<Utc>,
}

impl HongmengCheckpoint {
    /// Initialize the checkpoint table in the SQLite database.
    fn ensure_schema(conn: &rusqlite::Connection) -> Result<(), TianJiError> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS hongmeng_checkpoints (
                simulation_id TEXT NOT NULL,
                tick INTEGER NOT NULL,
                worldline_json TEXT NOT NULL,
                agent_states_json TEXT NOT NULL,
                board_snapshot_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                PRIMARY KEY (simulation_id, tick)
            )",
        )?;
        Ok(())
    }

    /// Save this checkpoint to the SQLite database.
    pub fn save(&self, conn: &rusqlite::Connection) -> Result<(), TianJiError> {
        Self::ensure_schema(conn)?;

        let worldline_json = serde_json::to_string(&self.worldline_snapshot)?;
        let agent_states_json = serde_json::to_string(&self.agent_states)?;
        let board_snapshot_json = serde_json::to_string(&self.board_snapshot)?;
        let created_at = self.created_at.to_rfc3339();

        let tick_i64 = i64::try_from(self.tick).unwrap_or(i64::MAX);

        conn.execute(
            "INSERT OR REPLACE INTO hongmeng_checkpoints
                (simulation_id, tick, worldline_json, agent_states_json, board_snapshot_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                self.simulation_id,
                tick_i64,
                worldline_json,
                agent_states_json,
                board_snapshot_json,
                created_at,
            ],
        )?;

        Ok(())
    }

    /// Load the latest checkpoint for a given simulation_id.
    /// Returns None if no checkpoint exists for that simulation.
    pub fn load(
        conn: &rusqlite::Connection,
        simulation_id: &str,
    ) -> Result<Option<Self>, TianJiError> {
        Self::ensure_schema(conn)?;

        let mut stmt = conn.prepare(
            "SELECT simulation_id, tick, worldline_json, agent_states_json, board_snapshot_json, created_at
             FROM hongmeng_checkpoints
             WHERE simulation_id = ?1
             ORDER BY tick DESC
             LIMIT 1",
        )?;

        let result = stmt.query_row(params![simulation_id], |row| {
            let simulation_id: String = row.get(0)?;
            let tick_i64: i64 = row.get(1)?;
            let tick = u64::try_from(tick_i64).unwrap_or(0);
            let worldline_json: String = row.get(2)?;
            let agent_states_json: String = row.get(3)?;
            let board_snapshot_json: String = row.get(4)?;
            let created_at_str: String = row.get(5)?;

            Ok((
                simulation_id,
                tick,
                worldline_json,
                agent_states_json,
                board_snapshot_json,
                created_at_str,
            ))
        });

        match result {
            Ok((sid, tick, wl_json, as_json, bs_json, ca_str)) => {
                let worldline_snapshot: Worldline = serde_json::from_str(&wl_json)?;
                let agent_states: BTreeMap<ActorId, AgentStatus> = serde_json::from_str(&as_json)?;
                let board_snapshot: Vec<BoardMessage> = serde_json::from_str(&bs_json)?;
                let created_at = DateTime::parse_from_rfc3339(&ca_str)
                    .map(|dt| dt.to_utc())
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(HongmengCheckpoint {
                    simulation_id: sid,
                    tick,
                    worldline_snapshot,
                    agent_states,
                    board_snapshot,
                    created_at,
                }))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(TianJiError::Storage(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hongmeng::agent::AgentStatus;
    use crate::hongmeng::board::{BoardMessage, MessageVisibility};
    use crate::worldline::types::{FieldKey, Worldline};
    use std::collections::BTreeSet;

    fn temp_sqlite_connection() -> rusqlite::Connection {
        rusqlite::Connection::open_in_memory().expect("in-memory sqlite")
    }

    fn sample_worldline() -> Worldline {
        let mut fields = BTreeMap::new();
        fields.insert(
            FieldKey {
                region: "global".to_string(),
                domain: "conflict".to_string(),
            },
            3.5,
        );
        let hash = Worldline::compute_snapshot_hash(&fields);

        Worldline {
            id: 1,
            fields,
            events: vec!["evt-1".to_string()],
            causal_graph: petgraph::graph::DiGraph::new(),
            active_actors: BTreeSet::from(["usa".to_string()]),
            divergence: 0.0,
            parent: None,
            diverge_tick: 0,
            snapshot_hash: hash,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn checkpoint_save_and_load_roundtrip() {
        let conn = temp_sqlite_connection();

        let worldline = sample_worldline();
        let mut agent_states = BTreeMap::new();
        agent_states.insert("usa".to_string(), AgentStatus::Thinking);
        agent_states.insert("china".to_string(), AgentStatus::Idle);

        let board = vec![BoardMessage {
            tick: 1,
            sender: "usa".to_string(),
            content: "Test message".to_string(),
            visibility: MessageVisibility::Public,
        }];

        let checkpoint = HongmengCheckpoint {
            simulation_id: "sim-001".to_string(),
            tick: 5,
            worldline_snapshot: worldline.clone(),
            agent_states: agent_states.clone(),
            board_snapshot: board.clone(),
            created_at: Utc::now(),
        };

        checkpoint.save(&conn).expect("save checkpoint");

        let loaded = HongmengCheckpoint::load(&conn, "sim-001")
            .expect("load checkpoint")
            .expect("checkpoint found");

        assert_eq!(loaded.simulation_id, "sim-001");
        assert_eq!(loaded.tick, 5);
        assert_eq!(loaded.agent_states.len(), 2);
        assert_eq!(loaded.agent_states["usa"], AgentStatus::Thinking);
        assert_eq!(loaded.board_snapshot.len(), 1);
        assert_eq!(loaded.board_snapshot[0].content, "Test message");
    }

    #[test]
    fn checkpoint_load_returns_none_for_unknown_simulation() {
        let conn = temp_sqlite_connection();

        let result = HongmengCheckpoint::load(&conn, "nonexistent");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn checkpoint_save_overwrites_on_same_simulation_and_tick() {
        let conn = temp_sqlite_connection();

        let worldline = sample_worldline();

        let checkpoint_v1 = HongmengCheckpoint {
            simulation_id: "sim-002".to_string(),
            tick: 3,
            worldline_snapshot: worldline.clone(),
            agent_states: BTreeMap::new(),
            board_snapshot: vec![],
            created_at: Utc::now(),
        };

        let mut agent_states_v2 = BTreeMap::new();
        agent_states_v2.insert("iran".to_string(), AgentStatus::Done);

        let checkpoint_v2 = HongmengCheckpoint {
            simulation_id: "sim-002".to_string(),
            tick: 3,
            worldline_snapshot: worldline,
            agent_states: agent_states_v2.clone(),
            board_snapshot: vec![],
            created_at: Utc::now(),
        };

        checkpoint_v1.save(&conn).expect("save v1");
        checkpoint_v2.save(&conn).expect("save v2 (overwrite)");

        let loaded = HongmengCheckpoint::load(&conn, "sim-002")
            .expect("load")
            .expect("found");

        assert_eq!(loaded.agent_states.len(), 1);
        assert_eq!(loaded.agent_states["iran"], AgentStatus::Done);
    }

    #[test]
    fn checkpoint_loads_latest_tick() {
        let conn = temp_sqlite_connection();

        let worldline = sample_worldline();

        for tick in 1..=5u64 {
            let checkpoint = HongmengCheckpoint {
                simulation_id: "sim-003".to_string(),
                tick,
                worldline_snapshot: worldline.clone(),
                agent_states: BTreeMap::new(),
                board_snapshot: vec![],
                created_at: Utc::now(),
            };
            checkpoint.save(&conn).expect("save checkpoint");
        }

        let loaded = HongmengCheckpoint::load(&conn, "sim-003")
            .expect("load")
            .expect("found");

        assert_eq!(loaded.tick, 5);
    }
}
