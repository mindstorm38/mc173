//! NBT format serialization and deserialization.

use std::io::{self, Read, Write};
use std::collections::BTreeMap;
use std::fmt;

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


/// A generic NBT tag, this structure has a size of 32 bytes. 
#[derive(Clone, PartialEq)]
pub enum Nbt {
    // Primitive tags.
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<u8>),
    String(String),
    // List tags.
    ListByte(Vec<i8>),
    ListShort(Vec<i16>),
    ListInt(Vec<i32>),
    ListLong(Vec<i64>),
    ListFloat(Vec<f32>),
    ListDouble(Vec<f64>),
    ListByteArray(Vec<Vec<u8>>),
    ListString(Vec<String>),
    ListCompound(Vec<NbtCompound>),
    // Compound tag.
    Compound(NbtCompound),
}

/// An abstract NBT compound type that hides the internal implementation of the mapping.
#[derive(Clone, PartialEq)]
pub struct NbtCompound {
    inner: BTreeMap<String, Nbt>,
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
        NBT_BYTE => Nbt::Byte(reader.read_java_byte()?),
        NBT_SHORT => Nbt::Short(reader.read_java_short()?),
        NBT_INT => Nbt::Int(reader.read_java_int()?),
        NBT_LONG => Nbt::Long(reader.read_java_long()?),
        NBT_FLOAT => Nbt::Float(reader.read_java_float()?),
        NBT_DOUBLE => Nbt::Double(reader.read_java_double()?),
        NBT_BYTE_ARRAY => Nbt::ByteArray(byte_array_from_reader(reader)?),
        NBT_STRING => Nbt::String(reader.read_java_string8()?),
        NBT_LIST => {

            // NOTE: A list can contain a single type.
            let type_id = reader.read_java_byte()?;
            let len: usize = reader.read_java_int()?.try_into().map_err(|_| NbtError::IllegalLength)?;

            fn list_from_reader<T, E>(len: usize, mut func: impl FnMut() -> Result<T, E>) -> Result<Vec<T>, E> {
                let mut list = Vec::with_capacity(len as usize);
                for _ in 0..len {
                    list.push(func()?);
                }
                Ok(list)
            }

            match type_id {
                NBT_BYTE => Nbt::ListByte(list_from_reader(len, || reader.read_java_byte())?),
                NBT_SHORT => Nbt::ListShort(list_from_reader(len, || reader.read_java_short())?),
                NBT_INT => Nbt::ListInt(list_from_reader(len, || reader.read_java_int())?),
                NBT_LONG => Nbt::ListLong(list_from_reader(len, || reader.read_java_long())?),
                NBT_FLOAT => Nbt::ListFloat(list_from_reader(len, || reader.read_java_float())?),
                NBT_DOUBLE => Nbt::ListDouble(list_from_reader(len, || reader.read_java_double())?),
                NBT_BYTE_ARRAY => Nbt::ListByteArray(list_from_reader(len, || byte_array_from_reader(reader))?),
                NBT_STRING => Nbt::ListString(list_from_reader(len, || reader.read_java_string8())?),
                NBT_LIST => return Err(NbtError::IllegalTagType),  // Recursive list.
                NBT_COMPOUND => Nbt::ListCompound(list_from_reader(len, || compound_from_reader(reader))?),
                _ => return Err(NbtError::IllegalTagType),
            }

        }
        NBT_COMPOUND => Nbt::Compound(compound_from_reader(reader)?),
        _ => return Err(NbtError::IllegalTagType),
    })
}

fn byte_array_from_reader(reader: &mut impl Read) -> Result<Vec<u8>, NbtError> {
    let len: usize = reader.read_java_int()?.try_into().map_err(|_| NbtError::IllegalLength)?;
    let mut buf = vec![0u8; len as usize];
    reader.read_exact(&mut buf)?;
    Ok(buf)
}

fn compound_from_reader(reader: &mut impl Read) -> Result<NbtCompound, NbtError> {

    let mut map = BTreeMap::new();

    loop {

        let type_id = reader.read_java_byte()?;
        if type_id == 0 {
            break Ok(NbtCompound { inner: map });  // End tag.
        }

        let key = reader.read_java_string8()?;
        map.insert(key, from_reader_with_type(reader, type_id)?);

    }

}

/// Serialize a NBT tag into a writer.
pub fn to_writer(mut writer: impl Write, tag: &Nbt) -> Result<(), NbtError> {
    writer.write_java_byte(get_nbt_type_id(tag))?;
    writer.write_java_string8("")?; // Root tag has empty key.
    to_writer_raw(&mut writer, tag)
}

/// Internal function to write a NBT tag content.
fn to_writer_raw(writer: &mut impl Write, tag: &Nbt) -> Result<(), NbtError> {

    #[inline(never)]
    fn list_to_writer_generic(writer: &mut impl Write, len: usize, type_id: i8) -> Result<(), NbtError> {
        let len: i32 = len.try_into().map_err(|_| NbtError::IllegalLength)?;
        writer.write_java_byte(type_id)?;
        writer.write_java_int(len)?;
        Ok(())
    }

    #[inline]
    fn list_to_writer<W, T>(writer: &mut W, list: &[T], type_id: i8, mut func: impl FnMut(&mut W, &T) -> Result<(), NbtError>) -> Result<(), NbtError>
    where
        W: Write,
    {
        list_to_writer_generic(writer, list.len(), type_id)?;
        for item in list {
            func(writer, item)?;
        }
        Ok(())
    }

    #[inline]
    fn list_primitive_to_writer<W, T>(writer: &mut W, list: &[T], type_id: i8, mut func: impl FnMut(&mut W, T) -> io::Result<()>) -> Result<(), NbtError>
    where
        W: Write,
        T: Copy,
    {
        list_to_writer(writer, list, type_id, |w, v| func(w, *v).map_err(|e| NbtError::Io(e)))
    }

    match *tag {
        Nbt::Byte(n) => writer.write_java_byte(n)?,
        Nbt::Short(n) => writer.write_java_short(n)?,
        Nbt::Int(n) => writer.write_java_int(n)?,
        Nbt::Long(n) => writer.write_java_long(n)?,
        Nbt::Float(n) => writer.write_java_float(n)?,
        Nbt::Double(n) => writer.write_java_double(n)?,
        Nbt::ByteArray(ref buf) => byte_array_to_writer(writer, &buf)?,
        Nbt::String(ref string) => writer.write_java_string8(&string)?,
        Nbt::ListByte(ref list) => list_primitive_to_writer(writer, &list, NBT_BYTE, WriteJavaExt::write_java_byte)?,
        Nbt::ListShort(ref list) => list_primitive_to_writer(writer, &list, NBT_SHORT, WriteJavaExt::write_java_short)?,
        Nbt::ListInt(ref list) => list_primitive_to_writer(writer, &list, NBT_INT, WriteJavaExt::write_java_int)?,
        Nbt::ListLong(ref list) => list_primitive_to_writer(writer, &list, NBT_LONG, WriteJavaExt::write_java_long)?,
        Nbt::ListFloat(ref list) => list_primitive_to_writer(writer, &list, NBT_FLOAT, WriteJavaExt::write_java_float)?,
        Nbt::ListDouble(ref list) => list_primitive_to_writer(writer, &list, NBT_DOUBLE, WriteJavaExt::write_java_double)?,
        Nbt::ListByteArray(ref list) => list_to_writer(writer, &list, NBT_BYTE_ARRAY, byte_array_to_writer)?,
        Nbt::ListString(ref list) => list_to_writer(writer, &list, NBT_STRING, |w, v| {
            w.write_java_string8(&v)?;
            Ok(())
        })?,
        Nbt::ListCompound(ref list) => list_to_writer(writer, &list, NBT_COMPOUND, compound_to_writer)?,
        Nbt::Compound(ref compound) => compound_to_writer(writer, compound)?,
    }

    Ok(())

}

// NOTE: Intentionally using &Vec<u8>, it simplifies passing as closure..
fn byte_array_to_writer(writer: &mut impl Write, buf: &Vec<u8>) -> Result<(), NbtError> {
    let len: i32 = buf.len().try_into().map_err(|_| NbtError::IllegalLength)?;
    writer.write_java_int(len)?;
    writer.write_all(&buf)?;
    Ok(())
}

fn compound_to_writer(writer: &mut impl Write, compound: &NbtCompound) -> Result<(), NbtError> {
    
    for (key, tag) in &compound.inner {
        writer.write_java_byte(get_nbt_type_id(tag))?;
        writer.write_java_string8(&key)?;
        to_writer_raw(writer, tag)?;
    }

    writer.write_java_byte(0)?;
    Ok(())

}

/// Internal function to get the NBT type id of a tag.
fn get_nbt_type_id(tag: &Nbt) -> i8 {
    match tag {
        Nbt::Byte(_) => NBT_BYTE,
        Nbt::Short(_) => NBT_SHORT,
        Nbt::Int(_) => NBT_INT,
        Nbt::Long(_) => NBT_LONG,
        Nbt::Float(_) => NBT_FLOAT,
        Nbt::Double(_) => NBT_DOUBLE,
        Nbt::ByteArray(_) => NBT_BYTE_ARRAY,
        Nbt::String(_) => NBT_STRING,
        Nbt::ListByte(_) |
        Nbt::ListShort(_) |
        Nbt::ListInt(_) |
        Nbt::ListLong(_) |
        Nbt::ListFloat(_) |
        Nbt::ListDouble(_) |
        Nbt::ListByteArray(_) |
        Nbt::ListString(_) |
        Nbt::ListCompound(_) => NBT_LIST,
        Nbt::Compound(_) => NBT_COMPOUND,
    }
}


/// Basic methods to interpret a tag as its inner type if possible.
impl Nbt {

    #[inline]
    pub fn as_boolean(&self) -> Option<bool> {
        self.as_byte().map(|b| b != 0)
    }

    #[inline]
    pub fn as_byte(&self) -> Option<i8> {
        match *self {
            Self::Byte(n) => Some(n),
            _ => None
        }
    }

    #[inline]
    pub fn as_short(&self) -> Option<i16> {
        match *self {
            Self::Short(n) => Some(n),
            _ => None
        }
    }

    #[inline]
    pub fn as_int(&self) -> Option<i32> {
        match *self {
            Self::Int(n) => Some(n),
            _ => None
        }
    }

    #[inline]
    pub fn as_long(&self) -> Option<i64> {
        match *self {
            Self::Long(n) => Some(n),
            _ => None
        }
    }

    #[inline]
    pub fn as_float(&self) -> Option<f32> {
        match *self {
            Self::Float(n) => Some(n),
            _ => None
        }
    }

    #[inline]
    pub fn as_double(&self) -> Option<f64> {
        match *self {
            Self::Double(n) => Some(n),
            _ => None
        }
    }

    #[inline]
    pub fn as_byte_array(&self) -> Option<&[u8]> {
        match self {
            Self::ByteArray(buf) => Some(&buf[..]),
            _ => None
        }
    }

    #[inline]
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(string) => Some(string.as_str()),
            _ => None
        }
    }

    #[inline]
    pub fn as_list(&self) -> Option<&[Nbt]> {
        match self {
            Self::List(list) => Some(&list[..]),
            _ => None
        }
    }

    #[inline]
    pub fn as_compound(&self) -> Option<&NbtCompound> {
        match self {
            Self::Compound(comp) => Some(comp),
            _ => None
        }
    }

    pub fn parse(&self) -> NbtParse<'_> {
        NbtParse { inner: self, path: String::new() }
    }

}

/// Basic methods to create and manage keys in a compound.
impl NbtCompound {

    pub fn new() -> Self {
        Self { inner: BTreeMap::new() }
    }

    #[inline]
    pub fn insert(&mut self, key: String, tag: Nbt) {
        self.inner.insert(key, tag);
    }

    #[inline]
    pub fn get(&self, key: &str) -> Option<&Nbt> {
        self.inner.get(key)
    }

    #[inline]
    pub fn get_boolean(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Nbt::as_boolean)
    }

    #[inline]
    pub fn get_byte(&self, key: &str) -> Option<i8> {
        self.get(key).and_then(Nbt::as_byte)
    }

    #[inline]
    pub fn get_short(&self, key: &str) -> Option<i16> {
        self.get(key).and_then(Nbt::as_short)
    }

    #[inline]
    pub fn get_int(&self, key: &str) -> Option<i32> {
        self.get(key).and_then(Nbt::as_int)
    }

    #[inline]
    pub fn get_long(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Nbt::as_long)
    }

    #[inline]
    pub fn get_float(&self, key: &str) -> Option<f32> {
        self.get(key).and_then(Nbt::as_float)
    }

    #[inline]
    pub fn get_double(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Nbt::as_double)
    }

    #[inline]
    pub fn get_byte_array(&self, key: &str) -> Option<&[u8]> {
        self.get(key).and_then(Nbt::as_byte_array)
    }

    #[inline]
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Nbt::as_string)
    }

    #[inline]
    pub fn get_list(&self, key: &str) -> Option<&[Nbt]> {
        self.get(key).and_then(Nbt::as_list)
    }

    #[inline]
    pub fn get_compound(&self, key: &str) -> Option<&NbtCompound> {
        self.get(key).and_then(Nbt::as_compound)
    }

}


/// Manual debug implement to shrink the potential huge byte arrays.
impl fmt::Debug for Nbt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Byte(n) => f.debug_tuple("Byte").field(n).finish(),
            Self::Short(n) => f.debug_tuple("Short").field(n).finish(),
            Self::Int(n) => f.debug_tuple("Int").field(n).finish(),
            Self::Long(n) => f.debug_tuple("Long").field(n).finish(),
            Self::Float(n) => f.debug_tuple("Float").field(n).finish(),
            Self::Double(n) => f.debug_tuple("Double").field(n).finish(),
            Self::ByteArray(buf) => {
                f.debug_tuple("ByteArray")
                    .field(&format_args!("({}) {:X?}...", buf.len(), &buf[..buf.len().min(10)]))
                    .finish()
            }
            Self::String(string) => f.debug_tuple("String").field(string).finish(),
            Self::List(list) => f.debug_tuple("List").field(list).finish(),
            Self::Compound(compound) => f.debug_tuple("Compound").field(&compound.inner).finish(),
        }
    }
}


/// Error type used together with `RegionResult` for every call on region file methods.
#[derive(thiserror::Error, Debug)]
pub enum NbtError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("Illegal tag type.")]
    IllegalTagType,
    #[error("Illegal decoded length.")]
    IllegalLength,
}


/// Parsing utility structure for anonymous NBT data.
pub struct NbtParse<'nbt> {
    /// Reference to the parsed NBT data.
    inner: &'nbt Nbt,
    /// Current path being parsed, used to return relevant errors.
    path: String,
}

impl<'nbt> NbtParse<'nbt> {
    
    #[inline]
    fn make_error(self, kind: NbtParseExpected) -> NbtParseError {
        NbtParseError {
            path: self.path,
            expected: kind,
        }
    }

    #[inline]
    pub fn as_boolean(self) -> Result<bool, NbtParseError> {
        self.as_byte().map(|b| b != 0)
    }

    #[inline]
    pub fn as_byte(self) -> Result<i8, NbtParseError> {
        self.inner.as_byte().ok_or_else(|| self.make_error(NbtParseExpected::Byte))
    }

    #[inline]
    pub fn as_short(self) -> Result<i16, NbtParseError> {
        self.inner.as_short().ok_or_else(|| self.make_error(NbtParseExpected::Short))
    }

    #[inline]
    pub fn as_int(self) -> Result<i32, NbtParseError> {
        self.inner.as_int().ok_or_else(|| self.make_error(NbtParseExpected::Int))
    }

    #[inline]
    pub fn as_long(self) -> Result<i64, NbtParseError> {
        self.inner.as_long().ok_or_else(|| self.make_error(NbtParseExpected::Long))
    }

    #[inline]
    pub fn as_float(self) -> Result<f32, NbtParseError> {
        self.inner.as_float().ok_or_else(|| self.make_error(NbtParseExpected::Float))
    }

    #[inline]
    pub fn as_double(self) -> Result<f64, NbtParseError> {
        self.inner.as_double().ok_or_else(|| self.make_error(NbtParseExpected::Double))
    }

    #[inline]
    pub fn as_byte_array(self) -> Result<&'nbt [u8], NbtParseError> {
        self.inner.as_byte_array().ok_or_else(|| self.make_error(NbtParseExpected::ByteArray))
    }

    #[inline]
    pub fn as_string(self) -> Result<&'nbt str, NbtParseError> {
        self.inner.as_string().ok_or_else(|| self.make_error(NbtParseExpected::String))
    }

    #[inline]
    pub fn as_compound(self) -> Result<NbtCompoundParse<'nbt>, NbtParseError> {
        // If successful we wrap the compound into a parse structure to keep the path.
        match self.inner.as_compound() {
            Some(compound) => Ok(NbtCompoundParse {
                inner: compound,
                path: self.path,
            }),
            None => Err(self.make_error(NbtParseExpected::Compound))
        }
    }

    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[inline]
    pub fn inner(&self) -> &'nbt Nbt {
        &self.inner
    }

}

/// Parsing utility structure for a NBT compound.
pub struct NbtCompoundParse<'nbt> {
    /// Reference to the parsed NBT data.
    inner: &'nbt NbtCompound,
    /// Current path being parsed, used to return relevant errors.
    path: String,
}

impl<'nbt> NbtCompoundParse<'nbt> {

    /// Get a item from its key in this compound.
    pub fn get(&self, key: &str) -> Result<NbtParse<'nbt>, NbtParseError> {
        let path = format!("{}/{key}", self.path);
        match self.inner.get(key) {
            Some(inner) => Ok(NbtParse { 
                inner, 
                path,
            }),
            None => Err(NbtParseError { 
                path, 
                expected: NbtParseExpected::Item,
            })
        }
    }

    #[inline]
    pub fn get_boolean(&self, key: &str) -> Result<bool, NbtParseError> {
        self.get(key).and_then(NbtParse::as_boolean)
    }

    #[inline]
    pub fn get_byte(&self, key: &str) -> Result<i8, NbtParseError> {
        self.get(key).and_then(NbtParse::as_byte)
    }

    #[inline]
    pub fn get_short(&self, key: &str) -> Result<i16, NbtParseError> {
        self.get(key).and_then(NbtParse::as_short)
    }

    #[inline]
    pub fn get_int(&self, key: &str) -> Result<i32, NbtParseError> {
        self.get(key).and_then(NbtParse::as_int)
    }

    #[inline]
    pub fn get_long(&self, key: &str) -> Result<i64, NbtParseError> {
        self.get(key).and_then(NbtParse::as_long)
    }

    #[inline]
    pub fn get_float(&self, key: &str) -> Result<f32, NbtParseError> {
        self.get(key).and_then(NbtParse::as_float)
    }

    #[inline]
    pub fn get_double(&self, key: &str) -> Result<f64, NbtParseError> {
        self.get(key).and_then(NbtParse::as_double)
    }

    #[inline]
    pub fn get_byte_array(&self, key: &str) -> Result<&'nbt [u8], NbtParseError> {
        self.get(key).and_then(NbtParse::as_byte_array)
    }

    #[inline]
    pub fn get_string(&self, key: &str) -> Result<&'nbt str, NbtParseError> {
        self.get(key).and_then(NbtParse::as_string)
    }

    #[inline]
    pub fn get_compound(&self, key: &str) -> Result<NbtCompoundParse<'nbt>, NbtParseError> {
        self.get(key).and_then(NbtParse::as_compound)
    }

    #[inline]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[inline]
    pub fn inner(&self) -> &'nbt NbtCompound {
        &self.inner
    }

}


/// A parsing error as returned by [`NbtParse`] and [`NbtCompoundParse`] wrappers.
#[derive(thiserror::Error, Debug)]
#[error("{path}: expected {expected:?}")]
pub struct NbtParseError {
    /// The path to the failed parsing.
    pub path: String,
    pub expected: NbtParseExpected,
}

/// A type of expected value for a [`NbtParseError`].
#[derive(Debug)]
pub enum NbtParseExpected {
    /// Expected a compound or list item at this path.
    Item,
    Byte,
    Short,
    Int,
    Long,
    Float,
    Double,
    ByteArray,
    String,
    List,
    Compound,
}


#[cfg(test)]
mod tests {

    use super::*;

    fn all() -> Result<(), NbtParseError> {

        let nbt = Nbt::Byte(8);

        let parser = nbt.parse();
        parser.as_byte_array()?;

        Ok(())

    }
    
}