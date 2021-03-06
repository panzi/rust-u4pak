mod util;

use util::remove_dir_all_if_exists;
use u4pak::Result;

const ENCRYPTION_KEY: &str = "aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=";

#[test]
fn test_v7() -> Result<()> {
    let out_dir = "./v7-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_v7.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_encrypted() -> Result<()> {
    let out_dir = "./v7_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_encrypted_v7.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_encrypted_encindex() -> Result<()> {
    let out_dir = "./v7_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_encrypted_encindex_v7.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_encindex() -> Result<()> {
    let out_dir = "./v7_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_encindex_v7.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_compressed() -> Result<()> {
    let out_dir = "./v7_compressed-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_compressed_v7.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_compressed_encrypted() -> Result<()> {
    let out_dir = "./v7_compressed_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_compressed_encrypted_v7.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_compressed_encrypted_encindex() -> Result<()> {
    let out_dir = "./v7_compressed_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_compressed_encrypted_encindex_v7.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v7_compressed_encindex() -> Result<()> {
    let out_dir = "./v7_compressed_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v7/test_compressed_encindex_v7.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}
