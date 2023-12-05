//! Serde for byte arrays, replacing the default serde behavior of using sequences.

use std::borrow::Cow;

use serde::de::{Deserializer, Visitor};
use serde::ser::Serializer;


pub fn deserialize<'a, 'de, D: Deserializer<'de>>(deserializer: D) -> Result<Cow<'a, [u8]>, D::Error> {

    struct ByteVisitor;
    impl<'de> Visitor<'de> for ByteVisitor {

        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(formatter, "bytes or byte buffer")
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: serde::de::Error, 
        {
            Ok(v.to_vec())
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: serde::de::Error, 
        {
            Ok(v)
        }

    }

    deserializer.deserialize_byte_buf(ByteVisitor).map(Cow::Owned)

}

pub fn serialize<'a, S: Serializer>(value: &Cow<'a, [u8]>, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_bytes(&value)
}
