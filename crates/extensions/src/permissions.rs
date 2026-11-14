use crate::manifest::Manifest;

pub fn summarize(manifest: &Manifest) -> Vec<String> {
    let mut out = Vec::new();
    let mut all_sites = false;
    for p in manifest
        .permissions
        .iter()
        .chain(manifest.host_permissions.iter())
    {
        if p == "<all_urls>" || p == "*://*/*" || p == "http://*/*" || p == "https://*/*" {
            all_sites = true;
        }
    }
    if all_sites {
        out.push("Read and change all your data on all websites".to_string());
    } else {
        for p in manifest.host_permissions.iter() {
            out.push(format!("Read and change your data on {p}"));
        }
    }
    for p in &manifest.permissions {
        let line = match p.as_str() {
            "storage" => "Store data on your device".to_string(),
            "tabs" => "Read your browsing activity".to_string(),
            "scripting" => "Run scripts on pages you visit".to_string(),
            "alarms" => "Run in the background on a schedule".to_string(),
            "notifications" => "Show notifications".to_string(),
            "contextMenus" => "Add items to the context menu".to_string(),
            "declarativeNetRequest" => "Block or modify network requests".to_string(),
            "<all_urls>" | "*://*/*" | "http://*/*" | "https://*/*" => continue,
            _ => continue,
        };
        if !out.contains(&line) {
            out.push(line);
        }
    }
    out
}
