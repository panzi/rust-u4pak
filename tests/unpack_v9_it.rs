mod util;

use util::remove_dir_all_if_exists;
use u4pak::Result;

#[test]
fn test_v9() -> Result<()> {
    let out_dir = "./v9-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_v9.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_encrypted() -> Result<()> {
    let out_dir = "./v9_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_encrypted_v9.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_encrypted_encindex() -> Result<()> {
    let out_dir = "./v9_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_encrypted_encindex_v9.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_encindex() -> Result<()> {
    let out_dir = "./v9_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_encindex_v9.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_compressed() -> Result<()> {
    let out_dir = "./v9_compressed-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_compressed_v9.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_compressed_encrypted() -> Result<()> {
    let out_dir = "./v9_compressed_encrypted-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_compressed_encrypted_v9.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_compressed_encrypted_encindex() -> Result<()> {
    let out_dir = "./v9_compressed_encrypted_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_compressed_encrypted_encindex_v9.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}

#[test]
fn test_v9_compressed_encindex() -> Result<()> {
    let out_dir = "./v9_compressed_encindex-it";
    remove_dir_all_if_exists(out_dir)?;

    util::unpack("./pak-examples/pak/v9/test_compressed_encindex_v9.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all_if_exists(out_dir)?;
    Ok(())
}
