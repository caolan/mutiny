use rusqlite::{self, params, Connection, Result, Transaction, OptionalExtension};
use uuid::Uuid;

use crate::protocol::{Message, MessageInvite};

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

    pub fn generate_app_instance_uuid() -> String {
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
                    println!("Migrating database to version 2");
                    self.tx.execute_batch(
                        "DROP TABLE app;
                         DROP TABLE app_version;
                         DROP TABLE app_instance;
                         DROP TABLE message_invite;
                         DROP TABLE message_allow;
                         DROP TABLE message_data;
                         DROP TABLE message_inbox;
                         DROP TABLE message_outbox;
                         CREATE TABLE channel (
                             id INTEGER PRIMARY KEY,
                             protocol TEXT NOT NULL,
                             uuid TEXT NOT NULL,
                             UNIQUE(owner_id, uuid)
                         );
                         CREATE TABLE channel_member (
                             id INTEGER PRIMARY KEY,
                             channel_id INTEGER REFERENCES channel(id) NOT NULL,
                             peer_id INTEGER REFERENCES peer(id) NOT NULL,
                             UNIQUE(channel_id, peer_id)
                         );
                         CREATE TABLE channel_inbox (
                             id INTEGER PRIMARY KEY AUTOINCREMENT,
                             channel_id INTEGER REFERENCES channel(id) NOT NULL,
                             from INTEGER REFERENCES app(id) NOT NULL,
                             message BLOB NOT NULL
                         );
                         CREATE TABLE app (
                             id INTEGER PRIMARY KEY,
                             peer_id INTEGER REFERENCES peer(id) NOT NULL,
                             uuid TEXT NOT NULL,
                             UNIQUE(peer_id, uuid)
                         );
                         CREATE TABLE app_label (
                             app_id INTEGER PRIMARY KEY,
                             label TEXT UNIQUE NOT NULL
                         );
                         PRAGMA user_version = 2;"
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
            "INSERT INTO app (peer_id, app_version_id, uuid)
             VALUES (?1, ?2, ?3)
             RETURNING id;",
        )?;
        stmt.query_row(params![peer_id, app_version_id, uuid], |row| {
            row.get::<_, i64>(0)
        })
    }

    pub fn get_or_put_app(
        &self,
        peer_id: i64,
        app_version_id: i64,
        uuid: &str,
    ) -> Result<i64> {
        if let Some(id) = self.get_app(peer_id, uuid)? {
            return Ok(id);
        }
        self.put_app(peer_id, app_version_id, uuid)
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

    pub fn put_channel_inbox(&self, channel: i64, from: i64, message: &[u8]) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO channel_inbox (received, from_app_instance_id, to_app_instance_id, message_id)
             VALUES (?1, ?2, ?3, ?4)
             RETURNING id",
        )?;
        stmt.query_row([received, from, to, message_id], |row| row.get::<_, i64>(0))
    }

    pub fn delete_channel_inbox(&self, id: i64) -> Result<()> {
        let mut stmt = self.tx.prepare_cached(
            "DELETE FROM channel_inbox
             WHERE id = ?1",
        )?;
        stmt.execute([id])?;
    }

    pub fn get_channel_by_uuid(&self, uuid: &str) -> Result<Option<i64>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT id
             FROM channel
             WHERE uuid = ?1",
        )?;
        stmt.query_row([uuid], |row| row.get::<_, i64>(0)).optional()
    }

    pub fn list_channels(&self) -> Result<Vec<Channel>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT uuid, protocol
             FROM message_invite
             JOIN app_instance ON app_instance.id = app_instance_id
             JOIN app_version ON app_version.id = app_version_id
             JOIN app ON app.id = app_id
             JOIN peer ON peer.id = app_instance.peer_id",
        )?;
        let mut rows = stmt.query([])?;
        let mut results = Vec::new();
        while let Some(row) = rows.next()? {
            results.push(MessageInvite {
                peer: row.get(0)?,
                app_instance_uuid: row.get(1)?,
                manifest_id: row.get(2)?,
                manifest_version: row.get(3)?,
            });
        }
        return Ok(results);
    }

    pub fn put_message_invite(&self, received: i64, app_instance_id: i64) -> Result<i64> {
        let mut stmt = self.tx.prepare_cached(
            "INSERT INTO message_invite (received, app_instance_id)
             VALUES (?1, ?2)
             RETURNING id",
        )?;
        stmt.query_row([received, app_instance_id], |row| row.get::<_, i64>(0))
    }

    pub fn get_or_put_message_invite(&self, received: i64, app_instance_id: i64) -> Result<i64> {
        if let Some(id) = self.get_message_invite(app_instance_id)? {
            return Ok(id);
        }
        self.put_message_invite(received, app_instance_id)
    }

    pub fn get_app_id_and_version(&self, peer: &str, uuid: &str) -> Result<Option<(String, String)>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT manifest_id, manifest_version
             FROM app_instance
             JOIN app_version ON app_version.id = app_version_id
             JOIN app ON app.id = app_version.app_id
             JOIN peer ON peer.id = app_instance.peer_id
             WHERE peer.peer_id = ?1 AND uuid = ?2",
        )?;
        stmt.query_row(
            [&peer, &uuid],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        ).optional()
    }

    pub fn read_message(&self, app_id: i64) -> Result<Option<Message>> {
        let mut stmt = self.tx.prepare_cached(
            "SELECT peer.peer_id, app_instance.uuid, data
             FROM message_inbox
             JOIN message_data ON message_data.id = message_id
             JOIN app_instance ON app_instance.id = from_app_instance_id
             JOIN peer ON peer.id = app_instance.peer_id
             WHERE to_app_instance_id = ?1
             ORDER BY message_inbox.id ASC
             LIMIT 1",
        )?;
        stmt.query_row([app_id], |row| {
            Ok(Message {
                peer: row.get::<_, String>(0)?,
                uuid: row.get::<_, String>(1)?,
                message: row.get::<_, Vec<u8>>(2)?,
            })
        }).optional()
    }

    pub fn next_message(&self, app_id: i64) -> Result<()> {
        let mut stmt = self.tx.prepare_cached(
            "DELETE FROM message_inbox
             WHERE id IN (
                 SELECT min(id)
                 FROM message_inbox
                 WHERE to_app_instance_id = ?1
             )",
        )?;
        stmt.execute([app_id])?;
        Ok(())
    }

    pub fn commit(self) -> Result<()> {
        let tx = self.tx;
        tx.commit()
    }
}
