use thiserror::Error;
use url::Url;

#[derive(Debug, Error)]
pub enum PatternError {
    #[error("invalid pattern: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchPattern {
    inner: Inner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Inner {
    AllUrls,
    Standard {
        scheme: SchemePat,
        host: HostPat,
        path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SchemePat {
    AnyHttp,
    Exact(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HostPat {
    Any,
    Exact(String),
    Suffix(String), // *.example.com -> suffix ".example.com" and also exact "example.com"
}

impl MatchPattern {
    pub fn parse(p: &str) -> Result<Self, PatternError> {
        if p == "<all_urls>" {
            return Ok(Self {
                inner: Inner::AllUrls,
            });
        }
        let rest = p;
        let (scheme, after_scheme) = rest
            .split_once("://")
            .ok_or_else(|| PatternError::Invalid(p.to_string()))?;
        let scheme_pat = if scheme == "*" {
            SchemePat::AnyHttp
        } else {
            SchemePat::Exact(scheme.to_ascii_lowercase())
        };
        let slash = after_scheme
            .find('/')
            .ok_or_else(|| PatternError::Invalid(p.to_string()))?;
        let host_part = &after_scheme[..slash];
        let path_part = &after_scheme[slash..]; // includes leading /
        let host_pat = if host_part == "*" {
            HostPat::Any
        } else if let Some(suf) = host_part.strip_prefix("*.") {
            HostPat::Suffix(suf.to_ascii_lowercase())
        } else {
            HostPat::Exact(host_part.to_ascii_lowercase())
        };
        Ok(Self {
            inner: Inner::Standard {
                scheme: scheme_pat,
                host: host_pat,
                path: path_part.to_string(),
            },
        })
    }

    pub fn matches(&self, url: &Url) -> bool {
        let scheme = url.scheme();
        if scheme == "wren" || scheme == "chrome-extension" {
            return false;
        }
        match &self.inner {
            Inner::AllUrls => matches!(
                scheme,
                "http" | "https" | "ws" | "wss" | "ftp" | "ftps" | "file"
            ),
            Inner::Standard {
                scheme: sp,
                host: hp,
                path,
            } => {
                let scheme_ok = match sp {
                    SchemePat::AnyHttp => scheme == "http" || scheme == "https",
                    SchemePat::Exact(s) => s == scheme,
                };
                if !scheme_ok {
                    return false;
                }
                let host_ok = match hp {
                    HostPat::Any => true,
                    HostPat::Exact(h) => url
                        .host_str()
                        .map(|x| x.eq_ignore_ascii_case(h))
                        .unwrap_or(false),
                    HostPat::Suffix(suf) => {
                        if let Some(h) = url.host_str() {
                            let h = h.to_ascii_lowercase();
                            h == *suf || h.ends_with(&format!(".{suf}"))
                        } else {
                            false
                        }
                    }
                };
                if !host_ok {
                    return false;
                }
                glob_match(path, url.path()) || glob_match(path, &full_path(url))
            }
        }
    }
}

fn full_path(url: &Url) -> String {
    let mut p = url.path().to_string();
    if let Some(q) = url.query() {
        p.push('?');
        p.push_str(q);
    }
    p
}

fn glob_match(pat: &str, text: &str) -> bool {
    // simple * glob over the whole string
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == text;
    }
    let mut pos = 0usize;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !text.starts_with(part) {
                return false;
            }
            pos = part.len();
        } else if i == parts.len() - 1 {
            if !text[pos..].ends_with(part) {
                return false;
            }
            pos = text.len();
        } else {
            if let Some(idx) = text[pos..].find(part) {
                pos += idx + part.len();
            } else {
                return false;
            }
        }
    }
    // if pattern ends with * anything ok, if not we checked end
    if !pat.ends_with('*') {
        // ensure we consumed exactly (last segment matched at end)
    }
    true
}
