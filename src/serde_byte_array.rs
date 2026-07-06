//! Serde serialization helpers for fixed-size byte arrays > 32 elements.
//!
//! serde's built-in array support only goes up to 32 elements. These helpers
//! provide Serialize/Deserialize for any `[u8; N]` by treating them as tuples.
//!
//! Usage: `#[serde(with = "crate::serde_byte_array")]` on the field.

use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::ser::{SerializeTuple, Serializer};
use std::fmt;

/// Serialize a fixed-size byte array as a tuple of bytes.
pub fn serialize<const N: usize, S>(arr: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_tuple(N)?;
    for elem in arr {
        seq.serialize_element(elem)?;
    }
    seq.end()
}

/// Deserialize a fixed-size byte array from a tuple of bytes.
pub fn deserialize<'de, const N: usize, D>(deserializer: D) -> Result<[u8; N], D::Error>
where
    D: Deserializer<'de>,
{
    struct ArrayVisitor<const M: usize>;

    impl<'de, const M: usize> Visitor<'de> for ArrayVisitor<M> {
        type Value = [u8; M];

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "an array of {} bytes", M)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<[u8; M], A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut arr = [0u8; M];
            for (i, slot) in arr.iter_mut().enumerate() {
                *slot = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(i, &self))?;
            }
            Ok(arr)
        }
    }

    deserializer.deserialize_tuple(N, ArrayVisitor::<N>)
}
