# REST API Reference (`/api/v1`)

All endpoints return a unified JSON envelope:

```json
{
  "ok": true,
  "data": {}
}
```

On error:

```json
{
  "ok": false,
  "error": {
    "code": "SMS_CODE_REQUIRED",
    "message": "需要完成中国石化短信验证",
    "human_action": {
      "id": "ha_...",
      "human_action_url": "http://192.168.8.11:8788/human/..."
    }
  }
}
```

## Endpoints

| Method | Path | Description |
| :--- | :--- | :--- |
| `GET` | `/api/v1/health` | Service health check and runtime flags |
| `GET` | `/api/v1/system` | Combined status for Backend, Auth, Steel, Scrapling, and Last Operation |
| `GET` | `/api/v1/auth/status` | Current authentication state machine and credential configuration summary |
| `POST` | `/api/v1/auth/ensure` | Check auth, sync Steel cookies, or create a Human Action if login/SMS is required |
| `POST` | `/api/v1/auth/check` | Run `AuthVerifier` against `/corpgas/webjsp/billQueryAction_queryBalance.json` |
| `GET` | `/api/v1/account` | Account profile (`personal` vs `corporate`, customer code, masked tax ID) |
| `GET` | `/api/v1/cards` | Bound fuel cards (`****1234`, card type, balances in CNY and Fen) |
| `GET` | `/api/v1/invoice-capabilities` | `InvoiceCapabilities` for the current account and master card |
| `GET` | `/api/v1/invoice-quota` | Corporate available invoice quota (`CNY`) |
| `GET` | `/api/v1/transactions` | Query fuel card transactions by `start_date`, `end_date`, `card_id` |
| `POST` | `/api/v1/invoices/preview` | Read-only invoice preview with exact `Decimal` total and blocking reasons |
| `POST` | `/api/v1/invoices` | Idempotent invoice creation with auth re-verification and confirmation gate |
| `GET` | `/api/v1/invoices` | List issued electronic invoices |
| `GET` | `/api/v1/invoices/{id}` | Get single invoice details |
| `POST` | `/api/v1/invoices/{id}/download` | Download official invoice file to `/data/downloads/YYYY/MM/` with SHA-256 |
| `GET` | `/api/v1/operations/{id}` | Query saved/recoverable operation state |
| `GET` | `/api/v1/human-actions` | List Human Action records |
| `GET` | `/api/v1/human-actions/{id}` | Get Human Action details |
| `POST` | `/api/v1/human-actions/{id}/complete` | Mark Human Action completed and trigger `AuthVerifier` + task resumption |
| `POST` | `/api/v1/human-actions/{id}/cancel` | Cancel Human Action |
| `GET` | `/api/v1/settings` | Get non-sensitive settings and boolean credential flags |
| `PUT` | `/api/v1/settings/credentials` | Write-only credential update (`/data/secrets/credentials.json`) |
| `DELETE` | `/api/v1/settings/credentials` | Delete stored credentials |
| `GET` | `/human/{token}` | Built-in minimal HTML Human Action UI (works without React frontend) |
