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

## Deploys without refused connections

Out of the box, a restart closes the port for the second or two the app takes
to stop and start; visitors in that moment get "connection refused". With
systemd socket activation, systemd keeps the port open and queues them:

```bash
sudo cp deploy/shop.socket /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl stop shop             # it holds the port; the socket takes it over
sudo systemctl enable --now shop.socket
sudo systemctl start shop
# From now on, deploys are: copy the new binary, then
sudo systemctl restart shop          # connections wait, none are refused
```

Keep `ListenStream` in the socket file equal to `APP_HOST:APP_PORT`. The
service stops gracefully on SIGTERM (running jobs finish first), then the new
binary runs its migrations and takes over the same socket.

Migrations must work with the old and the new code for that moment (and
while a second server is still on the old version):

- add a column as nullable or with a default, and backfill it later;
- rename or drop a column in two deploys: first stop using it, then drop it;
- a long backfill belongs in a job or a command, not in the migration.

For deploys that must never pause (a slow start, a warm cache), run two copies
behind Caddy (ports 3000 and 3001, each with its own `APP_PORT`) and restart
them one at a time; Caddy sends traffic to the one that answers `/health`:

```
your-domain.com {
    reverse_proxy 127.0.0.1:3000 127.0.0.1:3001 {
        lb_policy first
        health_uri /health
        health_interval 2s
        lb_try_duration 10s
    }
}
```

Two copies can share one SQLite file on the same machine; the queue and the
scheduler already take each job and each run once. Use `CACHE_STORE=database`
(and `SESSION_DRIVER=database` if sessions outgrow a cookie) so both copies
see the same rate limits, locks and sessions.

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
