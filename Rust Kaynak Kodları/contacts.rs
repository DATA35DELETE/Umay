use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const CONTACTS_FILE: &str = "contacts.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ContactBook {
    contacts: HashMap<String, String>, // nickname -> multiaddr
}

impl ContactBook {
    /// Load contacts from file, or create empty if doesn't exist
    pub fn load() -> Result<Self> {
        if Path::new(CONTACTS_FILE).exists() {
            let data = fs::read_to_string(CONTACTS_FILE)?;
            let book: ContactBook = serde_json::from_str(&data)?;
            Ok(book)
        } else {
            Ok(ContactBook::default())
        }
    }

    /// Save contacts to file
    pub fn save(&self) -> Result<()> {
        let data = serde_json::to_string_pretty(self)?;
        fs::write(CONTACTS_FILE, data)?;
        Ok(())
    }

    /// Add or update a contact
    pub fn add(&mut self, name: String, address: String) {
        self.contacts.insert(name, address);
    }

    /// Get a contact's address
    pub fn get(&self, name: &str) -> Option<&String> {
        self.contacts.get(name)
    }

    /// Remove a contact
    pub fn remove(&mut self, name: &str) -> bool {
        self.contacts.remove(name).is_some()
    }

    /// List all contacts
    pub fn list(&self) -> Vec<(&String, &String)> {
        self.contacts.iter().collect()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.contacts.is_empty()
    }
}
