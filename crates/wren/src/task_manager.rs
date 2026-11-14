use engine::{Engine, ProcessInfo, WebViewId};
use gpui::actions;
use tabs::TabStrip;

actions!(task_manager, [Toggle]);

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub id: Option<WebViewId>,
    pub pid: u32,
    pub title: String,
    pub rss_bytes: u64,
    pub cpu_percent: f32,
}

pub fn rows(processes: Vec<ProcessInfo>, strip: &TabStrip) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::new();
    for p in processes {
        let title = match p.id {
            None => "Browser".to_string(),
            Some(wv) => strip
                .tabs()
                .iter()
                .find(|t| t.webview == wv)
                .map(|t| {
                    if t.title.is_empty() {
                        t.url.to_string()
                    } else {
                        t.title.clone()
                    }
                })
                .unwrap_or_else(|| "Tab".to_string()),
        };
        out.push(Row {
            id: p.id,
            pid: p.pid,
            title,
            rss_bytes: p.rss_bytes,
            cpu_percent: p.cpu_percent,
        });
    }
    out.sort_by_key(|r| std::cmp::Reverse(r.rss_bytes));
    out
}

pub fn end_process(engine: &mut dyn Engine, id: WebViewId) {
    engine.kill(id);
}
