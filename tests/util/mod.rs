use std::fs::File;
use std::io::BufReader;
use std::num::NonZeroUsize;
use std::path::Path;

use u4pak::index::Encoding;
use u4pak::pak::Options;
use u4pak::unpack::UnpackOptions;
use u4pak::util::{sha1_digest};
use u4pak::walkdir::{walkdir};
use u4pak::{Error, Pak, Result, Variant};

pub fn remove_dir_all_if_exists(path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    if let Err(error) = std::fs::remove_dir_all(path) {
        if let std::io::ErrorKind::NotFound = error.kind() {
            return Ok(());
        }
        return Err(error);
    }

    Ok(())
}

pub fn unpack(path: &str, outdir: &str, encryption: Option<String>) -> Result<()> {
    let encryption_key = if let Some(key) = encryption {
        Some(
            base64::decode(
                key.parse::<String>()
                    .expect("Failed to read encryption key."),
            )
            .expect("Failed to parse encryption key."),
        )
    } else {
        None
    };

    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(error) => return Err(Error::io_with_path(error, path)),
    };
    let mut reader = BufReader::new(&mut file);

    let pak = Pak::from_reader(
        &mut reader,
        Options {
            variant: Variant::default(),
            ignore_magic: false,
            encoding: Encoding::default(),
            force_version: None,
            encryption_key: encryption_key.clone(),
        },
    )?;

    drop(reader);

    u4pak::unpack::unpack(
        &pak,
        &mut file,
        outdir,
        UnpackOptions {
            dirname_from_compression: false,
            verbose: false,
            null_separated: false,
            paths: None,
            thread_count: NonZeroUsize::new(num_cpus::get())
                .unwrap_or(NonZeroUsize::new(1).unwrap()),
            encryption_key,
        },
    )
}

pub fn validate(source_dir: &str, out_dir: &str) -> Result<()> {
    let out_path = Path::new(out_dir);

    let iter = match walkdir(source_dir) {
        Ok(iter) => iter,
        Err(err) => return Err(Error::io_with_path(err, source_dir)),
    };

    for entry in iter {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => return Err(Error::io_with_path(error, source_dir)),
        };
        let file_path = entry.path();
        let file = match file_path.strip_prefix(source_dir) {
            Ok(file) => file,
            Err(err) => return Err(Error::new(format!("Failed to strip prefix from {:?}: {:?}", file_path, err).to_string()))
        };

        let out_path_buff = out_path.join(file);
        let out_path = out_path_buff.as_path();
        
        if out_path.exists() {
            let source_digest = sha1_digest(File::open(&file_path)?)?;
            let out_digest = sha1_digest(File::open(out_path)?)?;
            if source_digest != out_digest {
                return Err(Error::new(format!("Source digest {:?} does not match out digest {:?} for file {:?}", source_digest, out_digest, file).to_string()));
            }
        } else {
            return Err(Error::new(format!("File {:?} does not exist in output", file).to_string()));
        }
    }

    Ok(())
}
