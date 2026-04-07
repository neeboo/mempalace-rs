use std::path::PathBuf;
use std::{io::Read, path::Path};

use clap::{Parser, Subcommand, ValueEnum};

use mempalace_rs::config::MempalaceConfig;
use mempalace_rs::convo::{ExtractMode, mine_conversations_with_extract_mode};
use mempalace_rs::dialect::{CompressionMetadata, Dialect};
use mempalace_rs::hook_protocol::{handle_precompact_hook, handle_stop_hook};
use mempalace_rs::layers::MemoryStack;
use mempalace_rs::mcp_server::McpServer;
use mempalace_rs::miner::mine_project;
use mempalace_rs::onboarding::bootstrap_project;
use mempalace_rs::search::search_memories;
use mempalace_rs::split::split_file;
use mempalace_rs::storage::PalaceStore;

#[derive(Debug, Parser)]
#[command(name = "mempalace", about = "MemPalace — Give your AI a memory. No API key required.")]
struct Cli {
    #[arg(long, global = true)]
    palace: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Init {
        dir: PathBuf,
        #[arg(long, default_value_t = false)]
        yes: bool,
    },
    Mine {
        dir: PathBuf,
        #[arg(long, default_value = "projects")]
        mode: Mode,
        #[arg(long, default_value = "exchange")]
        extract: ExtractMode,
        #[arg(long)]
        wing: Option<String>,
        #[arg(long, default_value = "mempalace")]
        agent: String,
        #[arg(long, default_value_t = 0)]
        limit: usize,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Search {
        query: String,
        #[arg(long)]
        wing: Option<String>,
        #[arg(long)]
        room: Option<String>,
        #[arg(long, default_value_t = 5)]
        results: usize,
    },
    Status,
    Compress {
        #[arg(long)]
        wing: Option<String>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    WakeUp {
        #[arg(long)]
        wing: Option<String>,
    },
    Split {
        dir: PathBuf,
        #[arg(long)]
        output_dir: Option<PathBuf>,
        #[arg(long, default_value_t = false)]
        dry_run: bool,
    },
    Hook {
        #[command(subcommand)]
        event: HookEvent,
    },
    McpServer,
}

#[derive(Debug, Subcommand)]
enum HookEvent {
    Stop {
        #[arg(long)]
        state_dir: Option<PathBuf>,
        #[arg(long)]
        mempal_dir: Option<PathBuf>,
        #[arg(long, default_value_t = 15)]
        save_interval: usize,
    },
    Precompact {
        #[arg(long)]
        state_dir: Option<PathBuf>,
        #[arg(long)]
        mempal_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, ValueEnum)]
enum Mode {
    Projects,
    Convos,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> mempalace_rs::Result<()> {
    let cli = Cli::parse();
    let cfg = MempalaceConfig::new(None)?;
    let palace_path = cli.palace.unwrap_or_else(|| cfg.palace_path());

    match cli.command {
        Commands::Init { dir, yes } => {
            cfg.init()?;
            let summary = bootstrap_project(&dir, None, yes)?;
            println!("Config saved: {}", summary.config_path.display());
            if let Some(path) = summary.entities_path {
                println!("Entities saved: {}", path.display());
            }
            println!("Registry saved: {}", summary.registry_path.display());
        }
        Commands::Mine {
            dir,
            mode,
            extract,
            wing,
            agent,
            limit,
            dry_run,
        } => match mode {
            Mode::Projects => {
                let summary = mine_project(&dir, &palace_path, wing.as_deref(), &agent, limit, dry_run)?;
                println!(
                    "Files processed: {} | Files skipped: {} | Drawers filed: {}",
                    summary.files_processed, summary.files_skipped, summary.drawers_filed
                );
            }
            Mode::Convos => {
                let summary = mine_conversations_with_extract_mode(
                    &dir,
                    &palace_path,
                    wing.as_deref(),
                    &agent,
                    limit,
                    dry_run,
                    extract,
                )?;
                println!(
                    "Files processed: {} | Files skipped: {} | Drawers filed: {}",
                    summary.files_processed, summary.files_skipped, summary.drawers_filed
                );
            }
        },
        Commands::Search {
            query,
            wing,
            room,
            results,
        } => {
            let store = PalaceStore::open(&palace_path)?;
            let hits = search_memories(&store, &query, wing.as_deref(), room.as_deref(), results)?;
            if hits.is_empty() {
                println!("No results found for: {query}");
            } else {
                for (index, hit) in hits.iter().enumerate() {
                    println!("[{}] {} / {}", index + 1, hit.wing, hit.room);
                    println!("    Source: {}", hit.source_file);
                    println!("    Match:  {}", hit.score);
                    for line in hit.text.lines() {
                        println!("    {line}");
                    }
                    println!();
                }
            }
        }
        Commands::Status => {
            let store = PalaceStore::open(&palace_path)?;
            let counts = store.status_counts()?;
            if counts.is_empty() {
                println!("MemPalace Status — 0 drawers");
            } else {
                let total = store.drawer_count()?;
                println!("MemPalace Status — {total} drawers");
                for (wing, room, count) in counts {
                    println!("  {wing:20} {room:20} {count}");
                }
            }
        }
        Commands::Compress { wing, dry_run } => {
            let store = PalaceStore::open(&palace_path)?;
            let dialect = Dialect::default();
            for drawer in store.list_drawers(wing.as_deref(), None)? {
                let compressed = dialect.compress(
                    &drawer.content,
                    Some(&CompressionMetadata {
                        source_file: Some(drawer.source_file.clone()),
                        wing: Some(drawer.wing.clone()),
                        room: Some(drawer.room.clone()),
                        date: drawer.date.clone(),
                    }),
                );
                let stats = dialect.compression_stats(&drawer.content, &compressed);
                if dry_run {
                    println!("[{}/{}] {}", drawer.wing, drawer.room, compressed);
                } else {
                    store.upsert_compressed_drawer(
                        &drawer.id,
                        &drawer.wing,
                        &drawer.room,
                        &drawer.source_file,
                        &compressed,
                        stats.ratio,
                        stats.original_tokens,
                    )?;
                }
            }
        }
        Commands::WakeUp { wing } => {
            let stack = MemoryStack::new(
                palace_path.clone(),
                Some(
                    std::env::var("HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|_| PathBuf::from("."))
                        .join(".mempalace/identity.txt"),
                ),
            );
            println!("{}", stack.wake_up(wing.as_deref())?);
        }
        Commands::Split {
            dir,
            output_dir,
            dry_run,
        } => {
            let mut created = 0usize;
            for entry in std::fs::read_dir(&dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) != Some("txt") {
                    continue;
                }
                created += split_file(&path, output_dir.as_deref(), dry_run)?.len();
            }
            println!("Processed split output count: {created}");
        }
        Commands::Hook { event } => {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input)?;
            let default_state_dir =
                Path::new(&std::env::var("HOME").unwrap_or_else(|_| ".".to_string()))
                    .join(".mempalace/hook_state");
            let response = match event {
                HookEvent::Stop {
                    state_dir,
                    mempal_dir,
                    save_interval,
                } => {
                    let state_dir = state_dir.unwrap_or_else(|| default_state_dir.clone());
                    handle_stop_hook(
                        &input,
                        &state_dir,
                        mempal_dir.as_deref(),
                        &palace_path,
                        save_interval,
                    )?
                }
                HookEvent::Precompact {
                    state_dir,
                    mempal_dir,
                } => {
                    let state_dir = state_dir.unwrap_or_else(|| default_state_dir.clone());
                    handle_precompact_hook(
                        &input,
                        &state_dir,
                        mempal_dir.as_deref(),
                        &palace_path,
                    )?
                }
            };
            println!("{response}");
        }
        Commands::McpServer => {
            let server = McpServer::new(None, Some(palace_path.clone()))?;
            server.serve_stdio()?;
        }
    }

    Ok(())
}
