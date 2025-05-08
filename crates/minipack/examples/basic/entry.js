// MULTIPLE ENTRY MODULES
import hyperCube, { b } from './hyper-cube.js';

const a = 1;

console.log(b, hyperCube(5), import('./square.js').then((v) => console.log(v)));
