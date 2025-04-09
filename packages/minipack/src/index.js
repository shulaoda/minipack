import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

export function minipack(args, options) {
  const binPath = path.resolve(__dirname, '../bin/minipack.exe');
  const result = spawnSync(binPath, args, options);

  if (result.error) {
    throw result.error;
  }

  process.exitCode = result.status;
}
