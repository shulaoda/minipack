import chalk from 'chalk';
import lodash from 'lodash';
import * as tinyBench from 'tinybench';

/**
 * @param {string} name
 * @param {(bench: import('tinybench').Bench) => void} collectBenches
 * @param {import('tinybench').Options} [options]
 */
export function group(name, collectBenches, options) {
  const bench = new tinyBench.Bench(options);
  collectBenches(bench);
  return {
    async run() {
      await bench.run();

      if (!bench.results) {
        throw new Error('No benchmark results');
      }

      return {
        raw: bench.results,
        display() {
          console.log(`${chalk.yellow(name)}:`);
          let resultsForDisplay = bench.tasks.map((task) => {
            if (!task.result) {
              throw new Error(
                `No benchmark result found for ${name} ${task.name}`,
              );
            }

            return {
              name: task.name,
              mean: task.result.mean,
            };
          });

          resultsForDisplay = lodash.sortBy(resultsForDisplay, 'mean');

          // Show which benchmark is the fastest
          resultsForDisplay.forEach((result, idx) => {
            let content = `  ${result.name}: ${result.mean.toFixed(2)}ms`;
            if (idx === 0) {
              content = chalk.green(`${content} (fastest)`);
            }
            console.log(content);
          });

          // Show how much faster it is compared to others
          console.log(
            `${chalk.blueBright('Summary')}${chalk.gray(`(${name})`)}:`,
          );
          const [fastest, ...others] = resultsForDisplay;
          const fastestMean = fastest.mean;
          if (fastest == null || others.length === 0) {
            return;
          }
          console.log(`  ${fastest.name} is`);
          for (const other of others) {
            const times = (other.mean / fastestMean).toFixed(2);
            // Example: xxxx is 1.5 times faster than yyyy
            console.log(
              `  - ${chalk.green(times)} times faster than ${other.name}`,
            );
          }
        },
      };
    },
  };
}
