//! `ti-extract` command line.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

use clap::{Parser, Subcommand, ValueEnum};
use ti_extract::{ExtractOptions, ImageFormat, Images, Progress};

/// Extract Terra Invicta game data into a stable, build-pinned dataset.
#[derive(Parser)]
#[command(name = "ti-extract", version)]
struct Cli {
    /// Terra Invicta install dir (default: auto-detect via Steam, or $TI_GAME_DIR)
    #[arg(long)]
    game_dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Png,
    Webp,
}

#[derive(Subcommand)]
enum Cmd {
    /// show the detected install and its data layers
    Info,
    /// extract game data into <data-dir>/<build id>/
    Extract {
        #[arg(long, default_value = "data")]
        data_dir: PathBuf,
        /// override the Steam build id used as output directory
        #[arg(long)]
        build_id: Option<String>,
        /// re-extract even if this build exists
        #[arg(long)]
        force: bool,
        /// exit successfully even if template files fail to parse
        #[arg(long)]
        allow_parse_errors: bool,
        /// output image format (lossless WebP by default)
        #[arg(long, value_enum, default_value = "webp")]
        image_format: Format,
        /// skip image extraction
        #[arg(long)]
        no_assets: bool,
        /// do not export images from this bundle (repeatable), e.g. illustrations
        #[arg(long, value_name = "NAME")]
        skip_bundle: Vec<String>,
    },
    /// changelog between two extracted builds
    Diff {
        old: PathBuf,
        new: PathBuf,
        #[arg(long, default_value = "en")]
        lang: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

fn json_count(dir: &Path) -> usize {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".json"))
        .count()
}

fn opt<T: std::fmt::Display>(v: &Option<T>) -> String {
    v.as_ref().map_or_else(|| "None".to_owned(), T::to_string)
}

fn run(cli: Cli) -> ti_extract::Result<u8> {
    match cli.cmd {
        Cmd::Info => {
            let install = ti_extract::discover(&ti_extract::resolve_game_dir(cli.game_dir.as_deref())?)?;
            println!("game dir:  {}", install.root.display());
            println!("version:   {}", opt(&install.version));
            println!("build id:  {}  (last updated {})", opt(&install.build_id), opt(&install.last_updated));
            for layer in &install.layers {
                println!(
                    "layer {:14} {:9} {:3} templates  {}",
                    layer.id,
                    layer.kind,
                    json_count(&layer.templates_dir),
                    layer.dlc.as_deref().unwrap_or("")
                );
            }
            println!("languages: {}", install.languages().join(", "));
            println!(
                "bundle dirs: {}",
                install.bundle_dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>().join(", ")
            );
            Ok(0)
        }
        Cmd::Extract {
            data_dir,
            build_id,
            force,
            allow_parse_errors,
            image_format,
            no_assets,
            skip_bundle,
        } => {
            let install = ti_extract::discover(&ti_extract::resolve_game_dir(cli.game_dir.as_deref())?)?;
            let opts = ExtractOptions {
                data_dir,
                build_id,
                force,
                allow_parse_errors,
                images: if no_assets {
                    Images::None
                } else if skip_bundle.is_empty() {
                    Images::All
                } else {
                    Images::Skip(skip_bundle)
                },
                image_format: match image_format {
                    Format::Png => ImageFormat::Png,
                    Format::Webp => ImageFormat::Webp,
                },
            };
            let mut progress = |p: Progress| {
                if let Progress::Log { message } = p {
                    println!("{message}");
                }
            };
            let report = ti_extract::extract(&install, &opts, &mut progress, &AtomicBool::new(false))?;
            Ok(u8::from(report.parse_errors > 0 && !allow_parse_errors))
        }
        Cmd::Diff { old, new, lang, output } => {
            let text = ti_extract::diff::diff_builds(&old, &new, &lang)?;
            match output {
                Some(path) => std::fs::write(&path, text).map_err(|source| ti_extract::Error::Io {
                    context: path.display().to_string(),
                    source,
                })?,
                None => print!("{text}"),
            }
            Ok(0)
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => ExitCode::from(code),
        Err(ti_extract::Error::GameNotFound) => {
            eprintln!("Terra Invicta install not found; pass --game-dir or set TI_GAME_DIR");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
