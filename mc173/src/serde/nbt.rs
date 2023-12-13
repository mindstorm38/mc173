//! NBT format serialization and deserialization.

use std::io::{self, Read, Write};
use std::fmt::{self, Write as _};

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
    let mut path = String::new();

    value.serialize(NbtSerializer {
        writer: &mut writer,
        path: &mut path,
        next_key: &mut next_key,
        seq_remaining_len: 0,
        seq_element_type_id: None,
        seq_type_id: None,
        in_key: false,
    }).map_err(|kind| NbtError { 
        path,
        kind 
    })

}

/// Deserialize a NBT tag from a reader.
pub fn from_reader<'de, D: de::Deserialize<'de>>(mut reader: impl Read) -> Result<D, NbtError> {

    let mut path = String::new();

    D::deserialize(NbtDeserializer {
        reader: &mut reader,
        path: &mut path,
        state: NbtDeserializerState::Root,
    }).map_err(|kind| NbtError { 
        path,
        kind 
    })

}


/// A NBT serializer around an arbitrary I/O writer.
/// 
/// NOTE: We are not using enumeration for the state because serde already defines the 
/// state using a strict type system.
struct NbtSerializer<'a, W> {
    /// The inner writer.
    writer: &'a mut W,
    /// The current path of the serializer, this is used for better diagnostic in errors.
    path: &'a mut String,
    /// The key to write for the next serialized value.
    next_key: &'a mut String,
    /// Length remaining in the sequence. When serializing the first sequence element, 
    /// this is also used to write the sequence header.
    seq_remaining_len: usize,
    /// If the current serializer is for a sequence element, then this should be set to
    /// a reference to the required sequence type id. If the sequence type is id is None
    /// then it should be set to the type, while also writing the sequence header.
    seq_element_type_id: Option<&'a mut Option<i8>>,
    /// If the current serializer is on a sequence, then this represent the current type
    /// of the sequence being serialized.
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
    fn write_key(&mut self, value_type_id: i8) -> Result<(), NbtErrorKind> {
        
        // We cannot write any key while serializing a key.
        if self.in_key {
            return Err(NbtErrorKind::IllegalKeyType);
        }

        // If we are serializing a sequence element, check its type or set it.
        if let Some(seq_element_type_id) = &mut self.seq_element_type_id {

            // If we are writing sequence element, this require no header.
            if let Some(seq_type_id) = **seq_element_type_id {
                if seq_type_id != value_type_id {
                    return Err(NbtErrorKind::IncoherentTagType);
                }
            } else {
                // This is the first element in the sequence, so remaining length
                // should contains the full length, we set the type id required for
                // elements inserted after it.
                // NOTE: Cast is safe because we checked it when creating sequence.
                self.writer.write_java_byte(value_type_id)?;
                self.writer.write_java_int(self.seq_remaining_len as i32)?;
                **seq_element_type_id = Some(value_type_id);
            }

        } else {
            // If we are not writing sequence element, just write regular header.
            self.writer.write_java_byte(value_type_id)?;
            self.writer.write_java_string8(&self.next_key)?;
        }

        Ok(())

    }

}

impl<W: Write> ser::Serializer for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtErrorKind;
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
        let len: i32 = v.len().try_into().map_err(|_| NbtErrorKind::IllegalLength)?;
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
                return Err(NbtErrorKind::IllegalLength);
            }

            // Modify the current state to a sequence and return itself,
            self.seq_remaining_len = len as usize;
            Ok(self)

        } else {
            Err(NbtErrorKind::MissingSeqLength)
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

    fn serialize_map(mut self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        self.write_key(NBT_COMPOUND)?;
        Ok(self)
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
    type Error = NbtErrorKind;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize 
    {

        let remaining_len = self.seq_remaining_len;
        if remaining_len == 0 {
            return Err(NbtErrorKind::IncoherentSeqLength(0));
        }
        
        let path_len = self.path.len();
        write!(self.path, "/-{remaining_len}").unwrap();

        // We also pass the next key to avoid reallocation.
        value.serialize(NbtSerializer {
            writer: &mut *self.writer,
            path: &mut *self.path,
            next_key: &mut *self.next_key,
            seq_remaining_len: remaining_len, // This is only used for first element.
            seq_element_type_id: Some(&mut self.seq_type_id),
            seq_type_id: None,
            in_key: false,
        })?;

        // Revert path.
        self.path.truncate(path_len);
        self.seq_remaining_len = remaining_len - 1;
        Ok(())

    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        if self.seq_remaining_len != 0 {
            Err(NbtErrorKind::IncoherentSeqLength(self.seq_remaining_len))
        } else {
            Ok(())
        }
    }

}

impl<W: Write> ser::SerializeTuple for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtErrorKind;

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
    type Error = NbtErrorKind;

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
    type Error = NbtErrorKind;

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
    type Error = NbtErrorKind;

    fn serialize_key<T: ?Sized>(&mut self, key: &T) -> Result<(), Self::Error>
    where
        T: serde::Serialize
    {

        assert!(!self.in_key, "missing call to serialize_value");

        key.serialize(NbtSerializer {
            writer: &mut *self.writer,
            path: &mut *self.path,
            next_key: &mut *self.next_key,
            seq_remaining_len: 0,
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

        let path_len = self.path.len();
        write!(self.path, "/{}", self.next_key).unwrap();

        value.serialize(NbtSerializer {
            writer: &mut *self.writer,
            next_key: &mut *self.next_key,
            path: &mut *self.path,
            seq_remaining_len: 0,
            seq_element_type_id: None,
            seq_type_id: None,
            in_key: false,
        })?;

        // Mark that the value has been serialized, key can be serialized again.
        self.in_key = false;

        // Revert path.
        self.path.truncate(path_len);
        Ok(())

    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        // Zero should be written after last element of a compound.
        self.writer.write_java_byte(0)?;
        Ok(())
    }

}

impl<W: Write> ser::SerializeStruct for NbtSerializer<'_, W> {

    type Ok = ();
    type Error = NbtErrorKind;

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
    type Error = NbtErrorKind;

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
struct NbtDeserializer<'a, R> {
    /// Inner reader.
    reader: &'a mut R,
    /// Debug path.
    path: &'a mut String,
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
    /// Inner reader.
    reader: &'a mut R,
    /// Debug path.
    path: &'a mut String,
    /// Initial debug path length.
    path_len: usize,
    /// Type id of tags in the sequence.
    type_id: i8,
    /// Length of the sequence.
    len: usize,
    /// Current index within the sequence.
    index: usize,
}

/// A NBT deserializer for a map.
struct NbtMapDeserializer<'a, R> {
    /// Inner reader.
    reader: &'a mut R,
    /// Debug path.
    path: &'a mut String,
    /// Initial path length, used to truncate back the path.
    path_len: usize,
    /// Type id of the next value, none if `next_key` must be called before.
    next_type_id: Option<i8>,
}

#[derive(Debug)]
enum NbtDeserializerHint {
    /// Default type should be returned.
    Default,
    /// Boolean should be returned if possible.
    Bool,
    /// Unsigned variant of the number should be returned.
    Unsigned,
    /// An option should be deserialized.
    /// TODO: Improve support for options when deserializing!
    Option,
}

impl<R: Read> NbtDeserializer<'_, R> {

    /// Internal helper function to deserialize any value, much like serde 
    /// `deserialize_any` but with a hint about the expected value variant.
    fn deserialize_any_hint<'de, V>(mut self, visitor: V, hint: NbtDeserializerHint) -> Result<V::Value, NbtErrorKind>
    where
        V: de::Visitor<'de> 
    {

        let type_id;
        match  self.state {
            NbtDeserializerState::Root => {

                type_id = self.reader.read_java_byte()?;
                if type_id == 0 {
                    // Root value cannot be of the end type.
                    return Err(NbtErrorKind::IllegalTagType);
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
                self.path.push_str("<byte>");
                let val = self.reader.read_java_byte()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u8(val as u8),
                    NbtDeserializerHint::Bool => visitor.visit_bool(val != 0),
                    NbtDeserializerHint::Option if val == 0 => visitor.visit_none(),
                    _ => visitor.visit_i8(val)
                }
            }
            NBT_SHORT => {
                self.path.push_str("<short>");
                let val = self.reader.read_java_short()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u16(val as u16),
                    _ => visitor.visit_i16(val)
                }
            }
            NBT_INT => {
                self.path.push_str("<int>");
                let val = self.reader.read_java_int()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u32(val as u32),
                    _ => visitor.visit_i32(val)
                }
            }
            NBT_LONG => {
                self.path.push_str("<long>");
                let val = self.reader.read_java_long()?;
                match hint {
                    NbtDeserializerHint::Unsigned => visitor.visit_u64(val as u64),
                    _ => visitor.visit_i64(val)
                }
            }
            NBT_FLOAT => {
                self.path.push_str("<float>");
                visitor.visit_f32(self.reader.read_java_float()?)
            }
            NBT_DOUBLE => {
                self.path.push_str("<double>");
                visitor.visit_f64(self.reader.read_java_double()?)
            }
            NBT_BYTE_ARRAY => {

                self.path.push_str("<bytes>");

                let len = self.reader.read_java_int()?;
                if len < 0 {
                    return Err(NbtErrorKind::IllegalLength);
                }

                let mut buf = vec![0u8; len as usize];
                self.reader.read_exact(&mut buf)?;
                visitor.visit_byte_buf(buf)

            }
            NBT_STRING => visitor.visit_string(self.reader.read_java_string8()?),
            NBT_LIST => {

                self.path.push_str("<list>");

                // NOTE: A list can contain a single type.
                let type_id = self.reader.read_java_byte()?;
                let len = self.reader.read_java_int()?;
                if len < 0 {
                    return Err(NbtErrorKind::IllegalLength);
                }

                visitor.visit_seq(NbtSeqDeserializer {
                    reader: self.reader,
                    path_len: self.path.len(),
                    path: self.path,
                    type_id,
                    len: len as usize,
                    index: 0,
                })

            }
            NBT_COMPOUND => {

                self.path.push_str("<compound>");
                
                visitor.visit_map(NbtMapDeserializer {
                    reader: self.reader,
                    path_len: self.path.len(),
                    path: self.path,
                    next_type_id: None,
                })

            }
            _ => return Err(NbtErrorKind::IllegalTagType)
        }

    }
    
}

impl<'de, R: Read> de::Deserializer<'de> for NbtDeserializer<'_, R> {

    type Error = NbtErrorKind;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Default)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Bool)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Unsigned)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: de::Visitor<'de> 
    {
        self.deserialize_any_hint(visitor, NbtDeserializerHint::Option)
    }

    serde::forward_to_deserialize_any! {
        i8 i16 i32 i64 f32 f64 char str string
        bytes byte_buf unit unit_struct newtype_struct seq tuple
        tuple_struct map struct enum identifier ignored_any
    }

}

impl<'de, R: Read> de::SeqAccess<'de> for NbtSeqDeserializer<'_, R> {

    type Error = NbtErrorKind;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: de::DeserializeSeed<'de> 
    {
       
        if self.index >= self.len {
            return Ok(None);
        }
        
        // Reset to the initial path length before appending key.
        self.path.truncate(self.path_len);
        write!(self.path, "/{}", self.index).unwrap();

        self.index += 1;
        
        seed.deserialize(NbtDeserializer {
            reader: &mut *self.reader,
            path: &mut *self.path,
            state: NbtDeserializerState::SeqValue(self.type_id),
        }).map(Some)

    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.len)
    }

}

impl<'de, R: Read> de::MapAccess<'de> for NbtMapDeserializer<'_, R> {

    type Error = NbtErrorKind;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: de::DeserializeSeed<'de>
    {

        let type_id = self.reader.read_java_byte()?;
        
        // End of map tag.
        if type_id == 0 {
            self.next_type_id = None;
            return Ok(None);
        }

        let key = self.reader.read_java_string8()?;
        self.next_type_id = Some(type_id);
        
        // Reset to the initial path length before appending key.
        self.path.truncate(self.path_len);
        write!(self.path, "/{key}").unwrap();

        seed.deserialize(NbtDeserializer {
            reader: &mut *self.reader,
            path: &mut *self.path,
            state: NbtDeserializerState::MapKey(Some(key)),
        }).map(Some)

    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: de::DeserializeSeed<'de> 
    {

        let type_id = self.next_type_id.take().expect("missing next key");

        seed.deserialize(NbtDeserializer {
            reader: &mut *self.reader,
            path: &mut *self.path,
            state: NbtDeserializerState::MapValue(Some(type_id)),
        })

    }

}


#[derive(thiserror::Error, Debug)]
#[error("{kind} ({path})")]
pub struct NbtError {
    pub path: String,
    pub kind: NbtErrorKind,
}


/// Error type used together with `RegionResult` for every call on region file methods.
#[derive(thiserror::Error, Debug)]
pub enum NbtErrorKind {
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
    #[error("incoherent amount of items added to sequence, remaining {0}")]
    IncoherentSeqLength(usize),
    #[error("illegal type for map key")]
    IllegalKeyType,
}

impl ser::Error for NbtErrorKind {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}

impl de::Error for NbtErrorKind {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self::Custom(msg.to_string())
    }
}



#[cfg(test)]
mod tests {

    use std::fmt::Debug;
    use std::io::Cursor;
    use super::*;

    fn test_value<'de, V>(value: V, bytes: &[u8])
    where
        V: serde::Serialize,
        V: serde::Deserialize<'de>,
        V: PartialEq + Debug,
    {
        
        let mut data = Vec::new();
        to_writer(&mut data, &value).expect("failed to write");
        assert_eq!(data, bytes, "invalid written value");

        let mut cursor = Cursor::new(bytes);
        let read_value: V = from_reader(&mut cursor).expect("failed to read");
        assert_eq!(value, read_value, "invalid read value");
        assert_eq!(cursor.position(), bytes.len() as u64, "not all data has been read");

    }

    #[test]
    fn primitives() {
        test_value(0x12u8,                  &[NBT_BYTE as u8,   0, 0, 0x12]);
        test_value(0x1234u16,               &[NBT_SHORT as u8,  0, 0, 0x12, 0x34]);
        test_value(0x12345678u32,           &[NBT_INT as u8,    0, 0, 0x12, 0x34, 0x56, 0x78]);
        test_value(0x123456789ABCDEF0u64,   &[NBT_LONG as u8,   0, 0, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0xDE, 0xF0]);
        test_value(3141592.5f32,            &[NBT_FLOAT as u8,  0, 0, 0x4A, 0x3F, 0xBF, 0x62]);
        test_value(3141592.5f64,            &[NBT_DOUBLE as u8, 0, 0, 0x41, 0x47, 0xF7, 0xEC, 0x40, 0x00, 0x00, 0x00]);
        test_value(format!("hello"),        &[NBT_STRING as u8, 0, 0, 0, 5, 0x68, 0x65, 0x6C, 0x6C, 0x6F]);
    }

    #[test]
    fn lists() {

        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug, Clone, Copy)]
        struct Compound {
            key0: bool,
        }

        test_value([0u8; 0],                &[NBT_LIST as u8,   0, 0, NBT_BYTE as u8,   0, 0, 0, 0]);
        test_value([0x12u8; 3],             &[NBT_LIST as u8,   0, 0, NBT_BYTE as u8,   0, 0, 0, 3, 0x12, 0x12, 0x12]);
        test_value([0x1234u16; 2],          &[NBT_LIST as u8,   0, 0, NBT_SHORT as u8,  0, 0, 0, 2, 0x12, 0x34, 0x12, 0x34]);

        test_value([Compound { key0: true }; 2], &[
            NBT_LIST as u8,     0, 0, NBT_COMPOUND as u8, 0, 0, 0, 2, // List header
            NBT_BYTE as u8,     0, 4, 0x6B, 0x65, 0x79, 0x30, 0x01, 0, // key0 header + value + terminating byte
            NBT_BYTE as u8,     0, 4, 0x6B, 0x65, 0x79, 0x30, 0x01, 0, // key0 header + value + terminating byte
        ]);

    }

    #[test]
    #[should_panic]
    fn lists_err() {
        test_value((0, format!("hello")), &[]);
    }

    #[test]
    fn compounds() {

        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct EmptyCompound {}

        #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
        struct Compound {
            key0: String,
            key1: bool,
            key2: f32,
        }

        test_value(EmptyCompound {},        &[NBT_COMPOUND as u8, 0, 0, 0]);

        let comp = Compound {
            key0: format!("hello"),
            key1: true,
            key2: 3141592.5f32,
        };

        test_value(comp, &[
            NBT_COMPOUND as u8, 0, 0, // Compound header
            NBT_STRING as u8,   0, 4, 0x6B, 0x65, 0x79, 0x30, 0, 5, 0x68, 0x65, 0x6C, 0x6C, 0x6F, // key0 header + value
            NBT_BYTE as u8,     0, 4, 0x6B, 0x65, 0x79, 0x31, 0x01, // key1 header + value
            NBT_FLOAT as u8,    0, 4, 0x6B, 0x65, 0x79, 0x32, 0x4A, 0x3F, 0xBF, 0x62, // key2 header + value
            0 // terminating byte
        ]);

    }

}
