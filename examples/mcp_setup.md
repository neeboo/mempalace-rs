# MCP Integration — Claude Code

## Setup

Run the MCP server:

```bash
cargo run --bin mempalace -- mcp-server
```

Or add to Claude Code:

```bash
claude mcp add mempalace -- /absolute/path/to/mempalace-rs/target/release/mempalace mcp-server
```

## Available Tools

- **`mempalace_status`** — palace stats, AAAK spec, and protocol hints
- **`mempalace_search`** — search across all memories
- **`mempalace_list_wings`** — list all wings in the palace

## Usage in Claude Code

Once configured, Claude Code can search your memories directly during conversations.
