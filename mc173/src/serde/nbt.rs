//! NBT format serialization and deserialization.

use std::io::{Read, self, Write};
use std::fmt;

use serde::{ser, de};

use crate::util::{ReadJavaExt, WriteJavaExt};


const NBT_BYTE       : i8 = 1;
const NBT_SHORT      : i8 = 2;
const NBT_INT        : i8 = 3;
const NBT_LONG       : i8 = 4;
const NBT_FLOAT      : i8 = 5;
const NBT_DOUBLE     : i8 = 6;
const NBT_BYTE_ARRAY : i8 = 7;
const NBT_STRING     : i8 = 8;
const NBT_LIST       : i8 = 9;
const NBT_COMPOUND   : i8 = 10;


/// Serialize a NBT tag to a writer.
pub fn to_writer<S: ser::Serialize>(mut writer: impl Write, value: &S) -> Result<(), NbtError> {
    
    let mut next_key = String::new();

    value.serialize(NbtSerializer {
        writer: &mut writer,
        next_key: &mut next_key,
        remaining_len: 0,
        seq_element_type_id: None,
        seq_type_id: None,
        in_key: false,
    })

}

/// Deserialize a NBT tag from a reader.
pub fn from_reader<'de, D: de::Deserialize<'de>>(reader: impl Read) -> Result<D, NbtError> {
    
    let mut deserializer = NbtDeserializer {
        reader,
        state: NbtDeserializerState::Root,
    };

    D::deserialize(&mut deserializer)

}


/// A NBT serializer around an arbitrary I/O writer.
/// 
/// NOTE: We are not using enumeration for the state because serde already defines the 
/// state using a strict type system.
struct NbtSerializer<'a, W> {
    /// The inner writer.
    writer: &'a mut W,
    /// The key to write for the next serialized value.
    next_key: &'a mut String,
    /// Length remaining in the sequence or map. When serializing the first sequence
    /// element, this is also used to write the sequence header.
    remaining_len: usize,
    /// If the current serializer is for a sequence element, then this should be set to
    /// a reference to the required sequence type id. If the sequence type is id is None
    /// then it should be set to the type, while also writing the sequence header.
    seq_element_type_id: Option<&'a mut Option<i8>>,
    /// If the current serializer is on a sequence, then this represent the current type
    /// of the 
    seq_type_id: Option<i8>,
    /// Set to true when the serializer should set the next_key from a serialized str, 
    /// any other serialized value should produce an error because only str key are
    /// allowed. If we are in the map serializer, this value is actually used to know
    /// if a key has been serialized before value.
    in_key: bool,
}

impl<W: Write> NbtSerializer<'_, W> {

    /// Set the next key for the next value serialized.
    fn set_next_key(&mut self, key: &str) {
        self.next_key.clear();
        self.next_key.push_str(key);
    }

    /// Write a value key and type just before writing the value.
    fn write_key(&mut self, value_type_id: i8) -> Result<(), NbtError> {
        
        // We cannot write any key while serializing a key.
        if self.in_key {
            return Err(NbtError::IllegalKeyType);
        }

        self.writer.write_java_byte(value_type_id)?;
        self.writer.write_java_string8(&self.next_key)?;

        // If we are serializing a sequence element, check its type or set it.
        if let Some(seq_element_type_id) = &mut self.seq_element_type_id {

            if let Some(seq_type_id) = **seq_element_type_id {
                if seq_type_id != value_type_id {
                    return Err(NbtError::IncoherentTagType);
                }
            } else {
                // This is the first element in the sequence, so remaining length
                // should contains the full length, we set the type id required for
                // elements inserted after it.
                // NOTE: Cast is safe because we checked it when creating sequence.
                self.writer.write_java_byte(value_type_id)?;
                self.writer.write_java_int(self.remaining_len as i32)?;
                **seq_element_type_id = Some(value_type_id);
            }

        }

        Ok(())

    }

}

impl<W: Write> ser::Serializer for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        self.serialize_i8(v as i8)
    }

    fn serialize_i8(mut self, v: i8) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_BYTE)?;
        self.writer.write_java_byte(v)?;
        Ok(())
    }

    fn serialize_i16(mut self, v: i16) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_SHORT)?;
        self.writer.write_java_short(v)?;
        Ok(())
    }

    fn serialize_i32(mut self, v: i32) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_INT)?;
        self.writer.write_java_int(v)?;
        Ok(())
    }

    fn serialize_i64(mut self, v: i64) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_LONG)?;
        self.writer.write_java_long(v)?;
        Ok(())
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        self.serialize_i8(v as i8)
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        self.serialize_i16(v as i16)
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        self.serialize_i32(v as i32)
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        self.serialize_i64(v as i64)
    }

    fn serialize_f32(mut self, v: f32) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_FLOAT)?;
        self.writer.write_java_float(v)?;
        Ok(())
    }

    fn serialize_f64(mut self, v: f64) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_DOUBLE)?;
        self.writer.write_java_double(v)?;
        Ok(())
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(v.encode_utf8(&mut [0; 4]))
    }

    fn serialize_str(mut self, v: &str) -> Result<Self::Ok, Self::Error> {
        if self.in_key {
            self.set_next_key(v);
        } else {
            self.write_key(NBT_STRING)?;
            self.writer.write_java_string8(v)?;
        }
        Ok(())
    }

    fn serialize_bytes(mut self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        self.write_key(NBT_BYTE_ARRAY)?;
        let len: i32 = v.len().try_into().map_err(|_| NbtError::IllegalLength)?;
        self.writer.write_java_int(len)?;
        self.writer.write_all(v)?;
        Ok(())
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        // There is no null value in NBT, so we use zero.
        self.serialize_i8(0)
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize
    {
        // Just forward to print the value as-is.
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        // There is no null value in NBT, so we use zero.
        self.serialize_i8(0)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        self.serialize_unit()
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        // Unit variant are serialized just with the name of the variant.
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize 
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: serde::Serialize 
    {
        // We represent a tuple variant as a compound {variant: value}, so we directly
        // write the compound key and return a sequence serializer just after.
        // Note that the compound still need a zero byte for termination.
        self.write_key(NBT_COMPOUND)?;
        self.set_next_key(variant);
        value.serialize(self)
    }

    fn serialize_seq(mut self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        if let Some(len) = len {

            self.write_key(NBT_LIST)?;

            // If length is known to be zero, write zero length here because no value 
            // will be serialized to initialize the sequence type.
            if len == 0 {
                self.writer.write_java_byte(NBT_BYTE)?; 
                self.writer.write_java_int(0)?;
            } else if len > i32::MAX as usize {
                return Err(NbtError::IllegalLength);
            }

            // Modify the current state to a sequence and return itself,
            self.remaining_len = len as usize;
            Ok(self)

        } else {
            Err(NbtError::MissingSeqLength)
        }
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        // We represent a tuple variant as a compound {variant: (data...)}, so we directly
        // write the compound key and return a sequence serializer just after.
        // Note that the compound still need a zero byte for termination.
        self.write_key(NBT_COMPOUND)?;
        self.set_next_key(variant);
        self.serialize_seq(Some(len))
    }

    fn serialize_map(mut self, len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        if let Some(len) = len {

            self.write_key(NBT_COMPOUND)?;

            if len > i32::MAX as usize {
                return Err(NbtError::IllegalLength);
            }

            // Modify the state and return itself.
            self.remaining_len = len as usize;
            Ok(self)

        } else {
            Err(NbtError::MissingMapLength)
        }
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        self.serialize_map(Some(len))
    }

    fn serialize_struct_variant(
        mut self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        // We represent a struct variant as a compound {variant: {key: data...}}, so we 
        // directly write the compound key and return a sequence serializer just after. 
        // Note that the compound still need a zero byte for termination.
        self.write_key(NBT_COMPOUND)?;
        self.set_next_key(variant);
        self.serialize_map(Some(len))
    }

}

impl<W: Write> ser::SerializeSeq for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize 
    {

        let remaining_len = self.remaining_len;
        if remaining_len == 0 {
            return Err(NbtError::IncoherentSeqLength(0));
        }

        // We also pass the next key to avoid reallocation.
        value.serialize(NbtSerializer {
            writer: &mut *self.writer,
            next_key: &mut *self.next_key,
            seq_element_type_id: Some(&mut self.seq_type_id),
            seq_type_id: None,
            remaining_len: 0,
            in_key: false,
        })?;

        self.remaining_len = remaining_len - 1;
        Ok(())

    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.remaining_len != 0 {
            Err(NbtError::IncoherentSeqLength(self.remaining_len))
        } else {
            Ok(())
        }
    }

}

impl<W: Write> ser::SerializeTuple for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize 
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }

}

impl<W: Write> ser::SerializeTupleStruct for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize 
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }

}

impl<W: Write> ser::SerializeTupleVariant for NbtSerializer<'_, W> {
    
    type Ok = ();
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize 
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // IMPORTANT: Terminate byte for the compound containing the variant.
        self.writer.write_java_byte(0)?;
        ser::SerializeSeq::end(self)
    }

}

impl<W: Write> ser::SerializeMap for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize
    {

        assert!(!self.in_key, "missing call to serialize_value");

        key.serialize(NbtSerializer {
            writer: &mut *self.writer,
            next_key: &mut *self.next_key,
            remaining_len: 0,
            seq_element_type_id: None,
            seq_type_id: None,
            in_key: true,  // Important!
        })?;

        // Mark that a key has been serialized.
        self.in_key = true;

        Ok(())

    }

    fn serialize_value<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize
    {

        assert!(self.in_key, "missing call to serialize_key");
        
        let remaining_len = self.remaining_len;
        if remaining_len == 0 {
            return Err(NbtError::IncoherentMapLength(0));
        }

        value.serialize(NbtSerializer {
            writer: &mut *self.writer,
            next_key: &mut *self.next_key,
            remaining_len: 0,
            seq_element_type_id: None,
            seq_type_id: None,
            in_key: false,
        })?;

        // Mark that the value has been serialized, key can be serialized again.
        self.in_key = false;

        self.remaining_len = remaining_len - 1;
        Ok(())

    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.remaining_len != 0 {
            Err(NbtError::IncoherentMapLength(self.remaining_len))
        } else {
            // Zero should be written after last element of a compound.
            self.writer.write_java_byte(0)?;
            Ok(())
        }
    }

}

impl<W: Write> ser::SerializeStruct for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize 
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeMap::end(self)
    }

}

impl<W: Write> ser::SerializeStructVariant for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtError;

    fn serialize_field<T: ?Sized>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: serde::Serialize
    {
        ser::SerializeMap::serialize_entry(self, key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // IMPORTANT: Terminate byte for the compound containing the variant.
        self.writer.write_java_byte(0)?;
        ser::SerializeMap::end(self)
    }
}


/// A NBT deserializer around an arbitrary I/O reader.
struct NbtDeserializer<R> {
    /// Inner reader.
    reader: R,
    /// State of the deserializer.
    state: NbtDeserializerState,
}

/// The NBT deserializer state.
enum NbtDeserializerState {
    /// This is the initial state of the deserialization, the key is unused.
    Root,
    /// Deserialization of a sequence value of the given type id.
    SeqValue(i8),
    /// A map key should be returned.
    MapKey(Option<String>),
    /// The given map key should be returned next time.
    MapValue(Option<i8>),
}

/// A NBT deserializer for a sequence.
struct NbtSeqDeserializer<'a, R> {
    /// Original deserializer.
    parent: &'a mut NbtDeserializer<R>,
    /// Type id of tags in the sequence.
    type_id: i8,
    /// Remaining length in the sequence.
    remaining_len: usize,
}

/// A NBT deserializer for a map.
struct NbtMapDeserializer<'a, R> {
    /// Original deserializer.
    parent: &'a mut NbtDeserializer<R>,
    /// Type id of the next value, none if `next_key` must be called before.
    next_type_id: Option<i8>,
}

enum NbtDeserializerHint {
    /// Default type should be returned.
    Default,
    /// Boolean should be returned if possible.
    Bool,
    /// Unsigned variant of the number should be returned.
    Unsigned,
}

impl<R: Read> NbtDeserializer<R> {

    /// Internal helper function to deserialize any value, much like serde 
    /// `deserialize_any` but with a hint about the expected sign of the value, only 
    /// relevant if the is an integer.
    fn deserialize_any_unsigned<'de, V>(&mut self, visitor: V, hint: NbtDeserializerHint) -> Result<V::Value, NbtError>
    where
        V: de::Visitor<'de> 
    {

        let type_id;
        match  self.state {
            NbtDeserializerState::Root => {

                type_id = self.reader.read_java_byte()?;
                if type_id == 0 {
                    // Root value cannot be of the end type.
                    return Err(NbtError::IllegalTagType);
                }

                let _key = self.reader.read_java_string8()?;

            }
            NbtDeserializerState::SeqValue(seq_type_id) => {
                // Use the sequence type and do not read a key.
                type_id = seq_type_id;
            }
            NbtDeserializerState::MapKey(ref mut key) => {
                return visitor.visit_string(key.take().expect("double deserialize key"));
            }
            NbtDeserializerState::MapValue(ref mut value_type_id) => {
                // Use the previously read value.
                type_id = value_type_id.take().expect("double deserialize value");
            }
        }

        match type_id {
            NBT_BYTE => {
                let val = self.reader.read_java_byte()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u8(val as u8),
                    NbtDeserializerHint::Bool => visitor.visit_bool(val != 0),
                    _ => visitor.visit_i8(val)
                }
            }
            NBT_SHORT => {
                let val = self.reader.read_java_short()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u16(val as u16),
                    _ => visitor.visit_i16(val)
                }
            }
            NBT_INT => {
                let val = self.reader.read_java_int()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u32(val as u32),
                    _ => visitor.visit_i32(val)
                }
            }
            NBT_LONG => {
                let val = self.reader.read_java_long()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u64(val as u64),
                    _ => visitor.visit_i64(val)
                }
            }
            NBT_FLOAT => visitor.visit_f32(self.reader.read_java_float()?),
            NBT_DOUBLE => visitor.visit_f64(self.reader.read_java_double()?),
            NBT_BYTE_ARRAY => {

                let len = self.reader.read_java_int()?;
                if len < 0 {
                    return Err(NbtError::IllegalLength);
                }

                let mut buf = vec![0u8; len as usize];
                self.reader.read_exact(&mut buf)?;
                visitor.visit_byte_buf(buf)

            }
            NBT_STRING => visitor.visit_string(self.reader.read_java_string8()?),
            NBT_LIST => {

                // NOTE: A list can contain a single type.
                let type_id = self.reader.read_java_byte()?;
                let len = self.reader.read_java_int()?;
                if len < 0 {
                    return Err(NbtError::IllegalLength);
                }

                visitor.visit_seq(NbtSeqDeserializer {
                    parent: self,
                    type_id,
                    remaining_len: len as usize,
                })

            }
            NBT_COMPOUND => {

                visitor.visit_map(NbtMapDeserializer {
                    parent: self,
                    next_type_id: None,
                })

            }
            _ => return Err(NbtError::IllegalTagType)
        }

    }
    
}

impl<'de, 'a, R: Read> de::Deserializer<'de> for &'a mut NbtDeserializer<R> {

    type Error = NbtError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_unsigned(visitor, NbtDeserializerHint::Default)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any_unsigned(visitor, NbtDeserializerHint::Bool)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_unsigned(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_unsigned(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_unsigned(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_unsigned(visitor, NbtDeserializerHint::Unsigned)
    }

    serde::forward_to_deserialize_any! {
        i8 i16 i32 i64 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }

}

impl<'de, 'a, R: Read> de::SeqAccess<'de> for NbtSeqDeserializer<'a, R> {

    type Error = NbtError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de> 
    {
        if self.remaining_len == 0 {
            Ok(None)
        } else {
            self.remaining_len -= 1;
            self.parent.state = NbtDeserializerState::SeqValue(self.type_id);
            let ret = seed.deserialize(&mut *self.parent).map(Some);
            ret
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining_len)
    }

}

impl<'de, 'a, R: Read> de::MapAccess<'de> for NbtMapDeserializer<'a, R> {

    type Error = NbtError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>
    {

        if let NbtDeserializerState::MapKey(_) = self.parent.state {
            panic!("double next key");
        }

        let type_id = self.parent.reader.read_java_byte()?;
        
        // End of map tag.
        if type_id == 0 {
            return Ok(None);
        }

        let key = self.parent.reader.read_java_string8()?;

        self.next_type_id = Some(type_id);
        self.parent.state = NbtDeserializerState::MapKey(Some(key));
        seed.deserialize(&mut *self.parent).map(Some)

    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de> 
    {

        let type_id = self.next_type_id.take().expect("missing next key");
        self.parent.state = NbtDeserializerState::MapValue(Some(type_id));
        seed.deserialize(&mut *self.parent)
    }

}


// /// A generic NBT tag, this structure has a size of 32 bytes. 
// #[derive(Clone, PartialEq)]
// pub enum Nbt {
//     Byte(i8),
//     Short(i16),
//     Int(i32),
//     Long(i64),
//     Float(f32),
//     Double(f64),
//     ByteArray(Vec<u8>),
//     String(String),
//     List(Vec<Nbt>),
//     Compound(NbtCompound),
// }

// /// An abstract NBT compound type that hides the internal implementation of the mapping.
// #[derive(Clone, PartialEq)]
// pub struct NbtCompound {
//     inner: BTreeMap<String, Nbt>,
// }

// impl Nbt {

//     #[inline]
//     pub fn as_boolean(&self) -> Option<bool> {
//         self.as_byte().map(|b| b != 0)
//     }

//     #[inline]
//     pub fn as_byte(&self) -> Option<i8> {
//         match *self {
//             Self::Byte(n) => Some(n),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_short(&self) -> Option<i16> {
//         match *self {
//             Self::Short(n) => Some(n),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_int(&self) -> Option<i32> {
//         match *self {
//             Self::Int(n) => Some(n),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_long(&self) -> Option<i64> {
//         match *self {
//             Self::Long(n) => Some(n),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_float(&self) -> Option<f32> {
//         match *self {
//             Self::Float(n) => Some(n),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_double(&self) -> Option<f64> {
//         match *self {
//             Self::Double(n) => Some(n),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_byte_array(&self) -> Option<&[u8]> {
//         match self {
//             Self::ByteArray(buf) => Some(&buf[..]),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_string(&self) -> Option<&str> {
//         match self {
//             Self::String(string) => Some(string.as_str()),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_list(&self) -> Option<&[Nbt]> {
//         match self {
//             Self::List(list) => Some(&list[..]),
//             _ => None
//         }
//     }

//     #[inline]
//     pub fn as_compound(&self) -> Option<&NbtCompound> {
//         match self {
//             Self::Compound(comp) => Some(comp),
//             _ => None
//         }
//     }

// }

// impl NbtCompound {

//     pub fn new() -> Self {
//         Self { inner: BTreeMap::new() }
//     }

//     #[inline]
//     pub fn insert(&mut self, key: String, tag: Nbt) {
//         self.inner.insert(key, tag);
//     }

//     #[inline]
//     pub fn get(&self, key: &str) -> Option<&Nbt> {
//         self.inner.get(key)
//     }

//     #[inline]
//     pub fn get_boolean(&self, key: &str) -> Option<bool> {
//         self.get(key).and_then(Nbt::as_boolean)
//     }

//     #[inline]
//     pub fn get_byte(&self, key: &str) -> Option<i8> {
//         self.get(key).and_then(Nbt::as_byte)
//     }

//     #[inline]
//     pub fn get_short(&self, key: &str) -> Option<i16> {
//         self.get(key).and_then(Nbt::as_short)
//     }

//     #[inline]
//     pub fn get_int(&self, key: &str) -> Option<i32> {
//         self.get(key).and_then(Nbt::as_int)
//     }

//     #[inline]
//     pub fn get_long(&self, key: &str) -> Option<i64> {
//         self.get(key).and_then(Nbt::as_long)
//     }

//     #[inline]
//     pub fn get_float(&self, key: &str) -> Option<f32> {
//         self.get(key).and_then(Nbt::as_float)
//     }

//     #[inline]
//     pub fn get_double(&self, key: &str) -> Option<f64> {
//         self.get(key).and_then(Nbt::as_double)
//     }

//     #[inline]
//     pub fn get_byte_array(&self, key: &str) -> Option<&[u8]> {
//         self.get(key).and_then(Nbt::as_byte_array)
//     }

//     #[inline]
//     pub fn get_string(&self, key: &str) -> Option<&str> {
//         self.get(key).and_then(Nbt::as_string)
//     }

//     #[inline]
//     pub fn get_list(&self, key: &str) -> Option<&[Nbt]> {
//         self.get(key).and_then(Nbt::as_list)
//     }

//     #[inline]
//     pub fn get_compound(&self, key: &str) -> Option<&NbtCompound> {
//         self.get(key).and_then(Nbt::as_compound)
//     }

// }


// /// Manual debug implement to shrink the potential huge byte arrays.
// impl fmt::Debug for Nbt {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         match self {
//             Self::Byte(n) => f.debug_tuple("Byte").field(n).finish(),
//             Self::Short(n) => f.debug_tuple("Short").field(n).finish(),
//             Self::Int(n) => f.debug_tuple("Int").field(n).finish(),
//             Self::Long(n) => f.debug_tuple("Long").field(n).finish(),
//             Self::Float(n) => f.debug_tuple("Float").field(n).finish(),
//             Self::Double(n) => f.debug_tuple("Double").field(n).finish(),
//             Self::ByteArray(buf) => {
//                 f.debug_tuple("ByteArray")
//                     .field(&format_args!("({}) {:X?}...", buf.len(), &buf[..buf.len().min(10)]))
//                     .finish()
//             }
//             Self::String(string) => f.debug_tuple("String").field(string).finish(),
//             Self::List(list) => f.debug_tuple("List").field(list).finish(),
//             Self::Compound(compound) => f.debug_tuple("Compound").field(&compound.inner).finish(),
//         }
//     }
// }


/// Error type used together with `RegionResult` for every call on region file methods.
#[derive(thiserror::Error, Debug)]
pub enum NbtError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("{0}")]
    Custom(String),
    #[error("illegal tag type")]
    IllegalTagType,
    #[error("illegal decoded length")]
    IllegalLength,
    #[error("all sequence items should be of the same tag type")]
    IncoherentTagType,
    #[error("sequence length must be known ahead of time")]
    MissingSeqLength,
    #[error("map length must be known ahead of time")]
    MissingMapLength,
    #[error("incoherent amount of items added to sequence, remaining {0}")]
    IncoherentSeqLength(usize),
    #[error("incoherent amount of items added to map, remaining {0}")]
    IncoherentMapLength(usize),
    #[error("illegal type for map key")]
    IllegalKeyType,
}

impl ser::Error for NbtError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

impl de::Error for NbtError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}
