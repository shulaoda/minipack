import type { BuildOptions } from 'esbuild';
import type { RollupOptions } from 'rollup';

type BundlerName = 'rollup' | 'esbuild';

export interface BenchSuite {
  title: string;
  inputs: string[];
  rollupOptions?: RollupOptions;
  esbuildOptions?: BuildOptions;
}
