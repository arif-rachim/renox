#!/usr/bin/env bash
# One-time setup of an Ubuntu 24.04 server for the documentation site, which
# .github/workflows/docs-site.yml then deploys to on every change of the docs.
# Run it as root, once (running it again changes nothing that is already set up):
#
#   curl -fsSL https://raw.githubusercontent.com/arif-rachim/renox/main/site/deploy/setup-ubuntu.sh \
#       | sudo DOMAIN=renox.renoxium.com DEPLOY_KEY="ssh-ed25519 AAAA… renox-docs-deploy" bash
#
# DOMAIN      the site's address (its DNS A/AAAA records point at this server)
# DEPLOY_KEY  the public half of the SSH key GitHub Actions deploys with
# PORT        where the site listens on 127.0.0.1 (3080; 3000 is often another app)
#
# What it does:
# - a `renox-site` user that runs the site, and a `deploy` user GitHub Actions logs
#   in as, allowed to replace the binary and restart the site, and nothing else;
# - /opt/renox-site with a production .env (a new APP_KEY) and storage/;
# - the systemd service and socket (the socket holds the port during restarts);
# - the web server: an existing Caddy gets a site block, an existing nginx gets
#   one printed for you to add, and with neither Caddy is installed (it gets the
#   HTTPS certificate by itself).
set -euo pipefail

DOMAIN=${DOMAIN:?set DOMAIN, e.g. DOMAIN=renox.renoxium.com}
DEPLOY_KEY=${DEPLOY_KEY:?set DEPLOY_KEY to the public SSH key GitHub Actions deploys with}
PORT=${PORT:-3080}
APP=/opt/renox-site

if [ "$(id -u)" -ne 0 ]; then
    echo "Run it as root (sudo)." >&2
    exit 1
fi
step() { printf '\n== %s\n' "$*"; }

step "users"
id renox-site >/dev/null 2>&1 || useradd --system --home "$APP" --shell /usr/sbin/nologin renox-site
id deploy >/dev/null 2>&1 || useradd --create-home --shell /bin/bash deploy
install -d -m 700 -o deploy -g deploy /home/deploy/.ssh
touch /home/deploy/.ssh/authorized_keys
grep -qxF "$DEPLOY_KEY" /home/deploy/.ssh/authorized_keys || echo "$DEPLOY_KEY" >>/home/deploy/.ssh/authorized_keys
chown deploy:deploy /home/deploy/.ssh/authorized_keys
chmod 600 /home/deploy/.ssh/authorized_keys

step "$APP"
# The deploy user writes the binary; the site's own user owns the data.
install -d -m 775 -o renox-site -g deploy "$APP"
install -d -m 750 -o renox-site -g renox-site "$APP/storage"
if [ ! -f "$APP/.env" ]; then
    cat >"$APP/.env" <<EOF
APP_NAME=Renox
APP_ENV=production
APP_DEBUG=false
APP_URL=https://$DOMAIN
APP_KEY=base64:$(openssl rand -base64 32)
APP_HOST=127.0.0.1
APP_PORT=$PORT
# The web server on this machine sends the visitor's address.
TRUSTED_PROXIES=127.0.0.1
# Other Host headers get a 400.
TRUSTED_HOSTS=$DOMAIN
DATABASE_URL=sqlite://storage/site.db
# Nothing to queue or schedule.
QUEUE_WORKERS=0
SCHEDULER=false
EOF
    echo "wrote $APP/.env (with a new APP_KEY)"
else
    echo "kept the existing $APP/.env"
fi
chown root:renox-site "$APP/.env"
chmod 640 "$APP/.env"

step "systemd"
cat >/etc/systemd/system/renox-site.service <<EOF
[Unit]
Description=Renox documentation site
After=network.target
Requires=renox-site.socket

[Service]
User=renox-site
WorkingDirectory=$APP
EnvironmentFile=$APP/.env
ExecStartPre=$APP/renox-site migrate
ExecStart=$APP/renox-site
Restart=on-failure
KillSignal=SIGTERM
TimeoutStopSec=40

[Install]
WantedBy=multi-user.target
EOF
cat >/etc/systemd/system/renox-site.socket <<EOF
# Holds the port and hands it to renox-site.service, so a deploy (a restart)
# makes visitors wait a moment instead of failing.
[Unit]
Description=Renox documentation site (listening socket)

[Socket]
ListenStream=127.0.0.1:$PORT
Backlog=4096

[Install]
WantedBy=sockets.target
EOF
systemctl daemon-reload
systemctl enable --now renox-site.socket
# The service starts with the first deploy (there is no binary yet).
systemctl enable renox-site.service >/dev/null 2>&1 || true

step "sudo for the deploy user: restarting the site, and nothing else"
cat >/etc/sudoers.d/renox-site-deploy <<'EOF'
deploy ALL=(root) NOPASSWD: /usr/bin/systemctl restart renox-site.service
EOF
chmod 440 /etc/sudoers.d/renox-site-deploy
visudo -cf /etc/sudoers.d/renox-site-deploy >/dev/null

step "web server for https://$DOMAIN"
BLOCK="$DOMAIN {
    reverse_proxy 127.0.0.1:$PORT
}"
if command -v caddy >/dev/null 2>&1; then
    if grep -q "^$DOMAIN" /etc/caddy/Caddyfile 2>/dev/null; then
        echo "Caddy already serves $DOMAIN"
    else
        printf '\n%s\n' "$BLOCK" >>/etc/caddy/Caddyfile
        systemctl reload caddy
        echo "added $DOMAIN to /etc/caddy/Caddyfile"
    fi
elif command -v nginx >/dev/null 2>&1; then
    cat <<EOF
nginx is installed: add a server block like this one (and get a certificate,
e.g. with certbot --nginx -d $DOMAIN), then reload nginx:

server {
    listen 80;
    server_name $DOMAIN;
    location / {
        proxy_pass http://127.0.0.1:$PORT;
        proxy_set_header Host \$host;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }
}
EOF
else
    apt-get update -qq
    apt-get install -y -qq caddy
    printf '%s\n' "$BLOCK" >/etc/caddy/Caddyfile
    systemctl reload caddy || systemctl restart caddy
    echo "installed Caddy for $DOMAIN (it gets the certificate when the DNS points here)"
fi

step "done"
cat <<EOF
Now, in GitHub (Settings → Environments → docs-site → secrets):
  DOCS_SSH_HOST         this server's address
  DOCS_SSH_KEY          the private half of DEPLOY_KEY
  DOCS_SSH_KNOWN_HOSTS  the output of: ssh-keyscan -t ed25519 <this server's address>
and run the "Docs site" workflow (Actions tab) for the first deploy.
EOF
