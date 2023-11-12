//! NBT format serialization and deserialization.

use std::collections::BTreeMap;
use std::io::{Read, self, Write};

use thiserror::Error;

use crate::util::{ReadJavaExt, WriteJavaExt};


/// A generic NBT tag, this structure has a size of 32 bytes. 
#[derive(Debug, Clone, PartialEq)]
pub enum Nbt {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    List(Vec<Nbt>),
    Compound(BTreeMap<String, Nbt>),
}


/// Deserialize a NBT tag from a reader.
pub fn from_reader(mut reader: impl Read) -> Result<Nbt, NbtError> {

    let type_id = reader.read_java_byte()?;
    if type_id == 0 {
        // We should not get a end tag directly.
        return Err(NbtError::IllegalTagType);
    }

    let _key = reader.read_java_string8()?;
    from_reader_with_type(&mut reader, type_id)
    
}

/// Internal function to read a NBT tag of a specific type.
fn from_reader_with_type(reader: &mut impl Read, type_id: i8) -> Result<Nbt, NbtError> {
    Ok(match type_id {
        1 => Nbt::Byte(reader.read_java_byte()?),
        2 => Nbt::Short(reader.read_java_short()?),
        3 => Nbt::Int(reader.read_java_int()?),
        4 => Nbt::Long(reader.read_java_long()?),
        5 => Nbt::Float(reader.read_java_float()?),
        6 => Nbt::Double(reader.read_java_double()?),
        7 => {
            
            let len = reader.read_java_int()?;
            if len < 0 {
                return Err(NbtError::IllegalLength);
            }

            let mut buf = vec![0u8; len as usize];
            reader.read_exact(&mut buf)?;

            Nbt::ByteArray(buf)

        }
        8 => Nbt::String(reader.read_java_string8()?),
        9 => {

            // NOTE: A list can contain a single type.
            let type_id = reader.read_java_byte()?;
            let len = reader.read_java_int()?;
            
            if len < 0 {
                return Err(NbtError::IllegalLength);
            }

            let mut list = Vec::with_capacity(len as usize);
            for _ in 0..len {
                list.push(from_reader_with_type(reader, type_id)?);
            }

            Nbt::List(list)

        }
        10 => {

            let mut map = BTreeMap::new();

            loop {

                let type_id = reader.read_java_byte()?;
                if type_id == 0 {
                    break Nbt::Compound(map);  // End tag.
                }

                let key = reader.read_java_string8()?;
                map.insert(key, from_reader_with_type(reader, type_id)?);

            }

        }
        _ => return Err(NbtError::IllegalTagType),
    })
}

/// Serialize a NBT tag into a writer.
pub fn to_writer(mut writer: impl Write, tag: &Nbt) -> Result<(), NbtError> {
    writer.write_java_byte(get_nbt_type_id(tag))?;
    writer.write_java_string8("")?; // Root tag has empty key.
    to_writer_raw(&mut writer, tag)
}

/// Internal function to write a NBT tag content.
fn to_writer_raw(writer: &mut impl Write, tag: &Nbt) -> Result<(), NbtError> {

    match *tag {
        Nbt::Byte(n) => writer.write_java_byte(n)?,
        Nbt::Short(n) => writer.write_java_short(n)?,
        Nbt::Int(n) => writer.write_java_int(n)?,
        Nbt::Long(n) => writer.write_java_long(n)?,
        Nbt::Float(n) => writer.write_java_float(n)?,
        Nbt::Double(n) => writer.write_java_double(n)?,
        Nbt::ByteArray(ref buf) => {
            let len: i32 = buf.len().try_into().map_err(|_| NbtError::IllegalLength)?;
            writer.write_java_int(len)?;
            writer.write_all(&buf)?;
        }
        Nbt::String(ref string) => writer.write_java_string8(&string)?,
        Nbt::List(ref list) => {

            let len: i32 = list.len().try_into().map_err(|_| NbtError::IllegalLength)?;
            let type_id = list.get(0).map(get_nbt_type_id).unwrap_or(1);
            writer.write_java_byte(type_id)?;
            writer.write_java_int(len)?;

            for tag in list {
                let tag_type_id = get_nbt_type_id(tag);
                if tag_type_id != type_id {
                    return Err(NbtError::IllegalTagType);
                }
                to_writer_raw(writer, tag)?;
            }

        }
        Nbt::Compound(ref map) => {

            for (key, tag) in map {
                writer.write_java_byte(get_nbt_type_id(tag))?;
                writer.write_java_string8(&key)?;
                to_writer_raw(writer, tag)?;
            }

            writer.write_java_byte(0)?;

        }
    }

    Ok(())

}

/// Internal function to get the NBT type id of a tag.
fn get_nbt_type_id(tag: &Nbt) -> i8 {
    match tag {
        Nbt::Byte(_) => 1,
        Nbt::Short(_) => 2,
        Nbt::Int(_) => 3,
        Nbt::Long(_) => 4,
        Nbt::Float(_) => 5,
        Nbt::Double(_) => 6,
        Nbt::ByteArray(_) => 7,
        Nbt::String(_) => 8,
        Nbt::List(_) => 9,
        Nbt::Compound(_) => 10,
    }
}


/// Error type used together with `RegionResult` for every call on region file methods.
#[derive(Error, Debug)]
pub enum NbtError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("Illegal tag type.")]
    IllegalTagType,
    #[error("Illegal decoded length.")]
    IllegalLength,
}
