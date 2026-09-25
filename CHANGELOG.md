# Changelog

## [0.1.0] - 2026-09-25

### Added
- Initialized Rust workspace (`sinopec-server`) with Axum REST API (`/api/v1`), Streamable HTTP MCP server (`/mcp`), and embedded `/human/{token}` UI.
- Implemented single-file SQLite storage (`operations`, `human_actions`, `downloads`, `research_state`, `kv`).
- Implemented `CredentialProvider`, `AuthVerifier` (HTTP `390` session detection on `/corpgas/webjsp/billQueryAction_queryBalance.json`), `HumanActionManager`, `Redactor`, and `ResearchRecorder`.
- Implemented `SteelBrowserDriver` (`192.168.8.11:13000` / `19223`) and optional `ScraplingClient` (`192.168.8.11:8111`).
- Completed initial protocol research on `https://www.sinopecsales.com/` (`docs/research/sinopec.md`) and redacted fixtures (`tests/fixtures/`).
- Built standalone React + TypeScript + Vite console (`web/`) with Dashboard, Invoice, Invoices, Human Actions, Settings, Research, and System views.
- Added multi-stage Dockerfiles, `docker-compose.yml` (`ui` profile), Unraid XML templates (`unraid/`), and GitHub Actions CI/GHCR release workflows.
