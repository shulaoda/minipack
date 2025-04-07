// MULTIPLE ENTRY MODULES
import hyperCube from './hyper-cube.js';

console.log(hyperCube(5), import('./square.js').then((v) => console.log(v)));
