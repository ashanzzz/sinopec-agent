# Sinopec Agent (`sinopec-agent`)

Lightweight, high-reliability Rust backend (`sinopec-server`), Model Context Protocol (`/mcp`) service, and optional React console (`web/`) for China Sinopec Fuel Card (`https://www.sinopecsales.com/`) invoice inquiry, capability detection, read-only preview, idempotent submission, and official electronic invoice download.

## Key Highlights

- **Deterministic Rust Backend (`sinopec-server`)**: Built with Tokio, Axum, Reqwest, SQLx (SQLite single-file `/data/sinopec.db`), and `rust_decimal` (exact integer `fen` & decimal arithmetic — never floats).
- **Unified Service Layer**: REST (`/api/v1/*`) and Streamable HTTP MCP (`/mcp`) share `SinopecService`.
- **Resilient Authentication & Human Action**: 16-state authentication state machine with automatic Steel Browser session cookie sync (`/corpgas/webjsp/billQueryAction_queryBalance.json`, HTTP `390` detection) and built-in `/human/{token}` UI that works even when the React frontend is stopped.
- **Safety First**: Read-only `sinopec_invoice_preview`, idempotent `sinopec_invoice_create` with `SINOPEC_ALLOW_AUTO_SUBMIT=false` default gate, and `REMOTE_RESULT_UNKNOWN` timeout protection.

## Quick Start

```bash
cp .env.example .env
cargo test --all
cargo run --package sinopec-server
```
