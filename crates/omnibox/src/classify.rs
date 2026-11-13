use std::net::IpAddr;

use psl;
use url::Url;

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Navigate(Url),
    Search(String),
}

const KNOWN_SCHEMES: &[&str] = &[
    "http",
    "https",
    "file",
    "about",
    "ftp",
    "ftps",
    "ws",
    "wss",
    "data",
    "javascript",
    "mailto",
    "tel",
];

pub fn classify(text: &str) -> Input {
    let t = text.trim();
    if t.is_empty() {
        return Input::Search(String::new());
    }
    // Scheme-present input.
    if let Ok(url) = Url::parse(t) {
        let scheme = url.scheme();
        if KNOWN_SCHEMES.contains(&scheme) || url.has_host() {
            // Reject things like "localhost:3000" where url has no host and scheme unknown.
            if url.has_host() || KNOWN_SCHEMES.contains(&scheme) {
                return Input::Navigate(url);
            }
        }
    }
    // Whitespace means a search query.
    if t.contains(char::is_whitespace) {
        return Input::Search(t.to_string());
    }
    if let Some(nav) = classify_host(t) {
        return nav;
    }
    Input::Search(t.to_string())
}

fn classify_host(t: &str) -> Option<Input> {
    let (hp, rest) = match t.find(['/', '?', '#']) {
        Some(i) => (&t[..i], &t[i..]),
        None => (t, ""),
    };
    let (host, port_suffix) = if hp.starts_with('[') {
        let close = hp.find(']')?;
        let host = &hp[..=close];
        let after = &hp[close + 1..];
        (host, after)
    } else if let Some(colon) = hp.find(':') {
        (&hp[..colon], &hp[colon..])
    } else {
        (hp, "")
    };
    if host.is_empty() {
        return None;
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    let scheme = if is_ip(bare)
        || bare.eq_ignore_ascii_case("localhost")
        || bare.to_ascii_lowercase().ends_with(".local")
    {
        "http"
    } else {
        let known = psl::suffix(bare.as_bytes())
            .map(|s| s.is_known())
            .unwrap_or(false);
        if known {
            "https"
        } else {
            return None;
        }
    };
    let url_str = format!("{}://{}{}{}", scheme, host, port_suffix, rest);
    Url::parse(&url_str).ok().map(Input::Navigate)
}

fn is_ip(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

pub fn search_url(template: &str, query: &str) -> Url {
    let encoded = percent_encode(query);
    let s = template.replace("{query}", &encoded);
    Url::parse(&s).unwrap()
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}
