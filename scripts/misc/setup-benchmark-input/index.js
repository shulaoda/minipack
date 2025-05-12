import 'zx/globals';
import fsExtra from 'fs-extra';

async function cloneThreeJsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/threejs')) {
    fsExtra.ensureDirSync('./tmp/github');
    await $`git clone --branch r108 --depth 1 https://github.com/mrdoob/three.js.git ./tmp/github/threejs`;
  } else {
    console.log('[skip] three.js already cloned');
  }

  if (fsExtra.existsSync('./tmp/bench/threejs')) {
    console.log('[skip] setup threejs already');
  } else {
    console.log('Setup `threejs` in tmp/bench');
    fsExtra.copySync('./tmp/github/threejs', './tmp/bench/threejs');

    fsExtra.writeFileSync(
      './tmp/bench/threejs/entry.js',
      `import * as threejs from './src/Three.js'; export { threejs }`,
    );
  }
}

async function cloneDayjsIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/dayjs')) {
    fsExtra.ensureDirSync('./tmp/github');
    await $`git clone https://github.com/shulaoda/dayjs.git ./tmp/github/dayjs`;
  } else {
    console.log('[skip] dayjs already cloned');
  }

  if (fsExtra.existsSync('./tmp/bench/dayjs')) {
    console.log('[skip] setup dayjs already');
  } else {
    console.log('Setup `dayjs` in tmp/bench');
    fsExtra.copySync('./tmp/github/dayjs', './tmp/bench/dayjs');
  }
}

async function cloneQueryStringIfNotExists() {
  if (!fsExtra.existsSync('./tmp/github/query-string')) {
    fsExtra.ensureDirSync('./tmp/github');
    await $`git clone https://github.com/sindresorhus/query-string.git ./tmp/github/query-string`;
  } else {
    console.log('[skip] query-string already cloned');
  }

  if (fsExtra.existsSync('./tmp/bench/query-string')) {
    console.log('[skip] setup query-string already');
  } else {
    console.log('Setup `query-string` in tmp/bench');
    fsExtra.copySync('./tmp/github/query-string', './tmp/bench/query-string');
  }
}

await cloneDayjsIfNotExists();
await cloneThreeJsIfNotExists();
await cloneQueryStringIfNotExists();
