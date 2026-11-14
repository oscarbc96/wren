use anyhow::{Context, Result};
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

use crate::{CredStore, Credential};

#[derive(Debug, Clone)]
pub struct KeychainStore {
    service: String,
}

impl KeychainStore {
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }

    fn account(origin: &str, username: &str) -> String {
        format!("{origin}\n{username}")
    }

    fn index_account(origin: &str) -> String {
        format!("__wren_index__\n{origin}")
    }

    fn load_index(&self, origin: &str) -> Result<Vec<String>> {
        match get_generic_password(&self.service, &Self::index_account(origin)) {
            Ok(bytes) => {
                if bytes.is_empty() {
                    return Ok(Vec::new());
                }
                let list: Vec<String> =
                    serde_json::from_slice(&bytes).context("parse keychain index")?;
                Ok(list)
            }
            Err(_) => Ok(Vec::new()),
        }
    }

    fn save_index(&self, origin: &str, usernames: &[String]) -> Result<()> {
        let account = Self::index_account(origin);
        let _ = delete_generic_password(&self.service, &account);
        if usernames.is_empty() {
            return Ok(());
        }
        let data = serde_json::to_vec(usernames).context("encode keychain index")?;
        set_generic_password(&self.service, &account, &data).context("save keychain index")?;
        Ok(())
    }
}

impl CredStore for KeychainStore {
    fn save(&self, c: &Credential) -> Result<()> {
        let account = Self::account(&c.origin, &c.username);
        let _ = delete_generic_password(&self.service, &account);
        set_generic_password(&self.service, &account, c.password.as_bytes())
            .context("set keychain password")?;
        let mut users = self.load_index(&c.origin)?;
        if !users.iter().any(|u| u == &c.username) {
            users.push(c.username.clone());
            users.sort();
        }
        self.save_index(&c.origin, &users)?;
        Ok(())
    }

    fn find(&self, origin: &str) -> Result<Vec<Credential>> {
        let users = self.load_index(origin)?;
        let mut out = Vec::new();
        for username in users {
            let account = Self::account(origin, &username);
            if let Ok(bytes) = get_generic_password(&self.service, &account) {
                let password = String::from_utf8(bytes).unwrap_or_default();
                out.push(Credential {
                    origin: origin.to_string(),
                    username,
                    password,
                });
            }
        }
        Ok(out)
    }

    fn delete(&self, origin: &str, username: &str) -> Result<()> {
        let account = Self::account(origin, username);
        let _ = delete_generic_password(&self.service, &account);
        let mut users = self.load_index(origin)?;
        users.retain(|u| u != username);
        self.save_index(origin, &users)?;
        Ok(())
    }

    fn list_origins(&self) -> Result<Vec<String>> {
        // Best-effort: we do not have a global index of origins. Return origins that
        // can be discovered is not directly supported; callers primarily use find.
        // To keep behavior reasonable, return empty when unknown.
        Ok(Vec::new())
    }
}
