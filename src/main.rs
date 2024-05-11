//! Command-line application for preloading thumbnails for Windows Explorer.

#[cfg(target_os = "windows")]
mod preloader;

fn main() {
	#[cfg(target_os = "windows")]
	{
		use std::path::PathBuf;

		use clap::Parser;

		/// Performs preload operations on directories
		#[derive(Debug, Parser)]
		#[command(author, version, about, long_about)]
		struct Cli {
			/// Specifies the directory to preload
			#[arg(short, long)]
			dir: PathBuf,
		}

		let cli = Cli::parse();

		match preloader::preload(cli.dir) {
			Ok(()) => println!("Successfully preloaded directory."),
			Err(err) => println!("Failed to preload directory: {}.", err),
		}
	}

	#[cfg(not(target_os = "windows"))]
	{
		println!("Application is only supported on Windows.");
	}
}
