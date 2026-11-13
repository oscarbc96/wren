use std::collections::BTreeSet;

use idna;
use unicode_script::{Script, UnicodeScript};
use url::Url;

pub fn display_host(url: &Url) -> String {
    let host = match url.host_str() {
        Some(h) => h,
        None => return String::new(),
    };
    let (unicode, _) = idna::domain_to_unicode(host);
    let mixed = unicode.split('.').any(|label| {
        let mut scripts = BTreeSet::new();
        for ch in label.chars() {
            let sc = ch.script();
            if sc == Script::Common || sc == Script::Inherited || sc == Script::Unknown {
                continue;
            }
            scripts.insert(sc.as_iso15924_tag()); // Script lacks Ord; use its ISO tag
        }
        scripts.len() > 1
    });
    if mixed { host.to_string() } else { unicode }
}
