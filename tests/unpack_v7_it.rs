mod util;

use std::fs::remove_dir_all;
use u4pak::Result;

#[test]
fn test_v7() -> Result<()> {
    let out_dir = "./v7-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_v7.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v7_encrypted() -> Result<()> {
    let out_dir = "./v7_encrypted-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_encrypted_v7.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v7_encrypted_encindex() -> Result<()> {
    let out_dir = "./v7_encrypted_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_encrypted_encindex_v7.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
fn test_v7_encindex() -> Result<()> {
    let out_dir = "./v7_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_encindex_v7.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
#[ignore]
fn test_v7_compressed() -> Result<()> {
    let out_dir = "./v7_compressed-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_compressed_v7.pak", out_dir, None)?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
#[ignore]
fn test_v7_compressed_encrypted() -> Result<()> {
    let out_dir = "./v7_compressed_encrypted-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_compressed_encrypted_v7.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
#[ignore]
fn test_v7_compressed_encrypted_encindex() -> Result<()> {
    let out_dir = "./v7_compressed_encrypted_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_compressed_encrypted_encindex_v7.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}

#[test]
#[ignore]
fn test_v7_compressed_encindex() -> Result<()> {
    let out_dir = "./v7_compressed_encindex-it";
    remove_dir_all(out_dir);

    util::unpack("./pak-examples/v7/test_compressed_encindex_v7.pak", out_dir, Some("aWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWlpaWk=".to_string()))?;
    util::validate("./pak-examples/original-files", out_dir)?;

    remove_dir_all(out_dir);
    Ok(())
}