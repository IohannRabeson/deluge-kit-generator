use crate::Error;
use bwavfile::{Cue, WaveReader};
use deluge::{Card, CardFolder, KitBuilder, LocalFileSystem, SamplePath, Sound};
use std::fs::File;
use std::path::{Path, PathBuf};

pub fn generate_kit_from_regions(
    source_sample_paths: &[PathBuf],
    destination_sample_directory: &PathBuf,
    card: &Card<LocalFileSystem>,
    replace_existing_samples: bool,
) -> Result<(), Error> {
    // Create the kit patch by building it row by row.
    let mut kit_builder = KitBuilder::default();
    let mut sample_file_path_to_copy = Vec::new();

    for source_sample_path in source_sample_paths {
        let sample_destination_file_path =
            get_sample_path(source_sample_path, destination_sample_directory, card)?;
        let sample_path_in_card = card.sample_path(&sample_destination_file_path)?;
        let cue_points = read_cue_points(source_sample_path)?;

        if !cue_points.is_empty() {
            add_regions_to_kit(&mut kit_builder, &cue_points, sample_path_in_card)?;

            sample_file_path_to_copy.push((source_sample_path, sample_destination_file_path.clone()));
        }
    }

    let kit = kit_builder.build().map_err(Error::KitBuilding)?;
    // Write the kit in the card.
    let kit_path = card.get_next_standard_patch_path(deluge::PatchType::Kit)?;
    println!(
        "Writing kit '{}' with {} row{}",
        &kit_path.to_string_lossy(),
        kit.rows.len(),
        if kit.rows.len() > 1 { "s" } else { "" }
    );
    deluge::write_kit_to_file(&kit, &kit_path)?;

    // Once the kit has been properly built, copy the samples.
    for (source_sample_path, sample_destination_file_path) in sample_file_path_to_copy {
        copy_sample_if_needed(
            source_sample_path,
            &sample_destination_file_path,
            replace_existing_samples,
        )?;
    }

    Ok(())
}

fn copy_sample_if_needed(
    original_sample_path: &Path,
    destination_sample_path: &Path,
    replace_existing: bool,
) -> Result<(), Error> {
    if destination_sample_path.exists() && !replace_existing {
        println!(
            "Sample '{}' already exists.",
            destination_sample_path.display()
        );
        return Ok(());
    }

    if destination_sample_path.exists() {
        println!(
            "Replacing existing sample '{}'",
            destination_sample_path.to_string_lossy()
        );
    } else {
        println!(
            "Copying sample as '{}'",
            destination_sample_path.to_string_lossy()
        );
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

    for i in 0usize..cue_points.len() {
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

fn add_regions_to_kit(
    builder: &mut KitBuilder,
    cue_points: &Vec<Cue>,
    destination_sample_path: SamplePath,
) -> Result<(), Error> {
    for cue_point in cue_points {
        if let Some(cue_point_length) = cue_point.length {
            let sound = Sound::new_sample(
                destination_sample_path.clone(),
                cue_point.frame.into(),
                (cue_point.frame + cue_point_length).into(),
            );

            match &cue_point.label {
                Some(label) => builder.add_named_sound_row(sound, label),
                None => builder.add_sound_row(sound),
            };
        }
    }

    Ok(())
}

fn get_sample_path(
    original_sample_path: &Path,
    destination_sample_directory: &Path,
    card: &Card<LocalFileSystem>,
) -> Result<PathBuf, Error> {
    if !original_sample_path.is_file() {
        return Err(Error::NotAFile(original_sample_path.to_path_buf()));
    }

    let mut path = if destination_sample_directory.is_absolute() {
        let card_directory = card.root_directory();
        if !destination_sample_directory.starts_with(card_directory) {
            return Err(Error::DirectoryOutOfCard {
                directory: destination_sample_directory.to_path_buf(),
                card_root_directory: card_directory.to_path_buf(),
            });
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
