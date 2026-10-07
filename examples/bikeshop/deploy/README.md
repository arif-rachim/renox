# Deploying Bikeshop

Renox apps deploy as one binary (views, translations and public files are
compiled in) plus a `.env` file and a `storage/` directory.

## The quick way: pull the builds GitHub makes

Every push to `main` that touches the bike shop or Renox is built by
[`.github/workflows/release-bikeshop.yml`](../../../.github/workflows/release-bikeshop.yml)
on Ubuntu 22.04 (so it runs on Ubuntu 22.04+ and Debian 12+, x86_64 and arm64)
and published as the public pre-release
[`bikeshop-latest`](https://github.com/arif-rachim/renox/releases/tag/bikeshop-latest)
(a tag `bikeshop-v1.2.3` makes a release of its own, to pin). A server pulls
it with `curl`: no GitHub account, token or key on the server, and CI holds
no server keys.

On a fresh server (as root, with systemd, `curl` and a domain pointing at it):

```bash
curl -fsSL https://raw.githubusercontent.com/arif-rachim/renox/main/examples/bikeshop/deploy/update.sh \
  | sudo bash -s -- --url https://shop.example.com --seed demo
```

That one command:

1. downloads the build for the server's CPU and checks its SHA-256;
2. creates the `bikeshop` system user and `/opt/bikeshop` with a production
   `.env` (`APP_ENV=production`, `APP_DEBUG=false`, a new `APP_KEY`,
   `APP_URL`, the app on `127.0.0.1:3000`, `TRUSTED_PROXIES=127.0.0.1`) and
   `storage/` (the SQLite database, uploads, logs);
3. runs the migrations and, with `--seed demo` (or `large`), fills the demo
   shop: the demo users are printed (password `password`);
4. installs and starts `bikeshop.socket` + `bikeshop.service` (restarts don't
   refuse connections) and `bikeshop-update.timer`, which runs the same script
   every hour as `/usr/local/sbin/bikeshop-update`;
5. checks `/health`.

Each later run (the timer, or `sudo bikeshop-update` by hand) downloads the
newest build, does nothing when it's the one running, else runs its
migrations, switches `/opt/bikeshop/current` to it, restarts and checks
`/health`. If the new build doesn't answer, it switches back to the previous
one (migrations are not undone: write them to keep working with the previous
build). The last three builds stay in `/opt/bikeshop/releases/`.

| Option | |
|---|---|
| `--version bikeshop-v1.0.0` | a pinned release instead of `bikeshop-latest` |
| `--url https://…` | `APP_URL` on the first install |
| `--port 3000` | the local port (read back from `.env` on later runs) |
| `--seed none\|demo\|large` | the demo data on the first install |
| `--dir /opt/bikeshop` | where it lives |
| `--no-timer` | no hourly updates (run `sudo bikeshop-update` yourself) |

Then put a reverse proxy in front for HTTPS, e.g. Caddy
(`/etc/caddy/Caddyfile`):

```
shop.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

Fill in the rest of `/opt/bikeshop/.env` (mail, payment gateway keys,
`GOOGLE_*`/`GITHUB_*` for social login: see the comments in it) and
`sudo systemctl restart bikeshop`. Logs: `journalctl -u bikeshop -f`; the app
binary's commands run as the app: `cd /opt/bikeshop && sudo -u bikeshop
./current/bikeshop schedule:list`.

## On a Linux server with systemd

1. Build: `rnx build` (or `cargo build --release`) → `dist/bikeshop`.
   Build on the same OS/CPU as the server, or use the Docker image below.
2. On the server:

   ```bash
   sudo useradd --system --home /opt/bikeshop bikeshop
   sudo mkdir -p /opt/bikeshop/storage
   sudo mkdir -p /opt/bikeshop/releases/manual
   sudo cp dist/bikeshop /opt/bikeshop/releases/manual/
   sudo ln -sfn releases/manual /opt/bikeshop/current
   sudo cp .env.example /opt/bikeshop/.env    # then edit it, see below
   sudo chown -R bikeshop /opt/bikeshop
   sudo cp deploy/bikeshop.service /etc/systemd/system/
   sudo systemctl enable --now bikeshop
   ```

3. In `/opt/bikeshop/.env` set at least:
   `APP_ENV=production`, `APP_DEBUG=false`, `APP_KEY=` (from `rnx key:generate --show`),
   `APP_URL=https://your-domain`, `APP_HOST=127.0.0.1` and `TRUSTED_PROXIES=127.0.0.1`
   behind a reverse proxy (Caddy or nginx), and your `MAIL_*` settings.
4. Updates: copy the new binary into `releases/manual/` and `sudo systemctl restart bikeshop`
   (migrations run before it starts).

## Deploys without refused connections

Out of the box, a restart closes the port for the second or two the app takes
to stop and start; visitors in that moment get "connection refused". With
systemd socket activation, systemd keeps the port open and queues them:

```bash
sudo cp deploy/bikeshop.socket /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl stop bikeshop             # it holds the port; the socket takes it over
sudo systemctl enable --now bikeshop.socket
sudo systemctl start bikeshop
# From now on, deploys are: copy the new binary, then
sudo systemctl restart bikeshop          # connections wait, none are refused
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
   `litestream restore -o /opt/bikeshop/storage/app.db /opt/bikeshop/storage/app.db`.

## With Docker

```bash
docker build -t bikeshop .
docker run -d --name bikeshop -p 3000:3000 \
  -e APP_KEY=base64:... -e APP_URL=https://your-domain \
  -v bikeshop-storage:/app/storage bikeshop
```

The `renox` dependency must be reachable from the build (crates.io or git), not a local path.
Health checks: `GET /health` answers 200 while the database is reachable.

Timeouts, health checks, backups and recovering failed jobs and webhooks:
https://github.com/arif-rachim/renox/blob/main/docs/operations.md
