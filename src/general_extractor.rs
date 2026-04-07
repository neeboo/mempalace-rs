use regex::Regex;

const DECISION_MARKERS: &[&str] = &[
    r"\blet'?s (?:use|go with|try|pick|choose|switch to)\b",
    r"\bwe (?:should|decided|chose|went with|picked|settled on)\b",
    r"\bi'?m going (?:to|with)\b",
    r"\bbecause\b",
    r"\btrade-?off\b",
    r"\barchitecture\b",
    r"\bapproach\b",
    r"\bstrategy\b",
    r"\bframework\b",
    r"\bconfigure\b",
    r"\bdefault\b",
];

const PREFERENCE_MARKERS: &[&str] = &[
    r"\bi prefer\b",
    r"\balways use\b",
    r"\bnever use\b",
    r"\bdon'?t (?:ever |like to )?(?:use|do|mock|stub|import)\b",
    r"\bi like (?:to|when|how)\b",
    r"\bi hate (?:when|how|it when)\b",
    r"\bplease (?:always|never|don'?t)\b",
    r"\bmy (?:rule|preference|style|convention) is\b",
    r"\bwe (?:always|never)\b",
    r"\buse\b.*\binstead of\b",
];

const MILESTONE_MARKERS: &[&str] = &[
    r"\bit works\b",
    r"\bit worked\b",
    r"\bnow it works\b",
    r"\bgot it working\b",
    r"\bfixed\b",
    r"\bsolved\b",
    r"\bbreakthrough\b",
    r"\bfigured (?:it )?out\b",
    r"\bfinally\b",
    r"\bdiscovered\b",
    r"\brealized\b",
    r"\bfound (?:out|that)\b",
    r"\bturns out\b",
    r"\bbuilt\b",
    r"\bcreated\b",
    r"\bimplemented\b",
    r"\bshipped\b",
    r"\blaunched\b",
    r"\bdeployed\b",
    r"\breleased\b",
    r"\bprototype\b",
    r"\bdemo\b",
    r"\d+x (?:compression|faster|slower|better|improvement|reduction)",
    r"\d+% (?:reduction|improvement|faster|better|smaller)",
];

const PROBLEM_MARKERS: &[&str] = &[
    r"\b(?:bug|error|crash|fail|broke|broken|issue|problem)\b",
    r"\bfailing\b",
    r"\bdoesn'?t work\b",
    r"\bnot working\b",
    r"\bwon'?t\b.*\bwork\b",
    r"\bkeeps? (?:failing|crashing|breaking|erroring)\b",
    r"\bkept (?:failing|crashing|breaking|erroring)\b",
    r"\broot cause\b",
    r"\bthe (?:problem|issue|bug) (?:is|was)\b",
    r"\bturns out\b.*\b(?:was|because|due to)\b",
    r"\bthe fix (?:is|was)\b",
    r"\bworkaround\b",
    r"\bthat'?s why\b",
    r"\bfixed (?:it |the |by )\b",
    r"\bsolution (?:is|was)\b",
    r"\bresolved\b",
    r"\bpatched\b",
];

const EMOTION_MARKERS: &[&str] = &[
    r"\blove\b",
    r"\bscared\b",
    r"\bafraid\b",
    r"\bproud\b",
    r"\bhurt\b",
    r"\bhappy\b",
    r"\bsad\b",
    r"\bcry(?:ing)?\b",
    r"\bmiss\b",
    r"\bsorry\b",
    r"\bgrateful\b",
    r"\bangry\b",
    r"\bworried\b",
    r"\blonely\b",
    r"\bbeautiful\b",
    r"\bamazing\b",
    r"\bwonderful\b",
    r"i feel",
    r"i'm scared",
    r"i love you",
    r"i'm sorry",
    r"i can't",
    r"i wish",
    r"i miss",
    r"i need",
];

const POSITIVE_WORDS: &[&str] = &[
    "pride", "proud", "joy", "happy", "love", "beautiful", "amazing", "wonderful", "excited",
    "grateful", "breakthrough", "success", "works", "working", "solved", "fixed", "nailed",
];

const NEGATIVE_WORDS: &[&str] = &[
    "bug", "error", "crash", "failed", "failing", "broken", "broke", "issue", "problem", "stuck",
    "blocked", "unable", "terrible", "panic", "disaster", "mess",
];

const CODE_LINE_PATTERNS: &[&str] = &[
    r"^\s*[\$#]\s",
    r"^\s*(?:cd|source|echo|export|pip|npm|git|python|bash|curl|wget|mkdir|rm|cp|mv|ls|cat|grep|find|chmod|sudo|brew|docker)\s",
    r"^\s*```",
    r"^\s*(?:import|from|def|class|function|const|let|var|return)\s",
    r"^\s*[A-Z_]{2,}=",
    r"^\s*\|",
    r"^\s*[-]{2,}",
    r"^\s*[{}\[\]]\s*$",
    r"^\s*(?:if|for|while|try|except|elif|else:)\b",
];

#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedMemory {
    pub content: String,
    pub memory_type: String,
    pub chunk_index: usize,
}

pub fn extract_memories(text: &str, min_confidence: f64) -> Vec<ExtractedMemory> {
    let mut memories = Vec::new();
    for segment in split_into_segments(text) {
        if segment.trim().len() < 20 {
            continue;
        }
        let prose = extract_prose(&segment);
        let mut scores = Vec::new();
        for (memory_type, markers) in marker_sets() {
            let score = score_markers(&prose, markers);
            if score > 0.0 {
                scores.push((memory_type, score));
            }
        }
        if scores.is_empty() {
            continue;
        }

        scores.sort_by(|left, right| right.1.total_cmp(&left.1));
        let (kind, base_score) = scores[0];
        let length_bonus = if segment.len() > 500 {
            2.0
        } else if segment.len() > 200 {
            1.0
        } else {
            0.0
        };
        let confidence = f64::min(1.0, (base_score + length_bonus) / 5.0);
        if confidence < min_confidence {
            continue;
        }

        let adjusted = disambiguate(kind, &prose, &scores);
        memories.push(ExtractedMemory {
            content: segment.trim().to_string(),
            memory_type: adjusted.to_string(),
            chunk_index: memories.len(),
        });
    }
    memories
}

fn marker_sets() -> [(&'static str, &'static [&'static str]); 5] {
    [
        ("decision", DECISION_MARKERS),
        ("preference", PREFERENCE_MARKERS),
        ("milestone", MILESTONE_MARKERS),
        ("problem", PROBLEM_MARKERS),
        ("emotional", EMOTION_MARKERS),
    ]
}

fn split_into_segments(text: &str) -> Vec<String> {
    let lines = text.lines().collect::<Vec<_>>();
    let turn_patterns = [
        Regex::new(r"^>\s").unwrap(),
        Regex::new(r"^(?:Human|User|Q)\s*:").unwrap(),
        Regex::new(r"^(?:Assistant|AI|A|Claude|ChatGPT)\s*:").unwrap(),
    ];
    let turn_count = lines
        .iter()
        .filter(|line| turn_patterns.iter().any(|pattern| pattern.is_match(line.trim())))
        .count();
    if turn_count >= 3 {
        return split_by_turns(&lines, &turn_patterns);
    }

    let paragraphs = text
        .split("\n\n")
        .map(str::trim)
        .filter(|paragraph| !paragraph.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if paragraphs.len() <= 1 && lines.len() > 20 {
        return lines
            .chunks(25)
            .map(|group| group.join("\n"))
            .filter(|group| !group.trim().is_empty())
            .collect();
    }
    paragraphs
}

fn split_by_turns(lines: &[&str], turn_patterns: &[Regex]) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        let is_turn = turn_patterns.iter().any(|pattern| pattern.is_match(trimmed));
        if is_turn && !current.is_empty() {
            segments.push(current.join("\n"));
            current = vec![(*line).to_string()];
        } else {
            current.push((*line).to_string());
        }
    }
    if !current.is_empty() {
        segments.push(current.join("\n"));
    }
    segments
}

fn extract_prose(text: &str) -> String {
    let mut prose = Vec::new();
    let mut in_code = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code || is_code_line(line) {
            continue;
        }
        prose.push(line);
    }
    let joined = prose.join("\n");
    if joined.trim().is_empty() {
        text.to_string()
    } else {
        joined
    }
}

fn is_code_line(line: &str) -> bool {
    let stripped = line.trim();
    if stripped.is_empty() {
        return false;
    }
    if CODE_LINE_PATTERNS
        .iter()
        .any(|pattern| Regex::new(pattern).unwrap().is_match(stripped))
    {
        return true;
    }
    let alpha_ratio = stripped.chars().filter(|c| c.is_alphabetic()).count() as f64
        / stripped.len().max(1) as f64;
    alpha_ratio < 0.4 && stripped.len() > 10
}

fn score_markers(text: &str, markers: &[&str]) -> f64 {
    let lowered = text.to_ascii_lowercase();
    markers
        .iter()
        .map(|pattern| Regex::new(pattern).unwrap().find_iter(&lowered).count() as f64)
        .sum()
}

fn disambiguate<'a>(memory_type: &'a str, text: &str, scores: &[(&str, f64)]) -> &'a str {
    let sentiment = sentiment(text);
    let score_for = |kind: &str| -> f64 {
        scores
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, score)| *score)
            .unwrap_or_default()
    };

    if memory_type == "problem" && has_resolution(text) {
        if score_for("emotional") > 0.0 && sentiment == "positive" {
            return "emotional";
        }
        return "milestone";
    }

    if memory_type == "problem" && sentiment == "positive" {
        if score_for("milestone") > 0.0 {
            return "milestone";
        }
        if score_for("emotional") > 0.0 {
            return "emotional";
        }
    }

    if memory_type == "milestone"
        && score_for("emotional") > 0.0
        && Regex::new(r"\bi feel\b|\bi'?m\b|\bi\b")
            .unwrap()
            .is_match(&text.to_ascii_lowercase())
    {
        return "emotional";
    }

    memory_type
}

fn has_resolution(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    [
        r"\bfixed\b",
        r"\bsolved\b",
        r"\bresolved\b",
        r"\bpatched\b",
        r"\bgot it working\b",
        r"\bit works\b",
        r"\bnailed it\b",
        r"\bfigured (?:it )?out\b",
        r"\bthe (?:fix|answer|solution)\b",
    ]
    .iter()
    .any(|pattern| Regex::new(pattern).unwrap().is_match(&lowered))
}

fn sentiment(text: &str) -> &'static str {
    let words = Regex::new(r"\b\w+\b")
        .unwrap()
        .find_iter(&text.to_ascii_lowercase())
        .map(|match_| match_.as_str().to_string())
        .collect::<Vec<_>>();
    let positive = words
        .iter()
        .filter(|word| POSITIVE_WORDS.contains(&word.as_str()))
        .count();
    let negative = words
        .iter()
        .filter(|word| NEGATIVE_WORDS.contains(&word.as_str()))
        .count();
    if positive > negative {
        "positive"
    } else if negative > positive {
        "negative"
    } else {
        "neutral"
    }
}
