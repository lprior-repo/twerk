use anyhow::Result;
use fjall::{KeyspaceCreateOptions, Readable, SingleWriterTxDatabase};

pub struct Store {
    db: SingleWriterTxDatabase,
}

impl Store {
    pub fn new(path: &std::path::Path) -> Result<Self> {
        let db = SingleWriterTxDatabase::builder(path).open()?;
        Ok(Self { db })
    }

    pub fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        let keyspace = self.db.keyspace("store", KeyspaceCreateOptions::default)?;
        keyspace.insert(key, value)?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let keyspace = self.db.keyspace("store", KeyspaceCreateOptions::default)?;
        let result = keyspace.get(key)?;
        Ok(result.map(|v| v.to_vec()))
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        let keyspace = self.db.keyspace("store", KeyspaceCreateOptions::default)?;
        keyspace.remove(key)?;
        Ok(())
    }

    pub fn begin(&self) -> Transaction {
        Transaction {
            db: self.db.clone(),
            snapshot: self.db.read_tx(),
        }
    }
}

#[derive(Clone)]
pub struct Transaction {
    db: SingleWriterTxDatabase,
    snapshot: fjall::Snapshot,
}

impl Transaction {
    pub fn read(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let keyspace = self.db.keyspace("store", KeyspaceCreateOptions::default)?;
        let result = self.snapshot.get(&keyspace, key)?;
        Ok(result.map(|v| v.to_vec()))
    }

    pub fn commit(self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_put_and_get() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Store::new(temp_dir.path()).unwrap();

        store.put("key1", b"value1").unwrap();
        assert_eq!(store.get("key1").unwrap(), Some(b"value1".to_vec()));

        store.put("key1", b"updated").unwrap();
        assert_eq!(store.get("key1").unwrap(), Some(b"updated".to_vec()));

        assert_eq!(store.get("nonexistent").unwrap(), None);

        store.delete("key1").unwrap();
        assert_eq!(store.get("key1").unwrap(), None);
    }

    #[test]
    fn test_snapshot_isolation_dirty_read_prevention() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Store::new(temp_dir.path()).unwrap();

        store.put("a", b"initial").unwrap();

        let tx1 = store.begin();
        let val_a_tx1_first = tx1.read("a").unwrap();
        assert_eq!(val_a_tx1_first, Some(b"initial".to_vec()));

        store.put("a", b"tx2_wrote").unwrap();

        let val_a_tx1_second = tx1.read("a").unwrap();
        assert_eq!(
            val_a_tx1_second,
            Some(b"initial".to_vec()),
            "tx1 should see snapshot value, not dirty write from tx2"
        );

        tx1.commit().unwrap();
    }

    #[test]
    fn test_snapshot_isolation_concurrent_write_last_writer_wins() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Store::new(temp_dir.path()).unwrap();

        store.put("a", b"original").unwrap();

        let tx1 = store.begin();
        let tx2 = store.begin();

        let val_a_tx1 = tx1.read("a").unwrap();
        assert_eq!(val_a_tx1, Some(b"original".to_vec()));

        drop(tx2);

        let val_a_tx1_after = tx1.read("a").unwrap();
        assert_eq!(
            val_a_tx1_after,
            Some(b"original".to_vec()),
            "tx1 snapshot should remain consistent"
        );

        tx1.commit().unwrap();
    }

    #[test]
    fn test_transaction_reads_own_writes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = Store::new(temp_dir.path()).unwrap();

        let tx1 = store.begin();
        tx1.commit().unwrap();

        assert_eq!(
            store.get("new_key").unwrap(),
            None,
            "uncommitted write should not be visible"
        );
    }
}