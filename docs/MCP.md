# MCP Server Reference (`/mcp`)

`sinopec-server` exposes a Streamable HTTP JSON-RPC 2.0 Model Context Protocol endpoint at `/mcp`.

## Production Tools (Always Enabled)

1. `sinopec_auth_status` — Query authentication state machine and account mode.
2. `sinopec_auth_ensure` — Ensure authentication or return a recoverable `human_action_url`.
3. `sinopec_open_human_action` — Open Steel Browser and create a Human Action for manual verification or demonstration.
4. `sinopec_account_info` — Get account mode (`personal`/`corporate`), company name, and customer type.
5. `sinopec_list_cards` — List bound fuel cards with masked card numbers (`****8816`).
6. `sinopec_invoice_capabilities` — Inspect `InvoiceCapabilities` for the account.
7. `sinopec_invoice_quota` — Query corporate invoice quota in CNY.
8. `sinopec_invoice_preview` — Generate a read-only preview for `start_date`..`end_date`.
9. `sinopec_invoice_create` — Submit an invoice with idempotency and confirmation checks.
10. `sinopec_invoice_list` — List issued electronic invoices.
11. `sinopec_invoice_download` — Download electronic invoice PDF and record SHA-256 in SQLite.

## Research Mode Tools (`SINOPEC_RESEARCH_MODE=true` Only)

12. `sinopec_research_status`
13. `sinopec_browser_open`
14. `sinopec_network_capture`
15. `sinopec_research_snapshot`
