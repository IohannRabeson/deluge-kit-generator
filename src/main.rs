/// # Deluge Kit Generator
/// This tool produces a kit based on a sample and his regions.
/// Currently I only tested to create regions in samples using Ocenaudio.

use std::path::{PathBuf, Path};
use bwavfile::{WaveReader, Error as BWavFileError, Cue};
use clap::{Parser, Subcommand, ValueHint};
use deluge::{LocalFileSystem, CardError, CardFolder, KitBuilder, Sound, Card, KitBuilderError, WriteError as DelugeWriteError, Kit, SamplePath};
use std::fs::File;
use std::io::Error as IoError;

#[derive(thiserror::Error, Debug)]
enum Error {
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
    DirectoryOutOfCard{ directory: PathBuf, card_root_directory: PathBuf },

    #[error("'{0}' is not a file")]
    NotAFile(PathBuf),
}

#[derive(Parser)]
#[clap(name = "Deluge Kit Generator")]
#[clap(author = "Iohann R.")]
#[clap(version = "0.0.1")]
#[clap(about = "Generate kit patches for Synthstrom Deluge")]
#[clap(long_about = r#"This tool produces a kit based on a sample and his regions."
                    "Currently I only tested to create regions in samples using Ocenaudio."
                    "If no regions are specified the tool does nothing."#)]
struct Cli {
    /// The path to the root directory of the Deluge card where the kit will be created.
    /// Samples are copied into the card as well if needed.
    #[clap(parse(from_os_str), value_hint = ValueHint::DirPath)]
    card_path: PathBuf,

    /// Force an operation, like replacing an already existing sample.
    #[clap(short, long, action)]
    force: bool,

    #[clap(subcommand)]
    command: Commands
}

#[derive(Subcommand)]
enum Commands {
    /// Generates a kit using the regions specified by the sample meta data.{n}The original
    /// sample is copied to the specified card into the directory '<root card>/SAMPLES/KITS'.
    /// If a file with the same name already exists in the SAMPLES directory the command is 
    /// aborted, excepted if the flag --force is specified.
    FromRegions {
        /// The path of the source sample file.
        #[clap(parse(from_os_str), value_hint = ValueHint::FilePath)]
        source_sample_path: PathBuf,

        /// Specify the directory where the sample is copied.
        /// If the directory specified is relative the actual path will be relative to the SAMPLES directory at the root of the card.
        /// If the directory is absolute, it must be inside the deluge card specified. 
        #[clap(short, long, default_value = "KITS")]
        destination_sample_directory: PathBuf,
    }
}

fn main() -> Result<(), Error> {
    let cli = Cli::parse();
    let card = &deluge::Card::open(LocalFileSystem::default(), &cli.card_path)?;

    // Generate a kit
    match cli.command {
        Commands::FromRegions { 
            source_sample_path ,
            destination_sample_directory,
        } => {
            let destination_sample_path = get_sample_path(&source_sample_path, &destination_sample_directory, card)?;
            let cue_points = read_cue_points(&source_sample_path)?;
            let kit = new_kit_from_regions(&cue_points, card.sample_path(&destination_sample_path)?)?;
            let kit_path = card.get_next_standard_patch_path(deluge::PatchType::Kit)?;

            // Write the kit as file
            println!("Writing kit '{}' with {} row{}", &kit_path.to_string_lossy(), kit.rows.len(), if kit.rows.len() > 1 { "s" } else { "" });

            deluge::write_kit_to_file(&kit, &kit_path)?;

            copy_sample_if_needed(&source_sample_path, &destination_sample_path, cli.force)?;
        },
    };

    Ok(())
}

fn copy_sample_if_needed(original_sample_path: &Path, destination_sample_path: &Path, replace_existing: bool) -> Result<(), Error> {
    if destination_sample_path.exists() && !replace_existing {
        return Err(Error::SampleAlreadyExists)
    }

    if destination_sample_path.exists() {
        println!("Replacing existing sample '{}'", destination_sample_path.to_string_lossy());
    } else {
        println!("Copying sample as '{}'", destination_sample_path.to_string_lossy());
    }

    if let Some(destination_sample_directory) = destination_sample_path.parent() {
        if !destination_sample_directory.exists() {
            std::fs::create_dir_all(destination_sample_directory)?;
        }
    }

    std::fs::copy(original_sample_path, &destination_sample_path)?;

    Ok(())
}

fn read_cue_points(sample_path: &Path) -> Result<Vec<Cue>, Error> {
    let mut wav_reader = WaveReader::new(File::open(&sample_path)?)?;
    let mut cue_points = wav_reader.cue_points()?;
    let total_length = wav_reader.frame_length()?;

    for i in 0usize .. cue_points.len() {
        // If the region is specified by a start marker only
        // the length is specified using the next marker or the end of the sample.
        if cue_points[i].length.is_none() {
            if i + 1 < cue_points.len() {
                cue_points[i].length = Some(cue_points[i + 1].frame - cue_points[i].frame);
            } else {
                cue_points[i].length = Some((total_length - cue_points[i].frame as u64) as u32);
            }
        }
    }

    Ok(cue_points)
}

fn new_kit_from_regions(cue_points: &Vec<Cue>, destination_sample_path: SamplePath) -> Result<Kit, Error> {
    let mut kit_builder = KitBuilder::default();

    for cue_point in cue_points {
        if let Some(cue_point_length) = cue_point.length {
            let sound = Sound::new_sample(
                destination_sample_path.clone(), 
                cue_point.frame.into(), (cue_point.frame + cue_point_length).into());

            match &cue_point.label {
                Some(label) => kit_builder.add_named_sound_row(sound, label),
                None => kit_builder.add_sound_row(sound),
            };
        }
    }

    kit_builder.build().map_err(Error::KitBuilding)
}

fn get_sample_path(original_sample_path: &Path, destination_sample_directory: &Path, card: &Card<LocalFileSystem>) -> Result<PathBuf, Error> {
    if !original_sample_path.is_file() {
        return Err(Error::NotAFile(original_sample_path.to_path_buf()))
    }
    
    let mut path = if destination_sample_directory.is_absolute() {
        let card_directory = card.root_directory();
        if !destination_sample_directory.starts_with(card_directory) {
            return Err(Error::DirectoryOutOfCard { directory: destination_sample_directory.to_path_buf(), card_root_directory: card_directory.to_path_buf() })
        }

        destination_sample_directory.to_path_buf()
    } else {
        let mut path = card.get_directory_path(CardFolder::Samples);

        path.push(destination_sample_directory);

        path
    };

    path.push(original_sample_path.file_name().expect("file name"));

    Ok(path)
}
