// MULTIPLE ENTRY MODULES
import hyperCube from './hyper-cube.js';

const a = 1;

console.log(hyperCube(5), import('./square.js').then((v) => console.log(v)));
