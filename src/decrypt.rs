use crate::{Error, Record, Result};
use aes::cipher::{BlockDecrypt, NewBlockCipher};
use aes::{Aes256, Block};
use base64;
use std::fs::File;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Seek;
use std::io::Write;

pub fn decrypt(data: &mut Vec<u8>, base64_key: &str) {
    let decoded_key = base64::decode(base64_key).unwrap();
    let cipher = Aes256::new_from_slice(&decoded_key).unwrap();

    for i in 0..data.len() / 16 {
        let mut block = Block::from_mut_slice(&mut data[i * 16..i * 16 + 16]);
        cipher.decrypt_block(&mut block);
    }
}

pub fn decrypt_file(
    input: &mut File,
    output: &mut File,
    base64_key: &str,
    mut length: usize,
) -> Result<bool> {
    let decoded_key = base64::decode(base64_key).unwrap();
    let cipher = Aes256::new_from_slice(&decoded_key).unwrap();

    let mut buf_reader = BufReader::new(input);
    let mut buf_writer = BufWriter::new(output);

    length = length
        + if length % 16 != 0 {
            16 - length % 16
        } else {
            0
        };
    for _ in 0..length / 16 {
        let block_buf = &mut [0u8; 16];
        buf_reader.read_exact(block_buf)?;

        let mut block = Block::from_mut_slice(block_buf);
        cipher.decrypt_block(&mut block);

        buf_writer.write(block_buf)?;
    }
    buf_writer.flush()?;

    Ok(true)
}
