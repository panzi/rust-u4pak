mod util;

use util::remove_dir_all_if_exists;
use u4pak::Result;

const ENCRYPTION_KEY: &str = "aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=";

#[test]
fn test_v11() -> Result<()> {
    let out_dir = "./v11-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_v11.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_encrypted() -> Result<()> {
    let out_dir = "./v11_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_encrypted_v11.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_encrypted_encindex() -> Result<()> {
    let out_dir = "./v11_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_encrypted_encindex_v11.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_encindex() -> Result<()> {
    let out_dir = "./v11_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_encindex_v11.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_compressed() -> Result<()> {
    let out_dir = "./v11_compressed-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_compressed_v11.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_compressed_encrypted() -> Result<()> {
    let out_dir = "./v11_compressed_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_compressed_encrypted_v11.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_compressed_encrypted_encindex() -> Result<()> {
    let out_dir = "./v11_compressed_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_compressed_encrypted_encindex_v11.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v11_compressed_encindex() -> Result<()> {
    let out_dir = "./v11_compressed_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v11/test_compressed_encindex_v11.pak", out_dir, Some(ENCRYPTION_KEY.to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}
