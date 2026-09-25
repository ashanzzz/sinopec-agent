# Development Guide (`DEVELOPMENT.md`)

## 1. Prerequisites

- Rust 1.85+ (`cargo`, `rustfmt`, `clippy`)
- Node.js 20+ (`npm`) for the optional `web/` console

## 2. Local Commands

```bash
# Run backend server (default 0.0.0.0:8788)
cargo run --package sinopec-server

# Run backend test suite
cargo test --all

# Run React web console in dev mode (port 5178, proxies /api, /mcp, /human to 8788)
npm --prefix web install
npm --prefix web run dev
```
