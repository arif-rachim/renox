// Starts a Renox app binary (the fixture or an example) on a free port with
// its own database and storage, migrated (and seeded when asked), and stops
// it again. `tests/browser/run.sh` builds the binaries first.

import { spawn, spawnSync } from 'node:child_process';
import { randomBytes } from 'node:crypto';
import { mkdtempSync, rmSync } from 'node:fs';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), '../../..');
const TARGET = process.env.CARGO_TARGET_DIR || join(ROOT, 'target');

/** A port nobody listens on right now. */
function freePort() {
  return new Promise((resolve, reject) => {
    const server = createServer();
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
    server.on('error', reject);
  });
}

/**
 * Starts `binary` (a package's binary name) from `dir` (relative to the
 * repository). `env` adds or overrides variables, e.g. `{ CSP: 'strict' }`.
 */
export async function start(binary, dir, { env = {}, seed = false } = {}) {
  const port = await freePort();
  const data = mkdtempSync(join(tmpdir(), `renox-${binary}-`));
  const cwd = join(ROOT, dir);
  const exe = join(TARGET, 'debug', binary);
  const vars = {
    PATH: process.env.PATH,
    HOME: process.env.HOME,
    APP_ENV: 'local',
    APP_DEBUG: 'true',
    APP_HOST: '127.0.0.1',
    APP_PORT: String(port),
    APP_URL: `http://127.0.0.1:${port}`,
    APP_KEY: `base64:${randomBytes(32).toString('base64')}`,
    APP_LOCALE: 'en',
    DATABASE_URL: `sqlite://${join(data, 'app.db')}`,
    STORAGE_PATH: join(data, 'storage'),
    MAIL_MAILER: 'log',
    QUEUE_WORKERS: '1',
    RUST_LOG: 'warn',
    ...env,
  };
  for (const command of ['migrate', ...(seed ? ['db:seed'] : [])]) {
    const run = spawnSync(exe, [command], { cwd, env: vars, encoding: 'utf8' });
    if (run.status !== 0) {
      throw new Error(`${binary} ${command} failed:\n${run.stdout}\n${run.stderr}`);
    }
  }
  const url = `http://127.0.0.1:${port}`;
  let log = '';
  let proc;
  const serve = async () => {
    proc = spawn(exe, ['serve'], { cwd, env: vars, stdio: ['ignore', 'pipe', 'pipe'] });
    proc.stdout.on('data', (d) => (log += d));
    proc.stderr.on('data', (d) => (log += d));
    const until = Date.now() + 20_000;
    for (;;) {
      if (proc.exitCode !== null) throw new Error(`${binary} exited:\n${log}`);
      try {
        if ((await fetch(`${url}/health`)).ok) break;
      } catch {}
      if (Date.now() > until) throw new Error(`${binary} didn't answer /health:\n${log}`);
      await new Promise((r) => setTimeout(r, 100));
    }
  };
  const halt = () =>
    new Promise((resolve) => {
      if (proc.exitCode !== null) return resolve();
      proc.on('exit', resolve);
      proc.kill('SIGTERM');
      setTimeout(() => {
        proc.kill('SIGKILL');
        resolve();
      }, 5000);
    });
  await serve();
  return {
    url,
    log: () => log,
    /** Stops the app and starts it again on the same port and database. */
    async restart() {
      await halt();
      await serve();
    },
    async stop() {
      await halt();
      rmSync(data, { recursive: true, force: true });
    },
  };
}

/** The fixture app (tests/browser/fixture). */
export function fixture(options = {}) {
  return start('browser-fixture', 'tests/browser/fixture', {
    ...options,
    env: { VIEWS_PATH: 'views', ...(options.env || {}) },
  });
}
