# Deploying Shop

Renox apps deploy as one binary (views, translations and public files are
compiled in) plus a `.env` file and a `storage/` directory.

## On a Linux server with systemd

1. Build: `rnx build` (or `cargo build --release`) → `dist/shop`.
   Build on the same OS/CPU as the server, or use the Docker image below.
2. On the server:

   ```bash
   sudo useradd --system --home /opt/shop shop
   sudo mkdir -p /opt/shop/storage
   sudo cp dist/shop /opt/shop/
   sudo cp .env.example /opt/shop/.env    # then edit it, see below
   sudo chown -R shop /opt/shop
   sudo cp deploy/shop.service /etc/systemd/system/
   sudo systemctl enable --now shop
   ```

3. In `/opt/shop/.env` set at least:
   `APP_ENV=production`, `APP_DEBUG=false`, `APP_KEY=` (from `rnx key:generate --show`),
   `APP_URL=https://your-domain`, `APP_HOST=127.0.0.1` and `TRUSTED_PROXIES=127.0.0.1`
   behind a reverse proxy (Caddy or nginx), and your `MAIL_*` settings.
4. Updates: copy the new binary and `sudo systemctl restart shop`
   (migrations run before it starts).

## Backups with Litestream

For SQLite apps. On PostgreSQL, set `DATABASE_URL` to the server instead and use
its own backups (`pg_dump` or your provider's); see Renox's docs/postgresql.md.

1. Install Litestream (https://litestream.io/install/).
2. `sudo cp deploy/litestream.yml /etc/litestream.yml`, fill in the bucket and
   put the keys in `/etc/default/litestream`:
   `LITESTREAM_ACCESS_KEY_ID=...` and `LITESTREAM_SECRET_ACCESS_KEY=...`.
3. `sudo systemctl enable --now litestream`.
4. Restore on a new server before starting the app:
   `litestream restore -o /opt/shop/storage/app.db /opt/shop/storage/app.db`.

## With Docker

```bash
docker build -t shop .
docker run -d --name shop -p 3000:3000 \
  -e APP_KEY=base64:... -e APP_URL=https://your-domain \
  -v shop-storage:/app/storage shop
```

The `renox` dependency must be reachable from the build (crates.io or git), not a local path.
Health checks: `GET /health` answers 200 while the database is reachable.

Timeouts, health checks, backups and recovering failed jobs and webhooks:
https://github.com/arif-rachim/renox/blob/main/docs/operations.md
