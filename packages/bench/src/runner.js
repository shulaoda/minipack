import commonjs from '@rollup/plugin-commonjs';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import { build as esbuild } from 'esbuild';
import { minipack } from 'minipack';
import path from 'node:path';
import { rollup } from 'rollup';
import { PROJECT_ROOT } from './utils.js';

/**
 * Rollup Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runRollup(suite) {
  const { output: outputOptions = {}, ...inputOptions } = suite.rollupOptions ??
    {};
  const build = await rollup({
    input: suite.inputs,
    onwarn() {},
    plugins: [
      nodeResolve({
        exportConditions: ['import'],
        mainFields: ['module', 'browser', 'main'],
      }),
      // @ts-ignore
      commonjs(),
    ],
    ...inputOptions,
  });
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${suite.title}`),
    ...outputOptions,
  });
}

/**
 * Esbuild Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runEsbuild(suite) {
  const options = suite.esbuildOptions ?? {};
  await esbuild({
    platform: 'node',
    entryPoints: suite.inputs,
    bundle: true,
    outdir: path.join(PROJECT_ROOT, `./dist/esbuild/${suite.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    ...options,
  });
}

/**
 * Minipack Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runMinipack(suite) {
  const outdir = path.join(PROJECT_ROOT, `./dist/minipack/${suite.title}`);
  minipack([
    '--platform=node',
    ...suite.inputs.map((path) => `--input=${path}`),
    `--dir=${outdir}`,
  ], {
    stdio: undefined,
  });
}
