mod util;

use util::remove_dir_all_if_exists;
use u4pak::Result;

const ENCRYPTION_KEY: &str = "aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=";

#[test]
fn test_v8() -> Result<()> {
    let out_dir = "./v8-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_v8.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_encrypted() -> Result<()> {
    let out_dir = "./v8_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_encrypted_v8.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_encrypted_encindex() -> Result<()> {
    let out_dir = "./v8_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_encrypted_encindex_v8.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_encindex() -> Result<()> {
    let out_dir = "./v8_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_encindex_v8.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_compressed() -> Result<()> {
    let out_dir = "./v8_compressed-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_compressed_v8.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_compressed_encrypted() -> Result<()> {
    let out_dir = "./v8_compressed_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_compressed_encrypted_v8.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_compressed_encrypted_encindex() -> Result<()> {
    let out_dir = "./v8_compressed_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_compressed_encrypted_encindex_v8.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v8_compressed_encindex() -> Result<()> {
    let out_dir = "./v8_compressed_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v8/test_compressed_encindex_v8.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}
