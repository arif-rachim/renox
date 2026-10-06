#!/usr/bin/env python3
"""Process e2e (#270): the app binary and `rnx` as real processes.

Uses the browser fixture (tests/browser/fixture), which tests/process/run.sh
builds first. Each check starts what it needs on a free port with its own
database and stops it again; nothing is left running.

  python3 tests/process/process.py            # every check
  python3 tests/process/process.py signals queue
"""

import base64
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.request

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
TARGET = os.environ.get("CARGO_TARGET_DIR", os.path.join(ROOT, "target"))
FIXTURE = os.path.join(TARGET, "debug", "browser-fixture")
FIXTURE_DIR = os.path.join(ROOT, "tests", "browser", "fixture")
RNX = os.path.join(TARGET, "debug", "rnx")


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


class App:
    """The fixture with its own port, database, storage and log file."""

    def __init__(self, **extra):
        self.dir = tempfile.mkdtemp(prefix="renox-process-")
        self.port = free_port()
        self.url = f"http://127.0.0.1:{self.port}"
        self.log = os.path.join(self.dir, "fixture.log")
        self.env = {
            "PATH": os.environ["PATH"],
            "HOME": os.environ.get("HOME", "/tmp"),
            "APP_ENV": "local",
            "APP_DEBUG": "true",
            "APP_HOST": "127.0.0.1",
            "APP_PORT": str(self.port),
            "APP_KEY": "base64:" + base64.b64encode(os.urandom(32)).decode(),
            "DATABASE_URL": f"sqlite://{self.dir}/app.db",
            "STORAGE_PATH": os.path.join(self.dir, "storage"),
            "VIEWS_PATH": "views",
            "QUEUE_WORKERS": "0",
            "SCHEDULER": "false",
            "FIXTURE_LOG": self.log,
            "RUST_LOG": "info",
        }
        self.env.update(extra)

    def run(self, *args, stdin=None, check=True, env=None):
        """Runs a command of the binary to the end."""
        result = subprocess.run(
            [FIXTURE, *args],
            cwd=FIXTURE_DIR,
            env={**self.env, **(env or {})},
            input=stdin,
            capture_output=True,
            text=True,
            timeout=120,
        )
        if check and result.returncode != 0:
            raise AssertionError(f"{args} failed ({result.returncode}):\n{result.stdout}\n{result.stderr}")
        return result

    def start(self, *args, prefix=(), env=None):
        """Starts a long-running command; returns the process."""
        out = open(os.path.join(self.dir, f"out-{len(os.listdir(self.dir))}.txt"), "w+")
        proc = subprocess.Popen(
            [*prefix, FIXTURE, *args],
            cwd=FIXTURE_DIR,
            env={**self.env, **(env or {})},
            stdout=out,
            stderr=subprocess.STDOUT,
            text=True,
        )
        proc.output = out
        return proc

    def wait_health(self, proc=None, timeout=30):
        until = time.time() + timeout
        while time.time() < until:
            if proc is not None and proc.poll() is not None:
                raise AssertionError(f"it exited ({proc.returncode}):\n{output(proc)}")
            try:
                with urllib.request.urlopen(f"{self.url}/health", timeout=2) as res:
                    if res.status == 200:
                        return
            except (urllib.error.URLError, ConnectionError, OSError):
                pass
            time.sleep(0.1)
        raise AssertionError(f"no /health within {timeout}s")

    def noted(self):
        try:
            with open(self.log) as f:
                return [line.strip() for line in f]
        except FileNotFoundError:
            return []

    def close(self):
        shutil.rmtree(self.dir, ignore_errors=True)


def output(proc):
    proc.output.flush()
    proc.output.seek(0)
    return proc.output.read()


def stop(proc, sig=signal.SIGTERM, timeout=15):
    proc.send_signal(sig)
    try:
        return proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        proc.kill()
        raise AssertionError(f"it didn't stop within {timeout}s after {sig.name}:\n{output(proc)}")


def get(url, timeout=10):
    with urllib.request.urlopen(url, timeout=timeout) as res:
        return res.status, res.read().decode()


def wait_until(check, timeout=20, what="the condition"):
    until = time.time() + timeout
    while time.time() < until:
        if check():
            return
        time.sleep(0.1)
    raise AssertionError(f"timed out waiting for {what}")


# ---------- checks ----------


def signals():
    """serve stops on SIGTERM and Ctrl-C (SIGINT), finishing the request in flight."""
    for sig in (signal.SIGTERM, signal.SIGINT):
        app = App()
        try:
            app.run("migrate")
            proc = app.start("serve")
            app.wait_health(proc)
            answer = {}

            def slow():
                try:
                    answer["value"] = get(f"{app.url}/pause")
                except Exception as error:  # noqa: BLE001
                    answer["value"] = error

            thread = threading.Thread(target=slow)
            thread.start()
            time.sleep(0.4)  # the request is running
            code = stop(proc, sig)
            thread.join(10)
            assert code == 0, f"{sig.name}: exit code {code}\n{output(proc)}"
            assert answer.get("value") == (200, "finished"), f"{sig.name}: the request in flight got {answer}"
            assert "Renox stopped" in output(proc), output(proc)
        finally:
            app.close()


def socket_activation():
    """With systemd's socket (LISTEN_FDS) the app serves on it."""
    launcher = shutil.which("systemd-socket-activate")
    if not launcher:
        print("  (skipped: systemd-socket-activate isn't installed)")
        return
    app = App()
    try:
        app.run("migrate")
        setenv = [f"--setenv={k}" for k in app.env]
        proc = app.start("serve", prefix=[launcher, "-l", f"127.0.0.1:{app.port}", *setenv])
        app.wait_health(proc)
        assert get(f"{app.url}/")[0] == 200
        assert stop(proc) == 0
        assert "using the socket systemd passed" in output(proc), output(proc)
    finally:
        app.close()


def queue():
    """queue:work --once drains; a long-running worker pool stops cleanly."""
    app = App()
    try:
        app.run("migrate")
        app.run("jobs:push", "3")
        result = app.run("queue:work", "--once", "--queue", "default,mail")
        assert "Ran 3 job(s)" in result.stdout, result.stdout
        assert sorted(app.noted()) == ["job 1", "job 2", "job 3"], app.noted()

        worker = app.start("queue:work", "--workers", "2")
        time.sleep(0.5)
        app.run("jobs:push", "2")
        wait_until(lambda: len(app.noted()) == 5, what="the worker to run two more jobs")
        assert stop(worker) == 0, output(worker)
    finally:
        app.close()


def scheduler():
    """schedule:work runs a task each second; two of them on one database run it once each time."""
    app = App(FIXTURE_TICK="1")
    try:
        app.run("migrate")
        first = app.start("schedule:work")
        second = app.start("schedule:work")
        time.sleep(4.5)
        assert stop(first) == 0 and stop(second) == 0
        ticks = app.noted().count("tick")
        assert 2 <= ticks <= 6, f"{ticks} ticks in about four seconds from two schedulers"
    finally:
        app.close()


def logs():
    """LOG_FORMAT=json, LOG_FILE, and LOG_FILE that can't be opened."""
    # The request's trace (tower_http, debug) carries the request span.
    app = App(LOG_FORMAT="json", RUST_LOG="info,tower_http=debug")
    try:
        app.run("migrate")
        proc = app.start("serve")
        app.wait_health(proc)
        get(f"{app.url}/")
        time.sleep(0.3)
        stop(proc)
        lines = [line for line in output(proc).splitlines() if line.strip()]
        parsed = [json.loads(line) for line in lines]
        assert parsed, "no log lines"
        assert any(entry.get("span", {}).get("uri") == "/" for entry in parsed), lines[-5:]
    finally:
        app.close()

    app = App()
    try:
        app.run("migrate")
        path = os.path.join(app.dir, "renox.log")
        proc = app.start("serve", env={"LOG_FILE": path})
        app.wait_health(proc)
        stop(proc)
        with open(path) as f:
            text = f.read()
        assert "Renox stopped" in text and "\x1b[" not in text, text
    finally:
        app.close()

    app = App()
    try:
        app.run("migrate")
        proc = app.start("serve", env={"LOG_FILE": "/nonexistent-dir/renox.log"})
        app.wait_health(proc)
        stop(proc)
        assert "logging to stdout" in output(proc), output(proc)
    finally:
        app.close()


def app_key():
    """Production refuses to start without APP_KEY."""
    app = App(APP_ENV="production", APP_DEBUG="false")
    try:
        del app.env["APP_KEY"]
        result = app.run("serve", check=False)
        assert result.returncode != 0, result.stdout
        assert "APP_KEY" in result.stdout + result.stderr, result.stdout + result.stderr
    finally:
        app.close()


def shell():
    """db:shell reads statements from a pipe."""
    app = App()
    try:
        app.run("migrate")
        result = app.run("db:shell", stdin=".tables\nSELECT 41 + 1 AS answer;\n.quit\n")
        assert "jobs" in result.stdout, result.stdout
        assert "answer" in result.stdout and "42" in result.stdout, result.stdout
    finally:
        app.close()


def prompts():
    """renox::prompt: answers read in turn from a pipe (a wrong one is an
    error there, for scripts); in a terminal a wrong one is asked again and
    a secret isn't echoed."""
    app = App()
    try:
        result = app.run("ask:me", stdin="Ana\ny\nlarge\ns3cret\n")
        assert "answers: Ana | true | large | 6 characters" in result.stdout, result.stdout + result.stderr
        result = app.run("ask:me", stdin="Ana\nmaybe\n", check=False)
        assert result.returncode != 0
        assert '"maybe" isn\'t yes or no' in result.stdout + result.stderr, result.stdout + result.stderr

        script = shutil.which("script")
        if not script:
            print("  (terminal part skipped: `script` isn't installed)")
            return
        # A pseudo-terminal through `script`: each answer goes when asked.
        command = f"{FIXTURE} ask:me"
        proc = subprocess.Popen(
            [script, "-qec", command, "/dev/null"],
            cwd=FIXTURE_DIR,
            env=app.env,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        for answer in ["Ana", "maybe", "y", "medium", "large", "s3cret"]:
            time.sleep(0.4)
            proc.stdin.write(answer.encode() + b"\n")
            proc.stdin.flush()
        out, _ = proc.communicate(timeout=30)
        text = out.decode(errors="replace")
        assert "Answer yes or no." in text, text
        assert "Pick one of the options." in text, text
        assert "answers: Ana | true | large | 6 characters" in text, text
        assert "s3cret" not in text, f"the secret was echoed:\n{text}"
    finally:
        app.close()


def rnx_serve():
    """rnx serve: builds and migrates, restarts on a change, keeps the old app on a failed build."""
    if not os.path.exists(RNX):
        raise AssertionError(f"{RNX} isn't built")
    work = tempfile.mkdtemp(prefix="renox-rnx-serve-")
    try:
        subprocess.run([RNX, "new", "watched", "--renox-path", ROOT], cwd=work, check=True, capture_output=True, text=True, timeout=300)
        app_dir = os.path.join(work, "watched")
        port = free_port()
        env = {
            **os.environ,
            "APP_PORT": str(port),
            "APP_HOST": "127.0.0.1",
            "DATABASE_URL": f"sqlite://{work}/watched.db",
            "QUEUE_WORKERS": "0",
            "SCHEDULER": "false",
        }
        out = open(os.path.join(work, "rnx.txt"), "w+")
        proc = subprocess.Popen([RNX, "serve"], cwd=app_dir, env=env, stdout=out, stderr=subprocess.STDOUT, text=True)
        proc.output = out
        url = f"http://127.0.0.1:{port}"
        app = App()
        app.url = url
        app.wait_health(proc, timeout=600)
        # Reading files (an OPEN event) restarts nothing.
        with open(os.path.join(app_dir, "src", "lib.rs")) as f:
            f.read()
        time.sleep(2)
        assert "change detected" not in output(proc), output(proc)
        # A source change: rebuilt and restarted.
        lib = os.path.join(app_dir, "src", "lib.rs")
        with open(lib, "a") as f:
            f.write("\n// changed by tests/process\n")
        wait_until(lambda: "change detected, rebuilding" in output(proc), timeout=60, what="the change to be seen")
        app.wait_health(proc, timeout=600)
        # A build that fails: the app that runs keeps answering.
        with open(lib, "a") as f:
            f.write("\nthis is not rust\n")
        wait_until(
            lambda: "build failed; the previous version keeps running" in output(proc),
            timeout=600,
            what="the failed build",
        )
        assert get(f"{url}/health")[0] == 200, "the old app stopped"
        stop(proc, signal.SIGINT, timeout=30)
        app.close()
    finally:
        shutil.rmtree(work, ignore_errors=True)


CHECKS = {
    "signals": signals,
    "socket": socket_activation,
    "queue": queue,
    "scheduler": scheduler,
    "logs": logs,
    "app_key": app_key,
    "shell": shell,
    "prompts": prompts,
    "rnx_serve": rnx_serve,
}


def main():
    names = sys.argv[1:] or [n for n in CHECKS if n != "rnx_serve" or os.environ.get("RNX_SERVE") == "1"]
    failed = []
    for name in names:
        started = time.time()
        try:
            CHECKS[name]()
            print(f"ok   {name} ({time.time() - started:.1f}s)")
        except Exception as error:  # noqa: BLE001
            failed.append(name)
            print(f"FAIL {name}: {error}")
    if failed:
        print(f"{len(failed)} failed: {', '.join(failed)}")
        sys.exit(1)
    print(f"all {len(names)} passed")


if __name__ == "__main__":
    main()
