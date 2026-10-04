# The documentation site

Renox's documentation site, built with Renox: the guides in `docs/`, the
tutorial, the "Coming from Laravel" guide, the cheat sheet, the changelog and
the project's pages, rendered from this repository's Markdown and compiled
into one binary. Pages carry an `ETag` (`Routes::etag`), there's a search
(`/search?q=`) and a sitemap.

## Run it

```bash
cd site
cargo run          # or: rnx serve
```

Then open http://127.0.0.1:3000. Edit a guide in `docs/` and rebuild: the
pages are compiled in (`include_str!`), so the site always shows the docs of
the commit it was built from.

| What | Where |
|---|---|
| The pages and the sidebar's order | [src/content.rs](src/content.rs) |
| Markdown to HTML: heading anchors, the table of contents, doctest setup lines hidden, links between files turned into site addresses | [src/render.rs](src/render.rs) |
| Routes, search, sitemap | [src/lib.rs](src/lib.rs) |
| The kit's shell with the guides in the sidebar | [resources/views/layouts/app.html](resources/views/layouts/app.html) |
| Long-form text styles | [public/site.css](public/site.css) |
| Every page renders, links land on headings that exist | [tests/site.rs](tests/site.rs) |

## Deploying from GitHub

The site runs at **https://renox.renoxium.com**, on the owner's Ubuntu 24.04 server.
[`.github/workflows/docs-site.yml`](../.github/workflows/docs-site.yml) builds it on Ubuntu
24.04 whenever the docs change on `main` (or from the Actions tab: "Docs site", Run workflow),
copies the binary over SSH, restarts it and checks `/health`.

Once, to set it up:

1. A key for GitHub Actions to deploy with (on your own machine):

   ```bash
   ssh-keygen -t ed25519 -N "" -C renox-docs-deploy -f renox-docs-deploy
   ```

2. The DNS: an `A` (and `AAAA`) record for `renox.renoxium.com` to the server.
3. On the server, as root (it reads `renox-docs-deploy.pub`'s content):

   ```bash
   curl -fsSL https://raw.githubusercontent.com/arif-rachim/renox/main/site/deploy/setup-ubuntu.sh \
       | sudo DOMAIN=renox.renoxium.com DEPLOY_KEY="$(cat renox-docs-deploy.pub)" bash
   ```

   It makes a `renox-site` user that runs the site and a `deploy` user that may only replace
   the binary and restart it, writes `/opt/renox-site/.env` (production, a new `APP_KEY`,
   port 3080, `TRUSTED_HOSTS`), installs the systemd service and socket, and sets up the web
   server: Caddy (installed if there's neither Caddy nor nginx; it fetches the HTTPS
   certificate itself), or a server block printed for an existing nginx.
4. In GitHub, Settings → Environments → New environment `docs-site`, with the secrets
   `DOCS_SSH_HOST` (the server), `DOCS_SSH_KEY` (the content of `renox-docs-deploy`, the
   private key) and `DOCS_SSH_KNOWN_HOSTS` (the output of `ssh-keyscan -t ed25519
   <server>`: the server's key is pinned, never trusted on first sight). Optional
   variables: `DOCS_SSH_USER` (`deploy`) and `DOCS_SITE_URL`.
5. Actions → "Docs site" → Run workflow, for the first deploy.

Until the secrets exist, the workflow builds the site and skips the deploy.

## By hand

The site keeps no data. On a Linux server with systemd, behind Caddy or nginx:

1. Build on a machine like the server: `cargo build --release -p renox-site`
   (from the repository root) → `target/release/renox-site`.
2. On the server:

   ```bash
   sudo useradd --system --home /opt/renox-site renox-site
   sudo mkdir -p /opt/renox-site/storage
   sudo cp renox-site /opt/renox-site/
   sudo cp .env.example /opt/renox-site/.env   # then edit it, below
   sudo chown -R renox-site /opt/renox-site
   sudo cp deploy/renox-site.service deploy/renox-site.socket /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now renox-site.socket
   sudo systemctl start renox-site
   ```

3. In `/opt/renox-site/.env`: `APP_ENV=production`, `APP_DEBUG=false`,
   `APP_KEY=` (from `rnx key:generate --show`), `APP_URL=https://your-domain`
   (links in the sitemap use it), `TRUSTED_PROXIES=127.0.0.1` and
   `TRUSTED_HOSTS=your-domain`, `APP_PORT=3080` (the socket's port; 3000 is often taken by
   another app on the same server).
4. The proxy, e.g. Caddy:

   ```text
   docs.example.com {
       reverse_proxy 127.0.0.1:3080
   }
   ```

5. A new version of the docs: build again, copy the binary over and
   `sudo systemctl restart renox-site` (the socket keeps the port open).
