use anyhow::{anyhow, Result};
use sled::{Db}; // Import Result and anyhow from the anyhow crate

pub struct StorageEngine {
    pub db: Db,
}

impl StorageEngine {
    // Open the storage engine without any specific column families
    pub fn new() -> Result<Self> {
        let dir = dirs::home_dir()
            .ok_or_else(|| anyhow!("Failed to get home directory"))?
            .join(".hydra");

        std::fs::create_dir_all(&dir)?;

        let dbpath = dir.join("sled");

        let db = sled::open(&dbpath)?;

        Ok(Self { db })
    }

    // Automatically creates a tree if it does not exist and returns a handle
    pub fn get_handle(&self, name: &str) -> Result<sled::Tree> {
        let tree = self.db.open_tree(name)?;
        Ok(tree)
    }
}
