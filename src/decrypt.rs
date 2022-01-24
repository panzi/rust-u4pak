// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use aes::cipher::{BlockDecrypt, NewBlockCipher};
use aes::{Aes256, Block};

pub const BLOCK_SIZE: usize = 16;

pub fn decrypt(data: &mut Vec<u8>, key: Vec<u8>) {
    let cipher = Aes256::new_from_slice(&key).expect("Unable to convert key to Aes256 cipher");
    assert_eq!(data.len() % 16, 0, "Data length must be a multiple of 16");

    for block in data.chunks_mut(BLOCK_SIZE) {
        cipher.decrypt_block(Block::from_mut_slice(block));
    }
}

