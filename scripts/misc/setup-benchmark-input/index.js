import 'zx/globals'
import fsExtra from 'fs-extra'

async function cloneThreeJsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/three')) {
    fsExtra.ensureDirSync('./tmp/github')
    await $`git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./tmp/github/three`
  } else {
    console.log('[skip] three.js already cloned')
  }
}

await cloneThreeJsIfNotExists()

await import('./threejs.js')
