mod args;
mod types;

use std::time::Instant;

use ansi_term::Colour;
use args::{EnhanceArgs, InputArgs, OutputArgs};
use clap::Parser;

use minipack::{Bundler, BundlerOptions, OutputAsset};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Commands {
  #[clap(flatten)]
  input: InputArgs,

  #[clap(flatten)]
  output: OutputArgs,

  #[clap(flatten)]
  enhance: EnhanceArgs,
}

fn print_output_assets(outputs: Vec<OutputAsset>) {
  let mut left = 0;
  let mut right = 0;

  let mut assets = Vec::with_capacity(outputs.len());

  for output in outputs {
    let size = format!("{:.2}", output.content.len() as f64 / 1024.0);

    if size.len() > right {
      right = size.len();
    }

    if output.filename.len() > left {
      left = output.filename.len()
    }

    assets.push((output.filename, size, true));
  }

  let dim = Colour::White.dimmed();
  let color = Colour::Cyan;

  for (filename, size, is_chunk) in assets {
    let asset_type = if is_chunk { "chunk" } else { "asset" };
    let filename_len = filename.len();

    println!(
      "{}{}{:left$} {}{}{:right$}{} kB",
      dim.paint("<DIR>/"),
      color.paint(filename),
      "",
      dim.paint(asset_type),
      dim.paint(" │ size: "),
      "",
      size,
      left = left - filename_len,
      right = right - size.len()
    )
  }
}

#[tokio::main]
async fn main() {
  let args = Commands::parse();
  let InputArgs { input, platform } = args.input;
  let input = input.map(|files| files.iter().map(|p| p.to_string_lossy().into()).collect());

  let mut bundler = Bundler::new(BundlerOptions {
    cwd: None,
    input,
    platform: platform.map(Into::into),
    dir: args.output.dir,
    format: args.output.format.map(Into::into),
    entry_filenames: args.output.entry_filenames,
    chunk_filenames: args.output.chunk_filenames,
    minify: Some(args.enhance.minify),
  });

  let start = Instant::now();
  match bundler.build(true).await {
    Ok(output) => {
      if !args.enhance.silent {
        // Print warnings
        for warning in output.warnings {
          println!("{} {}", Colour::Yellow.paint("Warning:"), warning);
        }

        // Print output assets
        if !output.assets.is_empty() {
          print_output_assets(output.assets);
        }
      }

      let elapsed = format!("{:.2} ms", start.elapsed().as_secs_f64() * 1000.0);
      println!("\n{} Finished in {}", Colour::Green.paint("✔"), Colour::White.bold().paint(elapsed))
    }
    Err(errors) => {
      for error in &*errors {
        println!("{} {}", Colour::Red.paint("Error:"), error);
      }
    }
  }
}
