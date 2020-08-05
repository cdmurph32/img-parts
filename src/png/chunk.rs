use std::convert::TryInto;
use std::fmt;
use std::io;

use bytes::{Buf, BufMut, Bytes, BytesMut};
use crc32fast::Hasher;

use crate::encoder::{EncodeAt, ImageEncoder};
use crate::{Error, Result};

/// The representation of a chunk making up a [`Png`][super::Png]
#[derive(Clone, PartialEq)]
pub struct PngChunk {
    kind: [u8; 4],
    contents: Bytes,
    crc: [u8; 4],
}

#[allow(clippy::len_without_is_empty)]
impl PngChunk {
    /// Construct an new `PngChunk`
    pub fn new(kind: [u8; 4], contents: Bytes) -> PngChunk {
        let crc = compute_crc(kind, &contents);
        Self::new_with_crc(kind, contents, crc)
    }

    /// Construct an new `PngChunk` with a known `crc`
    #[inline]
    fn new_with_crc(kind: [u8; 4], contents: Bytes, crc: [u8; 4]) -> PngChunk {
        PngChunk {
            kind,
            contents,
            crc,
        }
    }

    /// Create a `PngChunk` from `Bytes`
    ///
    /// # Errors
    ///
    /// This method fails if the chunk is corrupted or truncated.
    pub fn from_bytes(b: &mut Bytes) -> Result<PngChunk> {
        if b.len() < 8 {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "end of png").into());
        }

        let size = b.get_u32();

        let kind = b.split_to(4);
        let contents = b.split_to(size as usize);
        let crc = b.split_to(4);

        let kind = kind.as_ref().try_into().unwrap();
        let crc = crc.as_ref().try_into().unwrap();

        if crc != compute_crc(kind, &contents) {
            return Err(Error::BadCRC);
        }

        Ok(PngChunk::new_with_crc(kind, contents, crc))
    }

    /// Get the size of this `PngChunk` once it is encoded
    ///
    /// The size is the sum of:
    ///
    /// - The length (4 bytes).
    /// - The type (4 bytes)
    /// - The size of the content
    /// - The crc (4 bytes)
    pub fn len(&self) -> usize {
        // 4 bytes (length) + 4 bytes (type) + length of the content + crc (4 bytes)
        4 + 4 + self.contents.len() + 4
    }

    /// Get the type of this `PngChunk`
    #[inline]
    pub fn kind(&self) -> [u8; 4] {
        self.kind
    }

    /// Get the content of this `PngChunk`
    #[inline]
    pub fn contents(&self) -> &Bytes {
        &self.contents
    }

    /// Create an [encoder][crate::ImageEncoder] for this `PngChunk`
    #[inline]
    pub fn encoder(self) -> ImageEncoder<Self> {
        ImageEncoder::from(self)
    }
}

impl EncodeAt for PngChunk {
    fn encode_at(&self, pos: &mut usize) -> Option<Bytes> {
        match pos {
            0 => {
                let mut bytes = BytesMut::new();
                bytes.put_u32(self.contents.len().try_into().unwrap());
                Some(bytes.freeze())
            }
            1 => Some(Bytes::copy_from_slice(&self.kind)),
            2 => Some(self.contents.clone()),
            3 => Some(Bytes::copy_from_slice(&self.crc)),
            _ => {
                *pos -= 4;
                None
            }
        }
    }
}

impl fmt::Debug for PngChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PngChunk")
            .field("kind", &self.kind)
            .finish()
    }
}

/// Compute the `crc` for a `PngChunk`
fn compute_crc(kind: [u8; 4], contents: &[u8]) -> [u8; 4] {
    let mut hasher = Hasher::new();
    hasher.update(&kind);
    hasher.update(contents);
    let crc = hasher.finalize();

    crc.to_be_bytes()
}