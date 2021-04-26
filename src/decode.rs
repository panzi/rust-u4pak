use std::io::Read;
use crate::Result;
use crate::record::CompressionBlock;

pub trait Decode: Sized {
    fn decode(reader: &mut impl Read) -> Result<Self>;
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

// TODO: this all might be inefficient for T=u8
//default impl<T: Decode, const N: usize> Decode for [T; N] where T: Default, T: Copy {
//    #[inline]
//    fn decode(reader: &mut impl Read) -> Result<Self> {
//        let mut items: [T; N] = [T::default(); N];
//        for index in 0..N {
//            items[index] = T::decode(reader)?;
//        }
//        Ok(items)
//    }
//}

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
        let end_offset   = u64::decode(reader)?;

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

    (@decl $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        let $name;
    };

    (@decl $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
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

    (@none $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        $name = None;
    };

    (@none $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
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

    (@decode ($($wrap:tt)*) ($reader:expr) $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])? $(,)?) => {
        decode!(@read ($($wrap)*) ($reader) ($($($attrs)*)?) $name $type $([$($count)*])?);
    };

    (@decode ($($wrap:tt)*) ($reader:expr) $(#[$($attrs:tt)*])? $name:ident : $type:ty $([$($count:tt)*])?, $($rest:tt)*) => {
        decode!(@read ($($wrap)*) ($reader) ($($($attrs)*)?) $name $type $([$($count)*])?);
        decode!(@decode ($($wrap)*) ($reader) $($rest)*);
    };

    // FIXME: This never matches! Why?
    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident String) => {
        $name = {
            let _encoding = decode!(@attr_encoding $($attrs)*);
            let _size = <decode!(@attr_size $($attrs)*)>::decode($reader)? as usize;
            let _buffer = vec![0u8; _size];
            if let Some(_index) = _buffer.iter().position(|_byte| *_byte == 0) {
                _buffer.truncate(_index);
            }
            $($wrap)*(_encoding.parse_vec(_buffer))
        };
    };

    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident $type:ty) => {
        $name = $($wrap)*(<$type>::decode($reader)?);
    };

    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident $type:ty [$count:ty]) => {
        $name = {
            let _count = <$count>::decode($reader)? as usize;
            let mut _items = Vec::with_capacity(_count);
            for _ in 0..(_count) {
                _items.push(<$type>::decode($reader)?);
            }
            $($wrap)*(_items)
        };
    };

    (@read ($($wrap:tt)*) ($reader:expr) ($($attrs:tt)*) $name:ident $type:ty [$count:expr]) => {
        $name = {
            let _count = $count;
            let mut _items = Vec::with_capacity(_count);
            for _ in 0..(_count) {
                _items.push(<$type>::decode($reader)?);
            }
            $($wrap)*(_items)
        };
    };

    (@attr_size size = $value:expr $(,)?) => {
        $value
    };

    (@attr_size size = $value:expr, $($attrs:tt)*) => {
        $value
    };

    (@attr_size $attr:ident = $value:expr, $($attrs:tt)*) => {
        decode!(@attr_size $($attrs)*);
    };

    (@attr_size $(,)?) => {
        u32
    };

    (@attr_encoding encoding = $value:expr $(,)?) => {
        $value
    };

    (@attr_encoding encoding = $value:expr, $($attrs:tt)*) => {
        $value
    };

    (@attr_encoding $attr:ident = $value:expr, $($attrs:tt)*) => {
        decode!(@attr_encoding $($attrs)*);
    };

    (@attr_encoding $(,)?) => {
        Encoding::UTF8
    };
}
