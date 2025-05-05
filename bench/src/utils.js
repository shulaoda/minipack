import nodePath from 'node:path';
import nodeUrl from 'node:url';

const dirname = nodePath.dirname(nodeUrl.fileURLToPath(import.meta.url));

export const REPO_ROOT = nodePath.join(dirname, '../..');

export const PROJECT_ROOT = nodePath.join(dirname, '..');

/**
 * @param {import('./types.js').BenchSuite} config
 * @returns {import('./types.js').BenchSuite}
 */
export function defineSuite(config) {
  return config;
}
