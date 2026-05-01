use anyhow::Result;
use fjall::{Database, Keyspace, KeyspaceCreateOptions};

pub struct Store {
    keyspace: Keyspace,
}

impl Store {
    pub fn new(path: &std::path::Path) -> Result<Self> {
        let db = Database::builder(path).open()?;
        let keyspace = db.keyspace("store", KeyspaceCreateOptions::default)?;
        Ok(Self { keyspace })
    }

    pub fn put(&self, key: &str, value: &[u8]) -> Result<()> {
        self.keyspace.insert(key.as_bytes(), value)?;
        Ok(())
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let result = self.keyspace.get(key.as_bytes())?;
        Ok(result.map(|v| v.to_vec()))
    }

    pub fn delete(&self, key: &str) -> Result<()> {
        self.keyspace.remove(key.as_bytes())?;
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
}