use std::path::Path;

use anyhow::{Context, Result};

use crate::{CredStore, Credential};

pub fn import(path: &Path, store: &dyn CredStore) -> Result<usize> {
    let mut rdr = csv::Reader::from_path(path).context("open csv")?;
    let headers = rdr.headers().context("read csv headers")?.clone();
    let find_idx = |name: &str| headers.iter().position(|h| h.eq_ignore_ascii_case(name));
    let url_i = find_idx("url").context("missing url column")?;
    let user_i = find_idx("username").context("missing username column")?;
    let pass_i = find_idx("password").context("missing password column")?;
    let mut count = 0usize;
    for rec in rdr.records() {
        let rec = rec.context("read csv record")?;
        let url = rec.get(url_i).unwrap_or("").trim();
        let username = rec.get(user_i).unwrap_or("").to_string();
        let password = rec.get(pass_i).unwrap_or("").to_string();
        if url.is_empty() {
            continue;
        }
        let origin = normalize_origin(url);
        store.save(&Credential {
            origin,
            username,
            password,
        })?;
        count += 1;
    }
    Ok(count)
}

fn normalize_origin(url: &str) -> String {
    if let Ok(u) = url::Url::parse(url) {
        u.origin().ascii_serialization()
    } else {
        url.to_string()
    }
}
