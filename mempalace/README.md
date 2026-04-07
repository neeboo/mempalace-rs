# mempalace/ — Legacy Reference Package

This directory is preserved as the upstream Python reference while `src/` provides the active Rust runtime for MemPalace-rs. Original upstream project: [milla-jovovich/mempalace](https://github.com/milla-jovovich/mempalace).

## Why It Still Exists

- parity audits against the original project
- source comparison during the rewrite
- inherited benchmark behavior and fixtures

## Reference Modules

| Module | What it represents |
|--------|--------------------|
| `cli.py` | Original Python CLI entry point |
| `config.py` | Original config loading |
| `normalize.py` | Original transcript normalization |
| `miner.py` | Original project ingest |
| `convo_miner.py` | Original conversation ingest |
| `searcher.py` | Original search path |
| `layers.py` | Original wake-up stack |
| `dialect.py` | Original AAAK implementation |
| `knowledge_graph.py` | Original knowledge graph |
| `palace_graph.py` | Original navigation graph |
| `mcp_server.py` | Original MCP surface |
| `onboarding.py` | Original guided setup |
| `entity_registry.py` | Original entity registry |
| `entity_detector.py` | Original entity detector |
| `general_extractor.py` | Original five-type extractor |
| `room_detector_local.py` | Original folder-to-room mapper |
| `spellcheck.py` | Original spellcheck helper |
| `split_mega_files.py` | Original transcript splitter |

## Active Runtime

The Rust runtime lives in `src/` and owns the commands documented in the top-level [README](/Users/ghostcorn/dev/mempalace/README.md).
