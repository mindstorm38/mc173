//! This module provides read and write extension traits for Java types.

use byteorder::{ReadBytesExt, WriteBytesExt, BE};
use std::io::{self, Read, Write};


/// Extension trait with Minecraft-specific packet read methods.
pub trait ReadPacketExt: Read {

    fn read_java_byte(&mut self) -> io::Result<i8> {
        ReadBytesExt::read_i8(self)
    }

    fn read_java_short(&mut self) -> io::Result<i16> {
        ReadBytesExt::read_i16::<BE>(self)
    }

    fn read_java_int(&mut self) -> io::Result<i32> {
        ReadBytesExt::read_i32::<BE>(self)
    }

    fn read_java_long(&mut self) -> io::Result<i64> {
        ReadBytesExt::read_i64::<BE>(self)
    }

    fn read_java_float(&mut self) -> io::Result<f32> {
        ReadBytesExt::read_f32::<BE>(self)
    }

    fn read_java_double(&mut self) -> io::Result<f64> {
        ReadBytesExt::read_f64::<BE>(self)
    }

    fn read_java_boolean(&mut self) -> io::Result<bool> {
        Ok(self.read_java_byte()? != 0)
    }

    fn read_java_char(&mut self) -> io::Result<char> {
        // FIXME: Read real UTF-16 char.
        Ok(ReadBytesExt::read_u16::<BE>(self)? as u8 as char)
    }

    fn read_java_string16(&mut self, max_len: usize) -> io::Result<String> {
        
        let len = self.read_java_short()?;
        if len < 0 {
            return Err(new_invalid_data_err("negative length string"));
        }

        if len as usize > max_len {
            return Err(new_invalid_data_err("excedeed max string length"));
        }

        let mut ret = String::new();
        for _ in 0..len {
            ret.push(self.read_java_char()?);
        }

        Ok(ret)

    }

}

/// Extension trait with Minecraft-specific packet read methods.
pub trait WritePacketExt: Write {

    fn write_java_byte(&mut self, b: i8) -> io::Result<()> {
        WriteBytesExt::write_i8(self, b)
    }

    fn write_java_short(&mut self, s: i16) -> io::Result<()> {
        WriteBytesExt::write_i16::<BE>(self, s)
    }

    fn write_java_int(&mut self, i: i32) -> io::Result<()> {
        WriteBytesExt::write_i32::<BE>(self, i)
    }

    fn write_java_long(&mut self, l: i64) -> io::Result<()> {
        WriteBytesExt::write_i64::<BE>(self, l)
    }

    fn write_java_float(&mut self, f: f32) -> io::Result<()> {
        WriteBytesExt::write_f32::<BE>(self, f)
    }

    fn write_java_double(&mut self, d: f64) -> io::Result<()> {
        WriteBytesExt::write_f64::<BE>(self, d)
    }

    fn write_java_boolean(&mut self, b: bool) -> io::Result<()> {
        self.write_java_byte(b as i8)
    }

    fn write_java_char(&mut self, c: char) -> io::Result<()> {
        // FIXME: Write real UTF-16 char.
        Ok(WriteBytesExt::write_u16::<BE>(self, c as u16)?)
    }

    fn write_java_string16(&mut self, s: &str) -> io::Result<()> {
        
        if s.len() > i16::MAX as usize {
            return Err(new_invalid_data_err("string too big"));
        }
        
        self.write_java_short(s.len() as i16)?;
        for c in s.chars() {
            self.write_java_char(c)?;
        }

        Ok(())

    }

    fn write_java_string8(&mut self, s: &str) -> io::Result<()> {

        if s.len() > i16::MAX as usize {
            return Err(new_invalid_data_err("string too big"));
        }

        self.write_java_short(s.len() as i16)?;
        self.write_all(s.as_bytes())
        
    }

}

impl<R: Read> ReadPacketExt for R {}
impl<W: Write> WritePacketExt for W {}


/// Return an invalid data io error with specific message.
fn new_invalid_data_err(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
