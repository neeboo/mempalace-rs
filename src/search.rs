use std::cmp::Reverse;
use std::path::Path;

use serde::Serialize;

use crate::storage::PalaceStore;
use crate::Result;

#[derive(Debug, Clone, Serialize)]
pub struct SearchHit {
    pub text: String,
    pub wing: String,
    pub room: String,
    pub source_file: String,
    pub score: usize,
}

pub fn search_memories(
    store: &PalaceStore,
    query: &str,
    wing: Option<&str>,
    room: Option<&str>,
    limit: usize,
) -> Result<Vec<SearchHit>> {
    let terms = query_terms(query);
    let mut hits = store
        .list_drawers(wing, room)?
        .into_iter()
        .filter_map(|drawer| {
            let score = score(&drawer.content, query, &terms);
            (score > 0).then(|| SearchHit {
                text: drawer.content,
                wing: drawer.wing,
                room: drawer.room,
                source_file: Path::new(&drawer.source_file)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("?")
                    .to_string(),
                score,
            })
        })
        .collect::<Vec<_>>();

    hits.sort_by_key(|hit| (Reverse(hit.score), hit.source_file.clone()));
    hits.truncate(limit.max(1));
    Ok(hits)
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_ascii_lowercase())
        .collect()
}

fn score(content: &str, query: &str, terms: &[String]) -> usize {
    let haystack = content.to_ascii_lowercase();
    let mut score = 0;
    if haystack.contains(&query.to_ascii_lowercase()) {
        score += 5;
    }
    for term in terms {
        if haystack.contains(term) {
            score += 2;
        }
    }
    score
}
