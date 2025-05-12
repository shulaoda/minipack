import commonjs from '@rollup/plugin-commonjs';
import { nodeResolve } from '@rollup/plugin-node-resolve';
import { build as esbuild } from 'esbuild';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { rollup } from 'rollup';
import webpack from 'webpack';
import { PROJECT_ROOT } from './utils.js';

/**
 * Rollup Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runRollup(suite) {
  const build = await rollup({
    input: suite.inputs,
    onwarn() {},
    plugins: [
      nodeResolve({
        exportConditions: ['import'],
        mainFields: ['module', 'browser', 'main'],
      }),
      commonjs(),
    ],
  });
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${suite.title}`),
  });
}

/**
 * Webpack Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runWebpack(suite) {
  const compiler = webpack({
    entry: suite.inputs,
    target: 'node',
    mode: 'production',
    output: {
      path: path.join(PROJECT_ROOT, `./dist/webpack/${suite.title}`),
    },
    stats: 'none',
  });
  return new Promise((resolve, reject) => {
    compiler.run((err) => {
      if (err) {
        return reject(err);
      }
      resolve();
    });
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
  spawnSync('minipack', [
    '--platform=node',
    ...suite.inputs.map((path) => `--input=${path}`),
    `--dir=${outdir}`,
    '--silent',
  ]);
}
