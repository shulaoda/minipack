use std::{
  fs::File,
  io::Write,
  path::PathBuf,
  process::{Command, Stdio},
  time::{Duration, Instant},
};

#[derive(Debug)]
struct BenchmarkResult {
  name: String,
  average_time: Duration,
  peak_memory_kb: u64,
}

fn run_tool(name: &str, cmd: &str, args: &[&str], runs: usize) -> BenchmarkResult {
  let mut total_time = Duration::ZERO;
  let mut peak_memory = 0;

  for _ in 0..runs {
    let time_start = Instant::now();

    let child = Command::new("/usr/bin/time")
      .arg("-v")
      .arg(cmd)
      .args(args)
      .stderr(Stdio::piped())
      .stdout(Stdio::null())
      .spawn()
      .expect("failed to spawn process");

    let output = child.wait_with_output().expect("failed to wait on child");
    let elapsed = time_start.elapsed();
    total_time += elapsed;

    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines() {
      if line.contains("Maximum resident set size") {
        if let Some(kb_str) = line.split(':').nth(1) {
          let kb: u64 = kb_str.trim().parse().unwrap_or(0);
          peak_memory = peak_memory.max(kb);
        }
      }
    }
  }

  BenchmarkResult {
    name: name.to_string(),
    average_time: total_time / (runs as u32),
    peak_memory_kb: peak_memory,
  }
}

fn main() {
  let benchmark_root = PathBuf::from(env!("WORKSPACE_DIR")).join("tmp/bench/");
  let tools = vec![
    // ("rollup", "three.js", "three/entry.js", &["input.js"][..]),
    // ("webpack", "three.js", "three/entry.js", &["input.js"][..]),
    // ("esbuild", "three.js", "three/entry.js", &["input.js"][..]),
    ("minipack", "three.js", "three/entry.js", &["input.js"][..]),
  ];

  let runs = 5;
  let mut results = Vec::new();

  for (tool, name, path, args) in tools {
    println!("Benchmarking {}...", tool);
    let result = run_tool(name, path, args, runs);
    results.push(result);
  }

  let mut file = File::create("benchmark_results.csv").unwrap();
  writeln!(file, "Tool,Average Time (ms),Peak Memory (KB)").unwrap();
  for result in results {
    writeln!(
      file,
      "{},{:.2},{:.0}",
      result.name,
      result.average_time.as_secs_f64() * 1000.0,
      result.peak_memory_kb
    )
    .unwrap();
  }

  println!("✅ 基准测试完成，结果已保存至 benchmark_results.csv");
}
