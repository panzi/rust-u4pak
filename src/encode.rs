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

use std::io::Write;
use crate::Result;
use crate::record::CompressionBlock;

pub trait Encode: Sized {
    fn encode(&self, writer: &mut impl Write) -> Result<()>;
}

impl Encode for u8 {
    #[inline]
    fn encode(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_all(&[*self])?;
        Ok(())
    }
}
impl Encode for u32 {
    #[inline]
    fn encode(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}


impl Encode for u64 {
    #[inline]
    fn encode(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_all(&self.to_le_bytes())?;
        Ok(())
    }
}

impl<const N: usize> Encode for [u8; N] {
    #[inline]
    fn encode(&self, writer: &mut impl Write) -> Result<()> {
        writer.write_all(self)?;
        Ok(())
    }
}

impl Encode for CompressionBlock {
    #[inline]
    fn encode(&self, writer: &mut impl Write) -> Result<()> {
        self.start_offset.encode(writer)?;
        self.end_offset.encode(writer)
    }
}

#[macro_export]
macro_rules! encode {
    ($writer:expr, $($rest:tt)*) => {
        encode!(@encode ($writer) $($rest)*);
    };

    (@encode ($writer:expr) $(,)?) => {};

    (@encode ($writer:expr) if $($rest:tt)*) => {
        encode!(@if ($writer) () $($rest)*);
    };

    (@if ($writer:expr) ($($cond:tt)*) { $($body:tt)* } $($rest:tt)*) => {
        if $($cond)* {
            encode!(@encode ($writer) $($body)*);
        }
        encode!(@encode ($writer) $($rest)*);
    };

    (@if ($writer:expr) ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        encode!(@if ($writer) ($($cond)* $tok) $($rest)*);
    };

    (@encode ($writer:expr) $($rest:tt)*) => {
        encode!(@value ($writer) () $($rest)*);
    };

    (@value ($writer:expr) ($($expr:tt)*) [$($count:tt)*], $($rest:tt)*) => {
        encode!(@write ($writer) ($($expr)*) [$($count)*]);
        encode!(@encode ($writer) $($rest)*);
    };

    (@value ($writer:expr) ($($expr:tt)*) [$($count:tt)*]) => {
        encode!(@write ($writer) ($($expr)*) [$($count)*]);
    };

    (@value ($writer:expr) ($($expr:tt)*) , $($rest:tt)*) => {
        encode!(@write ($writer) ($($expr)*));
        encode!(@encode ($writer) $($rest)*);
    };

    (@value ($writer:expr) ($($expr:tt)*)) => {
        encode!(@write ($writer) ($($expr)*));
    };

    (@value ($writer:expr) ($($expr:tt)*) $tok:tt $($rest:tt)*) => {
        encode!(@value ($writer) ($($expr)* $tok) $($rest)*);
    };

    (@write ($writer:expr) ($value:expr)) => {
        ($value).encode($writer)?;
    };

    (@write ($writer:expr) ($value:expr) [$count:ty]) => {
        (($value).len() as $count).encode($writer)?;
        for _item in ($value).iter() {
            _item.encode($writer)?;
        }
    };
}
