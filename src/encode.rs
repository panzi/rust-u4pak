// This file is part of rust-u4pak.
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
