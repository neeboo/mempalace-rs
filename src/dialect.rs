use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct CompressionMetadata {
    pub source_file: Option<String>,
    pub wing: Option<String>,
    pub room: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompressionStats {
    pub original_tokens: usize,
    pub compressed_tokens: usize,
    pub ratio: f64,
    pub original_chars: usize,
    pub compressed_chars: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Dialect {
    entity_codes: HashMap<String, String>,
    skip_names: Vec<String>,
}

impl Dialect {
    pub fn new(entities: HashMap<String, String>, skip_names: Vec<String>) -> Self {
        let mut entity_codes = HashMap::new();
        for (name, code) in entities {
            entity_codes.insert(name.clone(), code.clone());
            entity_codes.insert(name.to_ascii_lowercase(), code);
        }
        Self {
            entity_codes,
            skip_names: skip_names
                .into_iter()
                .map(|name| name.to_ascii_lowercase())
                .collect(),
        }
    }

    pub fn compress(&self, text: &str, metadata: Option<&CompressionMetadata>) -> String {
        let metadata = metadata.cloned().unwrap_or_default();
        let entities = self.detect_entities(text);
        let entity_str = if entities.is_empty() {
            "???".to_string()
        } else {
            entities.join("+")
        };
        let topics = self.extract_topics(text);
        let topic_str = if topics.is_empty() {
            "misc".to_string()
        } else {
            topics.join("_")
        };
        let quote = self.extract_key_sentence(text);
        let quote_part = (!quote.is_empty() && quote.len() <= 24).then(|| format!("\"{quote}\""));
        let emotions = self.detect_emotions(text);
        let flags = self.detect_flags(text);

        let mut lines = Vec::new();
        if metadata.source_file.is_some() || metadata.wing.is_some() {
            let header = [
                metadata.wing.unwrap_or_else(|| "?".to_string()),
                metadata.room.unwrap_or_else(|| "?".to_string()),
                metadata.date.unwrap_or_else(|| "?".to_string()),
                metadata
                    .source_file
                    .as_deref()
                    .map(Path::new)
                    .and_then(|path| path.file_stem())
                    .and_then(|value| value.to_str())
                    .unwrap_or("?")
                    .to_string(),
            ]
            .join("|");
            lines.push(header);
        }

        let mut parts = vec![format!("0:{entity_str}"), topic_str];
        if let Some(quote_part) = quote_part {
            parts.push(quote_part);
        }
        if !emotions.is_empty() {
            parts.push(emotions.join("+"));
        }
        if !flags.is_empty() {
            parts.push(flags.join("+"));
        }
        lines.push(parts.join("|"));
        let compressed = lines.join("\n");
        if metadata.source_file.is_none() && compressed.len() >= text.len() {
            let compact_entity = entities.first().cloned().unwrap_or_else(|| "???".to_string());
            let compact_topic = topics.first().cloned().unwrap_or_else(|| "misc".to_string());
            let compact_flag = flags.first().cloned().unwrap_or_else(|| "NOTE".to_string());
            format!("0:{compact_entity}|{compact_topic}|{compact_flag}")
        } else {
            compressed
        }
    }

    pub fn compression_stats(&self, original_text: &str, compressed: &str) -> CompressionStats {
        let original_tokens = Self::count_tokens(original_text);
        let compressed_tokens = Self::count_tokens(compressed);
        CompressionStats {
            original_tokens,
            compressed_tokens,
            ratio: original_tokens as f64 / compressed_tokens.max(1) as f64,
            original_chars: original_text.len(),
            compressed_chars: compressed.len(),
        }
    }

    pub fn count_tokens(text: &str) -> usize {
        text.len() / 3
    }

    fn detect_entities(&self, text: &str) -> Vec<String> {
        let mut found = Vec::new();
        for (name, code) in &self.entity_codes {
            if name.chars().all(|char| char.is_lowercase()) {
                continue;
            }
            if text.to_ascii_lowercase().contains(&name.to_ascii_lowercase()) && !found.contains(code) {
                found.push(code.clone());
            }
        }
        if !found.is_empty() {
            return found.into_iter().take(3).collect();
        }

        let mut seen = HashSet::new();
        for (index, word) in text.split_whitespace().enumerate() {
            let clean = word
                .chars()
                .filter(|char| char.is_ascii_alphabetic())
                .collect::<String>();
            if clean.len() >= 2
                && clean.chars().next().is_some_and(|char| char.is_uppercase())
                && clean.chars().skip(1).all(|char| char.is_lowercase())
                && index > 0
                && !self.skip_names.iter().any(|name| clean.to_ascii_lowercase().contains(name))
            {
                let code = clean.chars().take(3).collect::<String>().to_ascii_uppercase();
                if seen.insert(code.clone()) {
                    found.push(code);
                }
                if found.len() >= 3 {
                    break;
                }
            }
        }
        found
    }

    fn detect_emotions(&self, text: &str) -> Vec<String> {
        emotion_signals()
            .into_iter()
            .filter_map(|(keyword, code)| {
                text.to_ascii_lowercase()
                    .contains(keyword)
                    .then(|| code.to_string())
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .take(3)
            .collect()
    }

    fn detect_flags(&self, text: &str) -> Vec<String> {
        flag_signals()
            .into_iter()
            .filter_map(|(keyword, flag)| {
                text.to_ascii_lowercase()
                    .contains(keyword)
                    .then(|| flag.to_string())
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .take(3)
            .collect()
    }

    fn extract_topics(&self, text: &str) -> Vec<String> {
        let mut counts = HashMap::<String, usize>::new();
        for raw in text.split(|char: char| !char.is_ascii_alphanumeric() && char != '_' && char != '-') {
            let word = raw.to_ascii_lowercase();
            if word.len() < 3 || stop_words().contains(&word.as_str()) {
                continue;
            }
            let mut score = 1usize;
            if raw.chars().next().is_some_and(|char| char.is_uppercase()) {
                score += 2;
            }
            if raw.contains('_') || raw.contains('-') || raw.chars().skip(1).any(|char| char.is_uppercase()) {
                score += 2;
            }
            *counts.entry(word).or_default() += score;
        }
        let mut ranked = counts.into_iter().collect::<Vec<_>>();
        ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        ranked.into_iter().take(3).map(|(word, _)| word).collect()
    }

    fn extract_key_sentence(&self, text: &str) -> String {
        let mut best = String::new();
        let mut best_score = i32::MIN;
        for sentence in text
            .split(['.', '!', '?', '\n'])
            .map(str::trim)
            .filter(|sentence| sentence.len() > 10)
        {
            let lowered = sentence.to_ascii_lowercase();
            let mut score = 0;
            for keyword in [
                "decided",
                "because",
                "instead",
                "prefer",
                "switched",
                "chose",
                "realized",
                "important",
                "solution",
                "insight",
            ] {
                if lowered.contains(keyword) {
                    score += 2;
                }
            }
            if sentence.len() < 80 {
                score += 1;
            }
            if sentence.len() > 150 {
                score -= 2;
            }
            if score > best_score {
                best_score = score;
                best = sentence.to_string();
            }
        }
        if best.len() > 24 {
            format!("{}...", &best[..21])
        } else {
            best
        }
    }
}

fn emotion_signals() -> [(&'static str, &'static str); 10] {
    [
        ("decided", "determ"),
        ("prefer", "convict"),
        ("worried", "anx"),
        ("excited", "excite"),
        ("frustrated", "frust"),
        ("confused", "confuse"),
        ("love", "love"),
        ("hope", "hope"),
        ("happy", "joy"),
        ("relieved", "relief"),
    ]
}

fn flag_signals() -> [(&'static str, &'static str); 14] {
    [
        ("decided", "DECISION"),
        ("chose", "DECISION"),
        ("switched", "DECISION"),
        ("migrated", "DECISION"),
        ("replaced", "DECISION"),
        ("because", "DECISION"),
        ("created", "ORIGIN"),
        ("started", "ORIGIN"),
        ("core", "CORE"),
        ("principle", "CORE"),
        ("turning point", "PIVOT"),
        ("api", "TECHNICAL"),
        ("database", "TECHNICAL"),
        ("architecture", "TECHNICAL"),
    ]
}

fn stop_words() -> &'static [&'static str] {
    &[
        "the", "and", "for", "with", "that", "this", "from", "have", "were", "what", "when",
        "where", "which", "your", "because", "about", "into", "they", "them", "then", "only",
        "also", "much", "many", "like", "used", "using", "make", "made", "thing", "need",
    ]
}
