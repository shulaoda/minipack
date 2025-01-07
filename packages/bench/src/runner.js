import path from 'node:path'
import * as rollup from 'rollup'
import * as esbuild from 'esbuild'
import commonjs from '@rollup/plugin-commonjs'
import { nodeResolve } from '@rollup/plugin-node-resolve'
import { PROJECT_ROOT } from './utils.js'

/**
 * Rollup Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runRollup(suite) {
  const { output: outputOptions = {}, ...inputOptions } =
    suite.rollupOptions ?? {}
  const build = await rollup.rollup({
    input: suite.inputs,
    onwarn() { },
    plugins: [
      nodeResolve({
        exportConditions: ['import'],
        mainFields: ['module', 'browser', 'main'],
      }),
      // @ts-ignore
      commonjs(),
    ],
    ...inputOptions,
  })
  await build.write({
    dir: path.join(PROJECT_ROOT, `./dist/rollup/${suite.title}`),
    ...outputOptions,
  })
}

/**
 * Esbuild Bench
 * @param {import('./types.js').BenchSuite} suite
 */
export async function runEsbuild(suite) {
  const options = suite.esbuildOptions ?? {}
  await esbuild.build({
    platform: 'node',
    entryPoints: suite.inputs,
    bundle: true,
    outdir: path.join(PROJECT_ROOT, `./dist/esbuild/${suite.title}`),
    write: true,
    format: 'esm',
    splitting: true,
    ...options,
  })
}
