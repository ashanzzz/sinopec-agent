# AGENTS.md — Sinopec Agent (`sinopec-agent`)

## 1. Project Objective

`sinopec-agent` is a lightweight, deterministic Rust backend (`sinopec-server`), MCP server (`/mcp`), and optional React console (`web/`) for Sinopec Fuel Card (`https://www.sinopecsales.com/`) authentication, card discovery, transaction querying, invoice capability detection, read-only invoice preview, idempotent invoice submission, and electronic invoice downloading.

## 2. Core Engineering Philosophy

- **Browser discovers protocol; Rust executes protocol deterministically.**
- Never build production features around AI guessing DOM selectors or clicking screenshots on every run.
- Always follow the scientific loop:
  `Observe -> Hypothesize -> Experiment -> Verify -> Reproduce -> Implement -> Test`
- Use four explicit research states in `docs/research/sinopec.md` and SQLite `research_state`:
  - `CONFIRMED`: Protocol understood, implemented in Rust, covered by fixtures/tests, and verified via read-only HTTP.
  - `HYPOTHESIS`: Entry URL or JS call site identified, awaiting authenticated verification.
  - `UNKNOWN`: Not yet observed in a real authenticated session or Human Demonstration.
  - `BROKEN`: Upstream contract changed (`API_CONTRACT_CHANGED`).

## 3. Architecture

- **Single Service Layer**: Both REST (`/api/v1/*`) and MCP (`/mcp`) call `SinopecService`. Never duplicate business logic between REST and MCP handlers.
- **Database**: Single-file SQLite (`/data/sinopec.db`) storing only local operational state (`operations`, `human_actions`, `downloads`, `research_state`, `kv`). Sinopec's server is always the business source of truth.
- **No Float Money**: Never use `f32` or `f64` for CNY currency. Sinopec's frontend (`res/js/money.js`) uses integer **Fen (分)** (`i64`) and `rust_decimal::Decimal`.

## 4. Authentication State Machine & CredentialProvider

- `AuthState` uses 16 explicit states (`UNKNOWN`, `CHECKING`, `CREDENTIALS_REQUIRED`, `LOGGED_OUT`, `AUTO_LOGIN`, `LOGIN_IN_PROGRESS`, `SMS_REQUIRED`, `CAPTCHA_REQUIRED`, `MFA_REQUIRED`, `HUMAN_ACTION_REQUIRED`, `AUTH_UNVERIFIED`, `LOGGED_IN`, `SESSION_EXPIRED`, `RATE_LIMITED`, `ACCOUNT_LOCKED`, `ERROR`).
- `CredentialProvider` stores credentials in `/data/secrets/credentials.json`. `GET /api/v1/settings` returns boolean flags and masked summaries only — never plaintext passwords.
- `AuthVerifier` checks `/corpgas/webjsp/billQueryAction_queryBalance.json` and `/corpgas/html/memberLoginAction_logInOrOut.json`. HTTP status `390` indicates `SESSION_EXPIRED`.

## 5. Human Action & Human Demonstration Mode

- When SMS verification (`loginAction_smsYzm.json`), arithmetic captcha (`YanZhengMaServlet`), or first-time real invoice confirmation is needed, `HumanActionManager` saves the current `OperationRecord` in SQLite and generates `/human/{token}`.
- Ephemeral SMS codes entered via `/human/{token}` exist in memory only and are purged immediately after use. Never write SMS codes to SQLite, logs, or fixtures.
- Before implementing or executing an unconfirmed write API (such as `/corpgas/webjsp/invoicev2/createInvoice.jsp`), request a **Human Demonstration** in Steel Browser (`http://192.168.8.11:13000/`).

## 6. Invoice Safety & Idempotency Rules

- `preview_invoice` (`POST /api/v1/invoices/preview` / `sinopec_invoice_preview`) is strictly read-only.
- `create_invoice` (`POST /api/v1/invoices` / `sinopec_invoice_create`) requires:
  1. Re-verifying authentication (`AuthVerifier`)
  2. Re-querying eligible transactions and capabilities
  3. Checking `idempotency_key` and `request_hash` in SQLite `operations`
  4. Respecting `SINOPEC_ALLOW_AUTO_SUBMIT=false` (default)
  5. Transitioning to `REMOTE_RESULT_UNKNOWN` if an upstream POST times out — never blindly re-POSTing.

## 7. Steel Browser & Scrapling

- Existing Unraid Steel Browser: `STEEL_BASE_URL=http://192.168.8.11:13000`, `STEEL_CDP_URL=ws://192.168.8.11:19223`.
- Optional Scrapling Research Assistant: `SCRAPLING_BASE_URL=http://192.168.8.11:8111`. Stopping Scrapling or `sinopec-web` must never affect `sinopec-server`.

## 8. Verification Commands

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
npm --prefix web run lint
npm --prefix web run build
```
