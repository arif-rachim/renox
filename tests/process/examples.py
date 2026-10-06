#!/usr/bin/env python3
"""Every example binary as a real process (#270): migrate, seed twice, serve,
then ask for every GET route without parameters (from `route:list`) as a
guest; a 5xx anywhere fails. tests/process/run.sh builds the binaries first.

  python3 tests/process/examples.py              # every example
  python3 tests/process/examples.py shop grid    # some
"""

import base64
import os
import re
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))

# Package (binary) → directory. examples/postgres is PostgreSQL-only.
EXAMPLES = {
    "hello": "hello",
    "crud": "crud",
    "api": "api",
    "jobs": "jobs",
    "uploads": "uploads",
    "shop": "shop",
    "htmx-recipes": "htmx-recipes",
    "relations": "relations",
    "grid": "grid",
    "backoffice": "backoffice",
    "teams": "teams",
    "admin": "admin",
    "billing": "billing",
    "webhooks": "webhooks",
    "fields": "fields",
}


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class Guest(urllib.request.HTTPRedirectHandler):
    """Doesn't follow redirects: a 302 to /login is an answer, not an error."""

    def redirect_request(self, *args, **kwargs):
        return None


def get(url):
    opener = urllib.request.build_opener(Guest)
    try:
        with opener.open(url, timeout=15) as res:
            return res.status
    except urllib.error.HTTPError as error:
        return error.code
    except (urllib.error.URLError, ConnectionError, OSError):
        return 0  # not listening (yet)


def routes(exe, cwd, env):
    """GET paths without parameters, on the default host, from `route:list`."""
    listing = subprocess.run([exe, "route:list"], cwd=cwd, env=env, capture_output=True, text=True, timeout=60)
    if listing.returncode != 0:
        raise AssertionError(f"route:list failed:\n{listing.stdout}\n{listing.stderr}")
    lines = listing.stdout.splitlines()
    with_domain = bool(lines) and lines[0].startswith("DOMAIN")
    paths = []
    for line in lines[1:]:
        cells = re.split(r"\s{2,}", line.strip())
        if with_domain and not line.startswith(" ") and not cells[0].startswith(("GET", "POST", "PUT", "PATCH", "DELETE", "*")):
            continue  # a route of another host
        for i, cell in enumerate(cells):
            if cell.startswith("/"):
                method = cells[i - 1] if i else ""
                if "GET" in method and "{" not in cell and "*" not in cell:
                    paths.append(cell)
                break
    return sorted(set(paths))


def smoke(binary, directory):
    exe = os.path.join(TARGET, "debug", binary)
    cwd = os.path.join(ROOT, "examples", directory)
    data = tempfile.mkdtemp(prefix=f"renox-{binary}-")
    port = free_port()
    env = {
        "PATH": os.environ["PATH"],
        "HOME": os.environ.get("HOME", "/tmp"),
        "APP_ENV": "local",
        "APP_DEBUG": "true",
        "APP_HOST": "127.0.0.1",
        "APP_PORT": str(port),
        "APP_URL": f"http://127.0.0.1:{port}",
        "APP_KEY": "base64:" + base64.b64encode(os.urandom(32)).decode(),
        "DATABASE_URL": f"sqlite://{data}/app.db",
        "STORAGE_PATH": os.path.join(data, "storage"),
        "MAIL_MAILER": "log",
        "QUEUE_WORKERS": "1",
        "RUST_LOG": "warn",
    }
    proc = None
    try:
        for command in ("migrate", "db:seed", "db:seed"):
            run = subprocess.run([exe, command], cwd=cwd, env=env, capture_output=True, text=True, timeout=300)
            if run.returncode != 0:
                raise AssertionError(f"{command} failed:\n{run.stdout}\n{run.stderr}")
        out = open(os.path.join(data, "serve.txt"), "w+")
        proc = subprocess.Popen([exe, "serve"], cwd=cwd, env=env, stdout=out, stderr=subprocess.STDOUT)
        url = f"http://127.0.0.1:{port}"
        until = time.time() + 30
        while get(f"{url}/health") != 200:
            if proc.poll() is not None or time.time() > until:
                out.seek(0)
                raise AssertionError(f"serve didn't answer:\n{out.read()}")
            time.sleep(0.2)
        broken = []
        paths = routes(exe, cwd, env)
        for path in paths:
            status = get(f"{url}{path}")
            if status >= 500:
                broken.append(f"{path} → {status}")
        if broken:
            out.seek(0)
            log = out.read()[-3000:]
            raise AssertionError(f"{len(broken)} of {len(paths)} pages failed: {broken}\n{log}")
        return len(paths)
    finally:
        if proc is not None:
            proc.send_signal(signal.SIGTERM)
            try:
                proc.wait(timeout=15)
            except subprocess.TimeoutExpired:
                proc.kill()
        shutil.rmtree(data, ignore_errors=True)


def main():
    names = sys.argv[1:] or list(EXAMPLES)
    failed = []
    for name in names:
        started = time.time()
        try:
            count = smoke(name, EXAMPLES[name])
            print(f"ok   {name}: {count} pages ({time.time() - started:.1f}s)")
        except Exception as error:  # noqa: BLE001
            failed.append(name)
            print(f"FAIL {name}: {error}")
    if failed:
        print(f"{len(failed)} failed: {', '.join(failed)}")
        sys.exit(1)
    print(f"all {len(names)} examples answered")


if __name__ == "__main__":
    main()
