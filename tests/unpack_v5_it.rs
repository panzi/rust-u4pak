mod util;

use std::fs::remove_dir_all;
use u4pak::Result;

#[test]
fn test_v5() -> Result<()> {
    let out_dir = "./v5-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_v5.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_encrypted() -> Result<()> {
    let out_dir = "./v5_encrypted-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_encrypted_v5.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_encrypted_encindex() -> Result<()> {
    let out_dir = "./v5_encrypted_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_encrypted_encindex_v5.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_encindex() -> Result<()> {
    let out_dir = "./v5_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_encindex_v5.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_compressed() -> Result<()> {
    let out_dir = "./v5_compressed-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_compressed_v5.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_compressed_encrypted() -> Result<()> {
    let out_dir = "./v5_compressed_encrypted-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_compressed_encrypted_v5.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_compressed_encrypted_encindex() -> Result<()> {
    let out_dir = "./v5_compressed_encrypted_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_compressed_encrypted_encindex_v5.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v5_compressed_encindex() -> Result<()> {
    let out_dir = "./v5_compressed_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/pak/v5/test_compressed_encindex_v5.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}