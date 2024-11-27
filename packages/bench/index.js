import nodePath from 'node:path'
import nodeUtil from 'node:util'
import * as bencher from './src/bencher.js'
import { runRollup, runEsbuild } from './src/runner.js'
import { PROJECT_ROOT, REPO_ROOT } from './src/utils.js'

/**
 * @type {import('./src/types.js').BenchSuite[]}
 */
const suites = [
  {
    title: 'threejs',
    inputs: [nodePath.join(REPO_ROOT, './tmp/bench/three/entry.js')],
  },
  {
    title: 'vue-stack',
    inputs: [nodePath.join(PROJECT_ROOT, 'vue-entry.js')],
  },
  {
    title: 'react-stack',
    inputs: ['react', 'react-dom'],
  },
]

console.log(
  nodeUtil.inspect(suites, { depth: null, colors: true, showHidden: false }),
)

for (const suite of suites) {
  const group = bencher.group(suite.title, (bench) => {
    bench.add('rollup', async () => {
      await runRollup(suite)
    })
    bench.add('esbuild', async () => {
      await runEsbuild(suite)
    })
  })

  const result = await group.run()
  result.display()
}
