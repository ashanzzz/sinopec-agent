# Unraid Deployment Guide (`UNRAID.md`)

## 1. Host Environment (`192.168.8.11`)

- **Appdata Path**: `/mnt/user/appdata/sinopec-agent/`
  - `/mnt/user/appdata/sinopec-agent/sinopec.db` (SQLite WAL database)
  - `/mnt/user/appdata/sinopec-agent/secrets/credentials.json`
  - `/mnt/user/appdata/sinopec-agent/auth/sinopec_cookies.json`
  - `/mnt/user/appdata/sinopec-agent/downloads/YYYY/MM/`
  - `/mnt/user/appdata/sinopec-agent/research/`
- **Existing Steel Browser**:
  - `STEEL_BASE_URL=http://192.168.8.11:13000`
  - `STEEL_CDP_URL=ws://192.168.8.11:19223`
- **Existing Scrapling**:
  - `SCRAPLING_BASE_URL=http://192.168.8.11:8111`

## 2. Docker Compose Commands

Run backend only (recommended production mode, minimal RAM/CPU):

```bash
docker compose up -d
```

Run backend + optional React Web UI:

```bash
docker compose --profile ui up -d
```

## 3. Unraid XML Templates

Import `unraid/my-sinopec-agent.xml` (and optionally `unraid/my-sinopec-agent-web.xml`) into `/boot/config/plugins/dockerMan/templates-user/`.
