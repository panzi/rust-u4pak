// This file is part of rust-u4pak.
//
// rust-u4pak is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-u4pak is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-u4pak.  If not, see <https://www.gnu.org/licenses/>.

use aes::cipher::{BlockDecrypt, NewBlockCipher};
use aes::{Aes256, Block};

pub fn decrypt(data: &mut Vec<u8>, key: Vec<u8>) {
    let cipher = Aes256::new_from_slice(&key).expect("Unable to convert key to Aes256 cipher");

    for i in 0..data.len() / 16 {
        let mut block = Block::from_mut_slice(&mut data[i * 16..i * 16 + 16]);
        cipher.decrypt_block(&mut block);
    }
}

