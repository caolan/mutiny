use rusqlite::{self, params, Connection, Result, Transaction, OptionalExtension};
use uuid::Uuid;

use crate::protocol::{Message, AppAnnouncement};

pub struct Store {
    db: Connection,
}

impl Store {
    pub fn new(db: Connection) -> Store {
        Store { db }
    }

    pub fn transaction<'a>(&'a mut self) -> Result<StoreTransaction<'a>> {
        Ok(StoreTransaction {
            tx: self.db.transaction()?,
        })
    }

    pub fn generate_app_uuid() -> String {
        let buffer = &mut Uuid::encode_buffer();
        return Uuid::new_v4().hyphenated().encode_lower(buffer).to_owned();
    }
}

pub struct StoreTransaction<'a> {
    tx: Transaction<'a>,
}

impl<'a> StoreTransaction<'a> {
    pub fn version(&self) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT user_version
             FROM pragma_user_version",
        )?;
        return stmt.query_row([], |row| row.get::<_, i64>(0));
    }

    // fn set_version(&self, version: i64) -> Result<()> {
    //     self.tx.pragma_update(None, "user_version", version)
    // }

    pub fn migrate(&self) -> Result<()> {
        loop {
            let version = self.version()?;
            match version {
                0 => {
                    // Initial app + messaging structure
                    println!("Migrating database to version 1");
                    self.tx.execute_batch(
                        "CREATE TABLE peer (
                             id INTEGER PRIMARY KEY,
                             peer_id TEXT UNIQUE NOT NULL
                         );
                         CREATE TABLE app (
                             id INTEGER PRIMARY KEY,
                             manifest_id TEXT UNIQUE NOT NULL
                         );
                         CREATE TABLE app_version (
                             id INTEGER PRIMARY KEY,
                             app_id INTEGER REFERENCES app(id) NOT NULL,
                             manifest_version TEXT NOT NULL,
                             UNIQUE(app_id, manifest_version)
                         );
                         CREATE TABLE app_instance (
                             id INTEGER PRIMARY KEY,
                             peer_id INTEGER REFERENCES peer(id) NOT NULL,
                             app_version_id INTEGER REFERENCES app_version(id) NOT NULL,
                             uuid TEXT NOT NULL,
                             UNIQUE(peer_id, uuid)
                         );
                         CREATE TABLE app_instance_label (
                             app_instance_id INTEGER PRIMARY KEY,
                             label TEXT UNIQUE NOT NULL
                         );
                         CREATE TABLE message_invite (
                             id INTEGER PRIMARY KEY,
                             received INTEGER NOT NULL,
                             app_instance_id REFERENCES app_instance(id) NOT NULL
                         );
                         CREATE TABLE message_allow (
                             id INTEGER PRIMARY KEY,
                             from_app_instance_id INTEGER REFERENCES app_instance(id) NOT NULL,
                             to_app_instance_id INTEGER REFERENCES app_instance(id) NOT NULL
                         );
                         CREATE TABLE message_data (
                             id INTEGER PRIMARY KEY AUTOINCREMENT,
                             data BLOB UNIQUE
                         );
                         CREATE TABLE message_inbox (
                             id INTEGER PRIMARY KEY AUTOINCREMENT,
                             received INTEGER NOT NULL,
                             from_app_instance_id INTEGER REFERENCES app_instance(id) NOT NULL,
                             to_app_instance_id INTEGER REFERENCES app_instance(id) NOT NULL,
                             message_id INTEGER REFERENCES mesage_data(id) NOT NULL
                         );
                         CREATE TABLE message_outbox (
                             id INTEGER PRIMARY KEY AUTOINCREMENT,
                             queued INTEGER NOT NULL,
                             from_app_instance_id INTEGER REFERENCES app_instance(id) NOT NULL,
                             to_app_instance_id INTEGER REFERENCES app_instance(id) NOT NULL,
                             message_id INTEGER REFERENCES mesage_data(id) NOT NULL
                         );
                         PRAGMA user_version = 1;"
                    )?;
                },
                1 => {
                    // Drop app manifests (name, version)
                    println!("Migrating database to version 2");
                    self.tx.execute_batch(
                        "ALTER TABLE app_instance DROP app_version_id;
                         DROP TABLE app_version;
                         DROP TABLE app;
                         ALTER TABLE app_instance RENAME TO app;
                         ALTER TABLE app_instance_label RENAME app_instance_id TO app_id;
                         ALTER TABLE app_instance_label RENAME TO app_label;
                         ALTER TABLE message_invite RENAME app_instance_id TO app_id;
                         ALTER TABLE message_allow RENAME from_app_instance_id TO from_app_id;
                         ALTER TABLE message_allow RENAME to_app_instance_id TO to_app_id;
                         ALTER TABLE message_inbox RENAME from_app_instance_id TO from_app_id;
                         ALTER TABLE message_inbox RENAME to_app_instance_id TO to_app_id;
                         ALTER TABLE message_outbox RENAME from_app_instance_id TO from_app_id;
                         ALTER TABLE message_outbox RENAME to_app_instance_id TO to_app_id;
                         PRAGMA user_version = 2;"
                    )?;
                },
                2 => {
                    // Announce API
                    println!("Migrating database to version 3");
                    self.tx.execute_batch(
                        "DROP TABLE message_allow;
                         DROP TABLE message_invite;
                         CREATE TABLE app_announcement (
                             app_id INTEGER PRIMARY KEY REFERENCES app(id),
                             received INTEGER NOT NULL,
                             data TEXT NOT NULL
                         );
                         PRAGMA user_version = 3;"
                    )?;
                },
                _ => break,
            }
        }
        Ok(())
    }

    pub fn get_app(&self, peer_id: i64, uuid: &str) -> Result<Option<i64>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT id
             FROM app
             WHERE peer_id = ?1 AND uuid = ?2",
        )?;
        stmt.query_row(params![peer_id, uuid], |row| row.get::<_, i64>(0)).optional()
    }

    pub fn get_app_uuid(&self, app_id: i64) -> Result<Option<String>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT uuid
             FROM app
             WHERE id = ?1",
        )?;
        stmt.query_row([app_id], |row| row.get::<_, String>(0)).optional()
    }

    pub fn put_app(&self, peer_id: i64, uuid: &str) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO app (peer_id, uuid)
             VALUES (?1, ?2)
             RETURNING id;",
        )?;
        stmt.query_row(params![peer_id, uuid], |row| {
            row.get::<_, i64>(0)
        })
    }

    pub fn get_or_put_app(
        &self,
        peer_id: i64,
        uuid: &str,
    ) -> Result<i64> {
        if let Some(id) = self.get_app(peer_id, uuid)? {
            return Ok(id);
        }
        self.put_app(peer_id, uuid)
    }

    pub fn get_app_by_label(&self, label: &str) -> Result<Option<i64>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT app_id
             FROM app_label
             WHERE label = ?1",
        )?;
        stmt.query_row([label], |row| row.get::<_, i64>(0)).optional()
    }

    pub fn put_app_label(&self, app_id: i64, label: &str) -> Result<()> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO app_label (app_id, label)
             VALUES (?1, ?2);",
        )?;
        stmt.execute(params![app_id, label])?;
        Ok(())
    }

    pub fn get_peer(&self, peer_id: &str) -> Result<Option<i64>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT id
             FROM peer
             WHERE peer_id = ?1",
        )?;
        stmt.query_row([peer_id], |row| row.get::<_, i64>(0)).optional()
    }

    pub fn put_peer(&self, peer_id: &str) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO peer (peer_id)
             VALUES (?1)
             RETURNING id",
        )?;
        stmt.query_row([peer_id], |row| row.get::<_, i64>(0))
    }

    pub fn get_or_put_peer(&self, peer_id: &str) -> Result<i64> {
        if let Some(id) = self.get_peer(peer_id)? {
            return Ok(id);
        }
        self.put_peer(peer_id)
    }

    pub fn get_message_data(&self, data: &[u8]) -> Result<Option<i64>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT id
             FROM message_data
             WHERE data = ?1",
        )?;
        stmt.query_row([data], |row| row.get::<_, i64>(0)).optional()
    }

    pub fn put_message_data(&self, data: &[u8]) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO message_data (data)
             VALUES (?1)
             RETURNING id",
        )?;
        stmt.query_row([data], |row| row.get::<_, i64>(0))
    }

    pub fn get_or_put_message_data(&self, data: &[u8]) -> Result<i64> {
        if let Some(id) = self.get_message_data(data)? {
            return Ok(id);
        }
        self.put_message_data(data)
    }

    pub fn prune_message_data(&self) -> Result<()> {
        // TODO: delete all messages without entry in inbox or outbox
        Ok(())
    }

    pub fn put_message_outbox(&self, queued: i64, from: i64, to: i64, message_id: i64) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO message_outbox (queued, from_app_id, to_app_id, message_id)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING id",
        )?;
        stmt.query_row([queued, from, to, message_id], |row| row.get::<_, i64>(0))
    }

    pub fn delete_message_outbox(&self, outbox_id: i64) -> Result<()> {
        let mut stmt = self.tx.prepare_cached(
            "DELETE FROM message_outbox
             WHERE id = ?1",
        )?;
        stmt.execute([outbox_id])?;
        self.prune_message_data()
    }

    pub fn list_app_announcements(&self) -> std::result::Result<Vec<AppAnnouncement>, Box<dyn std::error::Error>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT peer.peer_id, uuid, data
             FROM app_announcement
             JOIN app ON app.id = app_id
             JOIN peer ON peer.id = app.peer_id",
        )?;
        let mut rows = stmt.query([])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            let raw_data: String = row.get(2)?;
            let data = serde_json::from_str(&raw_data)?;
            results.push(AppAnnouncement {
                peer: row.get(0)?,
                app_uuid: row.get(1)?,
                data,
            });
        }
        return Ok(results);
    }

    // Note: serde_json::Value used for data argument to enforce valid JSON in db
    pub fn set_app_announcement(&self, app_id: i64, received: i64, data: &serde_json::Value) -> Result<()> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO app_announcement (app_id, received, data)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (app_id) DO UPDATE SET received=?2, data=?3",
        )?;
        stmt.execute(params![app_id, received, data.to_string()])?;
        Ok(())
    }

    pub fn put_message_inbox(&self, received: i64, from: i64, to: i64, message_id: i64) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO message_inbox (received, from_app_id, to_app_id, message_id)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING id",
        )?;
        stmt.query_row([received, from, to, message_id], |row| row.get::<_, i64>(0))
    }

    pub fn list_app_inbox_messages(&self, app_id: i64) -> Result<Vec<Message>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT message_inbox.id, peer.peer_id, app.uuid, data
             FROM message_inbox
             JOIN message_data ON message_data.id = message_id
             JOIN app ON app.id = from_app_id
             JOIN peer ON peer.id = app.peer_id
             WHERE to_app_id = ?1
             ORDER BY message_inbox.id ASC",
        )?;
        let mut rows = stmt.query([app_id])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(Message {
                id: row.get::<_, usize>(0)?,
                peer: row.get::<_, String>(1)?,
                uuid: row.get::<_, String>(2)?,
                message: row.get::<_, Vec<u8>>(3)?,
            });
        }
        return Ok(results);
    }

    pub fn delete_inbox_message(&self, to: i64, message_id: i64) -> Result<()> {
        let mut stmt = self.tx.prepare_cached(
            "DELETE FROM message_inbox
             WHERE to_app_id = ?1 AND id = ?2",
        )?;
        stmt.execute([to, message_id])?;
        Ok(())
    }

    pub fn commit(self) -> Result<()> {
        let tx = self.tx;
        tx.commit()
    }
}
