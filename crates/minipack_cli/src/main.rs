mod args;
mod types;

use std::time::Instant;

use ansi_term::Colour;
use args::{EnhanceArgs, InputArgs, OutputArgs};
use clap::Parser;

use minipack::{Bundler, BundlerOptions, Output};

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

fn print_output_assets(outputs: Vec<Output>) {
  let mut left = 0;
  let mut right = 0;

  let mut assets = Vec::with_capacity(outputs.len());

  for output in outputs {
    let asset = match output {
      minipack::Output::Chunk(output) => {
        let size = format!("{:.2}", output.code.len() as f64 / 1024.0);

        if size.len() > right {
          right = size.len();
        }

        if output.filename.len() > left {
          left = output.filename.len()
        }

        (output.filename, Colour::Cyan, size, true)
      }
      minipack::Output::Asset(output) => {
        let size = format!("{:.2}", output.source.len() as f64 / 1024.0);

        if size.len() > right {
          right = size.len();
        }

        if output.filename.len() > left {
          left = output.filename.len()
        }

        (output.filename, Colour::Green, size, false)
      }
    };

    assets.push(asset);
  }

  let dim = Colour::White.dimmed();

  for (filename, color, size, is_chunk) in assets {
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

  let InputArgs { cwd, input, platform } = args.input;

  let input = input.map(|files| {
    files.into_iter().map(|file| file.as_os_str().to_string_lossy().into_owned().into()).collect()
  });

  let options = BundlerOptions {
    cwd,
    input,
    platform: platform.map(Into::into),
    dir: args.output.dir,
    file: args.output.file,
    format: args.output.format.map(Into::into),
    exports: args.output.exports.map(Into::into),
    entry_filenames: args.output.entry_filenames,
    chunk_filenames: args.output.chunk_filenames,
    minify: args.enhance.minify,
    target: args.enhance.target.map(Into::into),
    shim_missing_exports: args.enhance.shim_missing_exports,
    resolve: None,
  };

  let start = Instant::now();
  let mut bundler = Bundler::new(options);

  match bundler.write().await {
    Ok(output) => {
      let elapsed = format!("{:.2} ms", start.elapsed().as_secs_f64() * 1000.0);

      // Print warnings
      for warning in output.warnings {
        println!("{} {}", Colour::Yellow.paint("Warning:"), warning);
      }

      // Print output assets
      if !output.assets.is_empty() {
        print_output_assets(output.assets);
      }

      println!("\n{} Finished in {}", Colour::Green.paint("✔"), Colour::White.bold().paint(elapsed))
    }
    Err(errors) => {
      for error in &*errors {
        println!("{} {}", Colour::Red.paint("Error:"), error);
      }
    }
  }
}
