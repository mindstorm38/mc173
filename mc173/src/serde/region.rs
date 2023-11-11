//! Minecraft region file format storing 32x32 chunks inside a single file.

use std::io::{self, Seek, SeekFrom, Write, Read};
use std::path::Path;
use std::fs::File;
use std::io::Take;

use byteorder::{ReadBytesExt, WriteBytesExt};
use thiserror::Error;

use flate2::read::{GzDecoder, ZlibDecoder};
use flate2::write::ZlibEncoder;
use flate2::Compression;


use crate::util::{ReadJavaExt, WriteJavaExt};


/// Internal function to calculate the index of a chunk metadata depending on its 
/// position, this is the same calculation as Notchian server.
#[inline]
fn calc_chunk_index(cx: i32, cz: i32) -> usize {
    (cx & 31) as usize | (((cz & 31) as usize) << 5)
}

const EMPTY_SECTOR: &'static [u8; 4096] = &[0; 4096];

/// A handle to a region file. This is an implementation of ".mcr" region files following
/// the same algorithms as the Notchian server, first developped by Scaevolus (legend!).
pub struct RegionFile {
    /// Underlying region file. 
    file: File,
    /// Stores the metadata of each chunks
    chunks: Box<[Chunk; 1024]>,
    /// Bit mapping of sectors that are allocated.
    sectors: Vec<u64>,
}

impl RegionFile {

    /// Open a region file, this constructor report every possible error with the region
    /// file without altering it in such case, it's up to the caller to delete the file
    /// and retry is wanted.
    pub fn open<P: AsRef<Path>>(path: P, create: bool) -> Result<Self, RegionError> {

        let mut file = File::options()
            .read(true)
            .write(true)
            .create(create)
            .open(path)?;

        // Start by querying the file length.
        let file_len = file.seek(SeekFrom::End(0))?;

        // A region file should have a length of at least 8K in order to store each chunk
        // metadata, we fix the file if this is not already the case.
        if file_len == 0 && create {
            // We write all first 8K to zero to initialize the file, only if it was meant
            // to be created, if not then we fall through and return too small error.
            for _ in 0..2 {
                file.write_all(EMPTY_SECTOR)?;
            }
        } else if file_len < 8192 {
            return Err(RegionError::FileTooSmall(file_len));
        } else if file_len % 4096 != 0 {
            return Err(RegionError::FileNotPadded(file_len));
        }

        // The first two sectors are always reserved for the chunk metadata.
        let mut chunks: Box<[Chunk; 1024]> = Box::new([Chunk::INIT; 1024]);
        let mut sectors = vec![0u64; file_len as usize / 4096];
        // First two sectors are reserved for headers.
        sectors[0] |= 0b11;

        file.seek(SeekFrom::Start(0))?;

        // Start by reading each offset, 4 bytes each for 1024 chunks, so 4K.
        for i in 0..1024 {

            let range_raw = file.read_java_int()? as u32;
            let range = SectorRange {
                offset: range_raw >> 8,
                count: range_raw & 0xFF,
            };

            chunks[i].range = range;

            for offset in range.offset..range.offset + range.count {
                if let Some(slot) = sectors.get_mut(offset as usize / 64) {
                    *slot |= 1u64 << (offset % 64);
                } else {
                    return Err(RegionError::IllegalRange);
                }
            }

        }

        // Then we read the timestamps, same format as offsets.
        for i in 0..1024 {
            chunks[i].timestamp = file.read_java_int()? as u32;
        }

        Ok(Self {
            file,
            chunks,
            sectors,
        })

    }

    /// Internal function to get the chunk metadata associated with a chunk.
    fn get_chunk(&self, cx: i32, cz: i32) -> Chunk {
        self.chunks[calc_chunk_index(cx, cz)]
    }

    fn set_chunk_and_sync(&mut self, cx: i32, cz: i32, chunk: Chunk) -> io::Result<()> {
        let index = calc_chunk_index(cx, cz);
        self.chunks[index] = chunk;
        // Synchronize range.
        let range_raw = chunk.range.offset << 8 | chunk.range.count & 0xFF;
        self.file.seek(SeekFrom::Start(index as u64 * 4))?;
        self.file.write_java_int(range_raw as i32)?;
        // Synchronize timestamp.
        self.file.seek(SeekFrom::Current(4096))?;
        self.file.write_java_int(chunk.timestamp as i32)?;
        Ok(())
    }

    /// Read the chunk at the given position, the chunk position is at modulo 32 in order
    /// to respect the limitations of the region size, caller don't have to do it.
    pub fn read_chunk(&mut self, cx: i32, cz: i32) -> Result<ChunkReader<'_>, RegionError> {

        let chunk = self.get_chunk(cx, cz);
        if chunk.is_empty() {
            return Err(RegionError::EmptyChunk);
        }

        if chunk.range.offset < 2 {
            return Err(RegionError::IllegalRange);
        }

        // Seek to the start of the chunk where the header is present.
        self.file.seek(SeekFrom::Start(chunk.range.offset as u64 * 4096))?;

        let chunk_size = self.file.read_java_int()?;
        if chunk_size < 0 || chunk_size as u32 + 4 > chunk.range.count * 4096 {
            return Err(RegionError::IllegalRange);
        }

        let compression_id = self.file.read_u8()?;
        let chunk_size = chunk_size as u64 - 1; // Subtract one for compression id.
        let chunk_data = Read::take(&mut self.file, chunk_size);

        let inner = match compression_id {
            1 => ChunkReaderInner::Gz(GzDecoder::new(chunk_data)),
            2 => ChunkReaderInner::Zlib(ZlibDecoder::new(chunk_data)),
            _ => return Err(RegionError::IllegalCompression),
        };

        Ok(ChunkReader { inner })

    }

    /// Write a chunk at the given position, the chunk position is at modulo 32 in order
    /// to respect the limitations of the region size, caller don't have to do it.
    pub fn write_chunk(&mut self, cx: i32, cz: i32) -> ChunkWriter<'_> {
        ChunkWriter { 
            cx, 
            cz, 
            encoder: ZlibEncoder::new(Vec::new(), Compression::best()), 
            region: self,
        }
    }

    fn write_chunk_data(&mut self, cx: i32, cz: i32, compression_id: u8, data: &[u8]) -> Result<(), RegionError> {
 
        // NOTE: This will always require at least 1 sector because of headers.
        let sector_count = (data.len() + 5 - 1) as u32 / 4096 + 1;
        if sector_count > 0xFF {
            return Err(RegionError::OutOfSector);
        }

        let mut chunk = self.get_chunk(cx, cz);

        // If the current chunk count doesn't match, we try to extend the current one or
        // allocate a new available range.
        if sector_count != chunk.range.count {

            let mut clear_range = chunk.range;

            // We just need to shrink sectors.
            if sector_count < chunk.range.count {
                clear_range.offset += sector_count;
                clear_range.count -= sector_count;
                chunk.range.count = sector_count;
            }

            // Clear the previous range.
            self.file.seek(SeekFrom::Start(clear_range.offset as u64 * 4096))?;
            for offset in clear_range.offset..clear_range.offset + clear_range.count {
                let slot = &mut self.sectors[offset as usize / 64];
                *slot &= !(1u64 << (offset % 64));
                self.file.write_all(EMPTY_SECTOR)?;
            }

            // If we did not shrink, we have deallocated everything so we need to alloc.
            if sector_count > chunk.range.count {

                let mut new_range = SectorRange::default();

                'out: for (slot_index, mut slot) in self.sectors.iter().copied().enumerate() {
                    // Avoid check a slot that is fully allocated.
                    if slot != u64::MAX {
                        // Check for each slot bit for a sequence of free sectors.
                        for bit_index in 0usize..64 {
                            if slot & 1 == 0 {
                                new_range.count += 1;
                                if new_range.count == sector_count {
                                    break 'out;
                                }
                            } else {
                                new_range.offset = slot_index as u32 * 64 + bit_index as u32 + 1;
                                new_range.count = 0;
                            }
                            slot >>= 1;
                        }
                    }
                }

                // NOTE: We are overwriting the count because if we did not find enough
                // free space we can still add it at the end.
                new_range.count = sector_count;
                for offset in new_range.offset..new_range.offset + new_range.count {
                    let slot_index = offset as usize / 64;
                    if let Some(slot) = self.sectors.get_mut(slot_index) {
                        *slot |= 1u64 << (offset % 64);
                    } else {
                        debug_assert_eq!(slot_index, self.sectors.len());
                        self.sectors.push(1);
                    }
                }

                chunk.range = new_range;

            }

        }

        self.set_chunk_and_sync(cx, cz, chunk)?;

        self.file.seek(SeekFrom::Start(chunk.range.offset as u64 * 4096))?;
        self.file.write_java_int(data.len() as i32 + 1)?; // Adding the compression id.
        self.file.write_u8(compression_id)?;
        self.file.write_all(data)?;

        Ok(())

    }

}


/// A handle for reading a chunk from a region file.
pub struct ChunkReader<'region> {
    /// Inner implementation depending on compression.
    inner: ChunkReaderInner<'region>,
}

/// The actual implementation of the chunk reader depending on the compression type.
enum ChunkReaderInner<'region> {
    Gz(GzDecoder<Take<&'region mut File>>),
    Zlib(ZlibDecoder<Take<&'region mut File>>),
}

impl Read for ChunkReader<'_> {

    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match &mut self.inner {
            ChunkReaderInner::Gz(gz) => gz.read(buf),
            ChunkReaderInner::Zlib(zlib) => zlib.read(buf),
        }
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match &mut self.inner {
            ChunkReaderInner::Gz(gz) => gz.read_exact(buf),
            ChunkReaderInner::Zlib(zlib) => zlib.read_exact(buf),
        }
    }

}


/// A handle for writing a chunk in a region file.
pub struct ChunkWriter<'region> {
    /// The chunk X coordinate.
    cx: i32,
    /// The chunk Z coordinate.
    cz: i32,
    /// The internal zlib encoder, we force using zlib when writing (id 2).
    encoder: ZlibEncoder<Vec<u8>>,
    /// The underlying region file used to finally write chunk data.
    region: &'region mut RegionFile,
}

impl ChunkWriter<'_> {

    /// A more costly variant of the regular IO's flush function, because this one also
    /// flush the inner encoded buffer to the region file, therefore searching available
    /// sectors and writing data.
    pub fn flush_chunk(&mut self) -> Result<(), RegionError> {
        self.flush()?;
        self.region.write_chunk_data(self.cx, self.cz, 2, &self.encoder.get_ref())
    }

}

impl Write for ChunkWriter<'_> {

    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.encoder.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.encoder.flush()
    }

}


/// Internal cached chunk metadata, it is kept in-sync with the region file.
#[derive(Debug, Clone, Copy)]
struct Chunk {
    /// The offset of the chunk in sectors within the region file. The least significant
    /// byte is used for counting the number of sectors used (can be zero), and the 
    /// remaining 3 bytes are storing the offset in sectors (should not be 0 or 1).
    range: SectorRange,
    /// Timestamp when the chunk was last saved in the region file.
    timestamp: u32,
}

impl Chunk {

    const INIT: Self = Self { range: SectorRange { offset: 0, count: 0 }, timestamp: 0 };

    fn is_empty(self) -> bool {
        self.range.is_empty()
    }

}

/// Indicate a free range of sector.
#[derive(Debug, Clone, Copy, Default)]
struct SectorRange {
    /// Offset of the first sector in that range.
    offset: u32,
    /// The number of sectors in the ranges, this should not be zero.
    count: u32,
}

impl SectorRange {

    fn is_empty(self) -> bool {
        self.count == 0
    }

}

/// Error type used together with `RegionResult` for every call on region file methods.
#[derive(Error, Debug)]
pub enum RegionError {
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("The region file size ({0}) is too short to store the two 4K header sectors.")]
    FileTooSmall(u64),
    #[error("The region file size ({0}) is not a multiple of 4K.")]
    FileNotPadded(u64),
    #[error("The region file has an invalid chunk range, likely out of range or colliding with another one.")]
    IllegalRange,
    #[error("The required chunk is empty, it has no sector allocated in the region file.")]
    EmptyChunk,
    #[error("The compression method in the chunk header is illegal.")]
    IllegalCompression,
    #[error("No more sectors are available in the region file, really unlikely to happen.")]
    OutOfSector,
}
