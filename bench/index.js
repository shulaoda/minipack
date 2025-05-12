import nodePath from 'node:path';
import nodeUtil from 'node:util';
import * as bencher from './src/bencher.js';
import {
  runEsbuild,
  runMinipack,
  runRollup,
  runWebpack,
} from './src/runner.js';
import { REPO_ROOT } from './src/utils.js';

/**
 * @type {import('./src/types.js').BenchSuite[]}
 */
const suites = [
  {
    title: 'query-string',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/query-string/index.js')],
    esbuildOptions: {
      external: ['decode-uri-component', 'filter-obj', 'split-on-first'],
    },
  },
  {
    title: 'dayjs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/dayjs/src/index.js')],
  },
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/threejs/entry.js')],
  },
];

console.log(
  nodeUtil.inspect(suites, { depth: null, colors: true, showHidden: false }),
);

for (const suite of suites) {
  const group = bencher.group(suite.title, (bench) => {
    bench.add('rollup', async () => {
      await runRollup(suite);
    });
    bench.add('webpack', async () => {
      await runWebpack(suite);
    });
    bench.add('esbuild', async () => {
      await runEsbuild(suite);
    });
    bench.add('minipack', async () => {
      await runMinipack(suite);
    });
  });

  const result = await group.run();
  result.display();
}
