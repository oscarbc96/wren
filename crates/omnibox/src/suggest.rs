use store::{Bookmark, HistoryHit, Store};
use url::Url;

use crate::classify::search_url;

#[derive(Debug, Clone, PartialEq)]
pub enum SuggestionKind {
    Bookmark,
    History,
    Search,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub kind: SuggestionKind,
    pub url: Url,
    pub title: String,
}

const DEFAULT_SEARCH_TEMPLATE: &str = "https://duckduckgo.com/?q={query}";

pub fn suggest(store: &dyn Store, text: &str, limit: usize) -> Vec<Suggestion> {
    let mut out: Vec<Suggestion> = Vec::new();
    if limit == 0 {
        return out;
    }
    // Reserve one slot for the trailing Search row.
    let content_limit = limit.saturating_sub(1);

    // Bookmarks first.
    if let Ok(bookmarks) = store.search_bookmarks(text, content_limit) {
        for b in bookmarks {
            if out.len() >= content_limit {
                break;
            }
            out.push(bookmark_to_suggestion(b));
        }
    }
    // Then history by visit score.
    if let Ok(history) = store.search_history(text, content_limit) {
        for h in history {
            if out.len() >= content_limit {
                break;
            }
            out.push(history_to_suggestion(h));
        }
    }
    // Then one Search row.
    if out.len() < limit {
        out.push(Suggestion {
            kind: SuggestionKind::Search,
            url: search_url(DEFAULT_SEARCH_TEMPLATE, text),
            title: text.to_string(),
        });
    }
    out
}

fn bookmark_to_suggestion(b: Bookmark) -> Suggestion {
    Suggestion {
        kind: SuggestionKind::Bookmark,
        url: b.url,
        title: b.title,
    }
}

fn history_to_suggestion(h: HistoryHit) -> Suggestion {
    Suggestion {
        kind: SuggestionKind::History,
        url: h.url,
        title: h.title,
    }
}
