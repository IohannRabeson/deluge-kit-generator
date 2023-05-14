//! # Deluge Kit Generator
//! This tool produces a kit using regions in one or more samples.

use bwavfile::Error as BWavFileError;
use clap::{Parser, Subcommand, ValueHint};
use deluge::{Card, CardError, KitBuilderError, LocalFileSystem, WriteError as DelugeWriteError};
use std::io::Error as IoError;
use std::path::PathBuf;

mod generate_kit;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Deluge card error: {0}")]
    Card(#[from] CardError),

    #[error("Wav file error: {0}")]
    Wav(#[from] BWavFileError),

    #[error("File error: {0}")]
    File(#[from] IoError),

    #[error("Kit building error: {0}")]
    KitBuilding(#[from] KitBuilderError),

    #[error("Write error: {0}")]
    WriteError(#[from] DelugeWriteError),

    #[error("The sample already exists. Use --force to replace the existing file.")]
    SampleAlreadyExists,

    #[error("The directory '{directory:?}' is outside the card '{card_root_directory:?}'")]
    DirectoryOutOfCard {
        directory: PathBuf,
        card_root_directory: PathBuf,
    },

    #[error("'{0}' is not a file")]
    NotAFile(PathBuf),
}

#[derive(Parser)]
#[clap(name = "Deluge Kit Generator")]
#[clap(author = "Iohann R.")]
#[clap(version)]
#[clap(about = "Generate kit patches for Synthstrom Deluge")]
#[clap(long_about)]
struct Cli {
    /// The path to the root directory of the Deluge card where the kit will be created.
    /// Samples are copied into the card as well if needed.
    #[clap(value_hint = ValueHint::DirPath)]
    card_path: PathBuf,

    /// Force an operation, like replacing an already existing sample.
    #[clap(short, long, action)]
    force: bool,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generates a kit using the regions specified by the sample meta data.{n}The original
    /// sample is copied to the specified card into the directory '<root card>/SAMPLES/KITS'.{n}If a
    /// file with the same name already exists in the SAMPLES directory the sample is not copied again, excepted if the flag --force is specified.
    FromRegions {
        /// The paths of the source samples files.
        #[clap(value_hint = ValueHint::FilePath)]
        source_sample_paths: Vec<PathBuf>,

        /// Specify the directory where the sample is copied.
        /// If the directory specified is relative the actual path will be relative to the SAMPLES directory at the root of the card.
        /// If the directory is absolute, it must be inside the deluge card specified.
        #[clap(short, long, default_value = "KITS")]
        destination_sample_directory: PathBuf,

        /// Enable the combine all mode where only one kit is created containing all the regions.
        /// The samples without any regions are ignored.
        #[clap(long)]
        combine_all: bool,
    },
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    let card = &Card::open(LocalFileSystem::default(), &cli.card_path)?;

    // Generate a kit
    match cli.command {
        Commands::FromRegions {
            source_sample_paths,
            destination_sample_directory,
            combine_all,
        } => match combine_all {
            true => {
                if let Err(error) = generate_kit::generate_kit_from_regions(
                    &source_sample_paths,
                    &destination_sample_directory,
                    card,
                    cli.force,
                ) {
                    println!("Error processing multiple samples: {}", error);
                }
            }
            false => {
                for source_sample_path in &source_sample_paths {
                    if let Err(error) = generate_kit::generate_kit_from_regions(
                        &[source_sample_path.clone()],
                        &destination_sample_directory,
                        card,
                        cli.force,
                    ) {
                        println!(
                            "Error processing '{}': {}",
                            source_sample_path.to_string_lossy(),
                            error
                        );
                    }
                }
            }
        },
    };

    Ok(())
}
