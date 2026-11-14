pub mod chrome;
pub mod csv;
pub mod keychain;

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub origin: String,
    pub username: String,
    pub password: String,
}

pub trait CredStore {
    fn save(&self, c: &Credential) -> Result<()>;
    fn find(&self, origin: &str) -> Result<Vec<Credential>>;
    fn delete(&self, origin: &str, username: &str) -> Result<()>;
    fn list_origins(&self) -> Result<Vec<String>>;
}

#[derive(Debug, Default, Clone)]
pub struct MemoryCredStore {
    items: std::sync::Arc<std::sync::Mutex<Vec<Credential>>>,
}

impl CredStore for MemoryCredStore {
    fn save(&self, c: &Credential) -> Result<()> {
        let mut items = self.items.lock().unwrap();
        items.retain(|x| !(x.origin == c.origin && x.username == c.username));
        items.push(c.clone());
        Ok(())
    }

    fn find(&self, origin: &str) -> Result<Vec<Credential>> {
        let items = self.items.lock().unwrap();
        Ok(items
            .iter()
            .filter(|x| x.origin == origin)
            .cloned()
            .collect())
    }

    fn delete(&self, origin: &str, username: &str) -> Result<()> {
        let mut items = self.items.lock().unwrap();
        items.retain(|x| !(x.origin == origin && x.username == username));
        Ok(())
    }

    fn list_origins(&self) -> Result<Vec<String>> {
        let items = self.items.lock().unwrap();
        let mut origins: Vec<String> = items.iter().map(|x| x.origin.clone()).collect();
        origins.sort();
        origins.dedup();
        Ok(origins)
    }
}

pub fn fill_script(username: &str, password: &str) -> String {
    let user = serde_json::to_string(username).unwrap_or_else(|_| "\"\"".to_string());
    let pass = serde_json::to_string(password).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(function(){{var u={user};var p={pass};var pw=document.querySelector(\"input[type='password']\");if(!pw)return;var inputs=Array.from(document.querySelectorAll('input'));var idx=inputs.indexOf(pw);var userField=null;for(var i=idx-1;i>=0;i--){{var t=(inputs[i].type||'').toLowerCase();if(t==='text'||t==='email'||t===''){{userField=inputs[i];break;}}}}if(userField){{userField.focus();userField.value=u;userField.dispatchEvent(new Event('input',{{bubbles:true}}));userField.dispatchEvent(new Event('change',{{bubbles:true}}));}}pw.focus();pw.value=p;pw.dispatchEvent(new Event('input',{{bubbles:true}}));pw.dispatchEvent(new Event('change',{{bubbles:true}}));}})();"
    )
}

pub use keychain::KeychainStore;
