import cube from './cube.js';
import square from './square.js';

// This is only imported by one entry module and
// shares a chunk with that module
export default function hyperCube(x) {
  return cube(x) * square(x);
}
