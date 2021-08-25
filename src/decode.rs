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

use std::io::Read;
use crate::Result;
use crate::record::CompressionBlock;

pub trait Decode: Sized {
    fn decode(reader: &mut impl Read) -> Result<Self>;
}

impl Decode for bool {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 1];
        reader.read_exact(&mut buffer)?;
        Ok(buffer[0] != 0u8)
    }
}

impl Decode for u8 {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 1];
        reader.read_exact(&mut buffer)?;
        Ok(buffer[0])
    }
}
impl Decode for u32 {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 4];
        reader.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl Decode for u64 {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 8];
        reader.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl Decode for u128 {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut buffer = [0u8; 16];
        reader.read_exact(&mut buffer)?;
        Ok(Self::from_le_bytes(buffer))
    }
}

impl<const N: usize> Decode for [u8; N] {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let mut items = [0u8; N];
        reader.read_exact(&mut items)?;
        Ok(items)
    }
}

impl Decode for CompressionBlock {
    #[inline]
    fn decode(reader: &mut impl Read) -> Result<Self> {
        let start_offset = u64::decode(reader)?;
        let end_offset = u64::decode(reader)?;

        Ok(Self {
            start_offset,
            end_offset,
        })
    }
}

#[macro_export]
macro_rules! decode {
    ($reader:expr, $($rest:tt)*) => {
        decode!(@decl $($rest)*);
        decode!(@decode () ($reader) $($rest)*);
    };

    (@decode ($($wrap:tt)*) ($reader:expr) $(,)?) => {};

    (@decode ($($wrap:tt)*) ($reader:expr) if $($rest:tt)*) => {
        decode!(@if ($($wrap)*) ($reader) () $($rest)*);
    };

    (@if ($($wrap:tt)*) ($reader:expr) ($($cond:tt)*) { $($body:tt)* } $($rest:tt)*) => {
        if $($cond)* {
            decode!(@decode (Some) ($reader) $($body)*);
        } else {
            decode!(@none $($body)*);
        }
        decode!(@decode ($($wrap)*) ($reader) $($rest)*);
    };

    (@if ($($wrap:tt)*) ($reader:expr) ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        decode!(@if ($($wrap)*) ($reader) ($($cond)* $tok) $($rest)*);
    };

    (@decl $(,)?) => {};

    (@decl if $($rest:tt)*) => {
        decode!(@decl_if () $($rest)*);
    };

    (@decl $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        let $name;
    };

    (@decl $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        let $name;
        decode!(@decl $($rest)*);
    };

    (@decl_if ($($cond:tt)*) { $($body:tt)* } $($rest:tt)*) => {
        decode!(@decl $($body)*);
        decode!(@decl $($rest)*);
    };

    (@decl_if ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        decode!(@decl_if ($($cond)* $tok) $($rest)*);
    };

    (@none $(,)?) => {};

    (@none if $($rest:tt)*) => {
        decode!(@none_if () $($rest)*);
    };

    (@none $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        $name = None;
    };

    (@none $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        $name = None;
        decode!(@none $($rest)*);
    };

    (@none_if ($cond:expr) { $($body:tt)* } $($rest:tt)*) => {
        decode!(@none $($body)*);
        decode!(@none $($rest)*);
    };

    (@none_if ($($cond:tt)*) $tok:tt $($rest:tt)*) => {
        decode!(@none_if ($($cond)* $tok) $($rest)*);
    };

    (@decode ($($wrap:tt)*) ($reader:expr) $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        decode!(@read ($($wrap)*) ($reader) $name $type $([$($count)*])?);
    };

    (@decode ($($wrap:tt)*) ($reader:expr) $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        decode!(@read ($($wrap)*) ($reader) $name $type $([$($count)*])?);
        decode!(@decode ($($wrap)*) ($reader) $($rest)*);
    };

    (@read ($($wrap:tt)*) ($reader:expr) $name:ident $type:ty) => {
        $name = $($wrap)*(<$type>::decode($reader)?);
    };

    (@read ($($wrap:tt)*) ($reader:expr) $name:ident $type:ty [$count:ty]) => {
        $name = {
            let _count = <$count>::decode($reader)? as usize;
            let mut _items = Vec::with_capacity(_count);
            for _ in 0.._count {
                _items.push(<$type>::decode($reader)?);
            }
            $($wrap)*(_items)
        };
    };

    (@read ($($wrap:tt)*) ($reader:expr) $name:ident $type:ty [$count:expr]) => {
        $name = {
            let _count = $count;
            let mut _items = Vec::with_capacity(_count);
            for _ in 0..(_count) {
                _items.push(<$type>::decode($reader)?);
            }
            $($wrap)*(_items)
        };
    };
}
