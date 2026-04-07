use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::Result;

const COMMON_ENGLISH_WORDS: &[&str] = &[
    "ever", "grace", "will", "bill", "mark", "april", "may", "june", "joy", "hope", "faith",
    "chance", "chase", "hunter", "dash", "flash", "star", "sky", "river", "brook", "lane",
    "art", "clay", "gil", "nat", "max", "rex", "ray", "jay", "rose", "violet", "lily", "ivy",
    "ash", "reed", "sage", "monday", "tuesday", "wednesday", "thursday", "friday", "saturday",
    "sunday", "january", "february", "march", "april", "june", "july", "august", "september",
    "october", "november", "december",
];

const PERSON_CONTEXT_PATTERNS: &[&str] = &[
    r"\b{name}\s+said\b",
    r"\b{name}\s+told\b",
    r"\b{name}\s+asked\b",
    r"\b{name}\s+laughed\b",
    r"\b{name}\s+smiled\b",
    r"\b{name}\s+was\b",
    r"\b{name}\s+is\b",
    r"\b{name}\s+called\b",
    r"\b{name}\s+texted\b",
    r"\bwith\s+{name}\b",
    r"\bsaw\s+{name}\b",
    r"\bcalled\s+{name}\b",
    r"\btook\s+{name}\b",
    r"\bpicked\s+up\s+{name}\b",
    r"\bdrop(?:ped)?\s+(?:off\s+)?{name}\b",
    r"\b{name}(?:'s|s')\b",
    r"\bhey\s+{name}\b",
    r"\bthanks?\s+{name}\b",
    r"^{name}[:\s]",
    r"\bmy\s+(?:son|daughter|kid|child|brother|sister|friend|partner|colleague|coworker)\s+{name}\b",
];

const CONCEPT_CONTEXT_PATTERNS: &[&str] = &[
    r"\bhave\s+you\s+{name}\b",
    r"\bif\s+you\s+{name}\b",
    r"\b{name}\s+since\b",
    r"\b{name}\s+again\b",
    r"\bnot\s+{name}\b",
    r"\b{name}\s+more\b",
    r"\bwould\s+{name}\b",
    r"\bcould\s+{name}\b",
    r"\bwill\s+{name}\b",
    r"(?:the\s+)?{name}\s+(?:of|in|at|for|to)\b",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RegistryPerson {
    pub name: String,
    pub relationship: String,
    pub context: String,
}

impl RegistryPerson {
    pub fn new(name: &str, relationship: &str, context: &str) -> Self {
        Self {
            name: name.to_string(),
            relationship: relationship.to_string(),
            context: context.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersonRecord {
    source: String,
    contexts: Vec<String>,
    aliases: Vec<String>,
    relationship: String,
    confidence: f64,
    #[serde(default)]
    canonical: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegistryData {
    version: u32,
    mode: String,
    people: BTreeMap<String, PersonRecord>,
    projects: Vec<String>,
    ambiguous_flags: Vec<String>,
    #[serde(default)]
    wiki_cache: BTreeMap<String, serde_json::Value>,
}

impl Default for RegistryData {
    fn default() -> Self {
        Self {
            version: 1,
            mode: "personal".to_string(),
            people: BTreeMap::new(),
            projects: Vec::new(),
            ambiguous_flags: Vec::new(),
            wiki_cache: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LookupResult {
    pub entity_type: String,
    pub confidence: f64,
    pub source: String,
    pub name: String,
    pub needs_disambiguation: bool,
}

pub struct EntityRegistry {
    path: PathBuf,
    data: RegistryData,
}

impl EntityRegistry {
    pub fn load(config_dir: Option<&Path>) -> Result<Self> {
        let path = config_dir
            .map(Path::to_path_buf)
            .unwrap_or_else(default_config_dir)
            .join("entity_registry.json");
        let data = if path.exists() {
            serde_json::from_str(&fs::read_to_string(&path)?)?
        } else {
            RegistryData::default()
        };
        Ok(Self { path, data })
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&self.path, serde_json::to_string_pretty(&self.data)?)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn seed(
        &mut self,
        mode: &str,
        people: &[RegistryPerson],
        projects: &[String],
        aliases: &HashMap<String, String>,
    ) -> Result<()> {
        self.data.mode = mode.to_string();
        self.data.projects = projects.to_vec();
        let reverse_aliases = aliases
            .iter()
            .map(|(alias, canonical)| (canonical.clone(), alias.clone()))
            .collect::<HashMap<_, _>>();

        for person in people {
            if person.name.trim().is_empty() {
                continue;
            }
            self.data.people.insert(
                person.name.clone(),
                PersonRecord {
                    source: "onboarding".to_string(),
                    contexts: vec![person.context.clone()],
                    aliases: reverse_aliases
                        .get(&person.name)
                        .map(|alias| vec![alias.clone()])
                        .unwrap_or_default(),
                    relationship: person.relationship.clone(),
                    confidence: 1.0,
                    canonical: None,
                },
            );

            if let Some(alias) = reverse_aliases.get(&person.name) {
                self.data.people.insert(
                    alias.clone(),
                    PersonRecord {
                        source: "onboarding".to_string(),
                        contexts: vec![person.context.clone()],
                        aliases: vec![person.name.clone()],
                        relationship: person.relationship.clone(),
                        confidence: 1.0,
                        canonical: Some(person.name.clone()),
                    },
                );
            }
        }

        self.data.ambiguous_flags = self
            .data
            .people
            .keys()
            .filter(|name| is_common_english_word(name))
            .map(|name| name.to_ascii_lowercase())
            .collect::<Vec<_>>();

        self.save()
    }

    pub fn lookup(&self, word: &str, context: &str) -> LookupResult {
        for (name, record) in &self.data.people {
            let alias_match = record
                .aliases
                .iter()
                .any(|alias| alias.eq_ignore_ascii_case(word));
            if name.eq_ignore_ascii_case(word) || alias_match {
                if self
                    .data
                    .ambiguous_flags
                    .iter()
                    .any(|flag| flag == &word.to_ascii_lowercase())
                {
                    if let Some(result) = disambiguate(word, context, record) {
                        return result;
                    }
                }
                return LookupResult {
                    entity_type: "person".to_string(),
                    confidence: record.confidence,
                    source: record.source.clone(),
                    name: word.to_string(),
                    needs_disambiguation: false,
                };
            }
        }

        if self
            .data
            .projects
            .iter()
            .any(|project| project.eq_ignore_ascii_case(word))
        {
            return LookupResult {
                entity_type: "project".to_string(),
                confidence: 1.0,
                source: "onboarding".to_string(),
                name: word.to_string(),
                needs_disambiguation: false,
            };
        }

        LookupResult {
            entity_type: "unknown".to_string(),
            confidence: 0.0,
            source: "none".to_string(),
            name: word.to_string(),
            needs_disambiguation: false,
        }
    }

    pub fn summary(&self) -> String {
        format!(
            "Mode: {}\nPeople: {}\nProjects: {}\nAmbiguous flags: {}",
            self.data.mode,
            self.data.people.len(),
            if self.data.projects.is_empty() {
                "(none)".to_string()
            } else {
                self.data.projects.join(", ")
            },
            if self.data.ambiguous_flags.is_empty() {
                "(none)".to_string()
            } else {
                self.data.ambiguous_flags.join(", ")
            }
        )
    }
}

fn disambiguate(word: &str, context: &str, record: &PersonRecord) -> Option<LookupResult> {
    let word = regex::escape(&word.to_ascii_lowercase());
    let context = context.to_ascii_lowercase();
    let person_score = PERSON_CONTEXT_PATTERNS
        .iter()
        .filter(|pattern| Regex::new(&pattern.replace("{name}", &word)).unwrap().is_match(&context))
        .count();
    let concept_score = CONCEPT_CONTEXT_PATTERNS
        .iter()
        .filter(|pattern| Regex::new(&pattern.replace("{name}", &word)).unwrap().is_match(&context))
        .count();

    if person_score > concept_score {
        Some(LookupResult {
            entity_type: "person".to_string(),
            confidence: f64::min(0.95, 0.7 + person_score as f64 * 0.1),
            source: record.source.clone(),
            name: word.replace('\\', ""),
            needs_disambiguation: false,
        })
    } else if concept_score > person_score {
        Some(LookupResult {
            entity_type: "concept".to_string(),
            confidence: f64::min(0.9, 0.7 + concept_score as f64 * 0.1),
            source: "context_disambiguated".to_string(),
            name: word.replace('\\', ""),
            needs_disambiguation: false,
        })
    } else {
        None
    }
}

fn is_common_english_word(word: &str) -> bool {
    COMMON_ENGLISH_WORDS.contains(&word.to_ascii_lowercase().as_str())
}

fn default_config_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".mempalace")
}
