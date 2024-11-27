import type { RollupOptions } from 'rollup'
import type { BuildOptions } from 'esbuild'

type BundlerName = 'rollup' | 'esbuild'

export interface BenchSuite {
  title: string
  inputs: string[]
  rollupOptions?: RollupOptions
  esbuildOptions?: BuildOptions
}
