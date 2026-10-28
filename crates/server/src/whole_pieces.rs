//! Torrent data reaches the disk a whole piece at a time.
//!
//! librqbit writes each 16 KiB block as it arrives. On a network share (a
//! Storage Box over SMB), a write that doesn't start and end on a 4 KiB page
//! makes Linux fetch the rest of the page from the server first, with the
//! file locked. In a torrent of many files, which start anywhere in a page,
//! nearly every block waits for that round trip: a download that started at
//! 110 MB/s fell to 1 MB/s. Holding a piece's blocks in memory until the
//! piece is verified, then writing each file's part of it at once, leaves
//! those round trips at the two ends of a piece instead of at every block,
//! and the check that verifies a piece reads it from memory.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Mutex;

use anyhow::Context;
use librqbit::storage::{BoxStorageFactory, StorageFactory, StorageFactoryExt, TorrentStorage};
use librqbit::{FileInfos, ManagedTorrentShared, TorrentMetadata};
use librqbit_core::lengths::{Lengths, ValidPieceIndex};

/// Pieces held at once, across a torrent, before new ones are written block
/// by block as before.
const MAX_HELD_BYTES: u64 = 512 << 20;

/// Wraps `underlying`, librqbit's own storage, so it is written a piece at a time.
#[derive(Clone)]
pub struct WholePieces<U> {
    pub underlying: U,
}

impl<U: StorageFactory + Clone> StorageFactory for WholePieces<U> {
    type Storage = WholePiecesStorage<U::Storage>;

    fn create(
        &self,
        shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> anyhow::Result<Self::Storage> {
        Ok(WholePiecesStorage {
            lengths: *metadata.info.lengths(),
            files: metadata.file_infos.clone(),
            held: Mutex::default(),
            underlying: self.underlying.create(shared, metadata)?,
        })
    }

    fn clone_box(&self) -> BoxStorageFactory {
        self.clone().boxed()
    }
}

pub struct WholePiecesStorage<U> {
    lengths: Lengths,
    files: FileInfos,
    held: Mutex<Held>,
    underlying: U,
}

#[derive(Default)]
struct Held {
    pieces: HashMap<ValidPieceIndex, Box<[u8]>>,
    bytes: u64,
    /// Pieces whose first block came when memory was full: written through
    /// until they complete, so none is part held and part on disk.
    through: HashSet<ValidPieceIndex>,
}

impl<U: TorrentStorage> WholePiecesStorage<U> {
    /// The piece that byte `offset` of file `file_id` is in, and where in the
    /// piece it is. (librqbit's `compute_current_piece` gives the piece's
    /// number for the second.)
    fn locate(&self, file_id: usize, offset: u64) -> anyhow::Result<(ValidPieceIndex, usize)> {
        let file = self.files.get(file_id).context("no such file")?;
        let at = file.offset_in_torrent + offset;
        let piece_length = u64::from(self.lengths.default_piece_length());
        let piece = u32::try_from(at / piece_length)
            .ok()
            .and_then(|p| self.lengths.validate_piece_index(p))
            .context("past the end of the torrent")?;
        Ok((piece, (at % piece_length) as usize))
    }

    /// Writes a completed piece's bytes to every file it covers.
    fn write_out(&self, piece: ValidPieceIndex, data: &[u8]) -> anyhow::Result<()> {
        let start = self.lengths.piece_offset(piece);
        let end = start + data.len() as u64;
        for (file_id, file) in self.files.iter().enumerate() {
            let file_start = file.offset_in_torrent;
            let file_end = file_start + file.len;
            if file.attrs.padding || file_end <= start || file_start >= end {
                continue;
            }
            let (from, to) = (start.max(file_start), end.min(file_end));
            let part = &data[(from - start) as usize..(to - start) as usize];
            self.underlying
                .pwrite_all(file_id, from - file_start, part)
                .with_context(|| {
                    format!("writing piece {piece} to {:?}", file.relative_filename)
                })?;
        }
        Ok(())
    }
}

impl<U: TorrentStorage> TorrentStorage for WholePiecesStorage<U> {
    fn init(
        &mut self,
        shared: &ManagedTorrentShared,
        metadata: &TorrentMetadata,
    ) -> anyhow::Result<()> {
        self.underlying.init(shared, metadata)
    }

    fn pread_exact(&self, file_id: usize, offset: u64, buf: &mut [u8]) -> anyhow::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }
        let (piece, at) = self.locate(file_id, offset)?;
        if let Some(bytes) = self
            .held
            .lock()
            .expect("held lock")
            .pieces
            .get(&piece)
            .and_then(|p| p.get(at..at + buf.len()))
        {
            buf.copy_from_slice(bytes);
            return Ok(());
        }
        self.underlying.pread_exact(file_id, offset, buf)
    }

    fn pwrite_all(&self, file_id: usize, offset: u64, buf: &[u8]) -> anyhow::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }
        let (piece, at) = self.locate(file_id, offset)?;
        {
            let mut held = self.held.lock().expect("held lock");
            let held = &mut *held;
            if !held.through.contains(&piece) {
                let len = u64::from(self.lengths.piece_length(piece));
                if !held.pieces.contains_key(&piece) && held.bytes + len > MAX_HELD_BYTES {
                    held.through.insert(piece);
                } else {
                    let bytes = held.pieces.entry(piece).or_insert_with(|| {
                        held.bytes += len;
                        vec![0; len as usize].into_boxed_slice()
                    });
                    bytes
                        .get_mut(at..at + buf.len())
                        .context("a block past the end of its piece")?
                        .copy_from_slice(buf);
                    return Ok(());
                }
            }
        }
        self.underlying.pwrite_all(file_id, offset, buf)
    }

    fn on_piece_completed(&self, piece: ValidPieceIndex) -> anyhow::Result<()> {
        let data = {
            let mut held = self.held.lock().expect("held lock");
            held.through.remove(&piece);
            let data = held.pieces.remove(&piece);
            held.bytes -= data.as_ref().map_or(0, |d| d.len() as u64);
            data
        };
        if let Some(data) = data {
            // librqbit counts the piece as had whatever this returns, and
            // only logs an error quietly.
            if let Err(e) = self.write_out(piece, &data) {
                tracing::warn!("a downloaded piece didn't reach the disk: {e:#}");
                return Err(e);
            }
        }
        self.underlying.on_piece_completed(piece)
    }

    fn remove_file(&self, file_id: usize, filename: &Path) -> anyhow::Result<()> {
        self.underlying.remove_file(file_id, filename)
    }

    fn remove_directory_if_empty(&self, path: &Path) -> anyhow::Result<()> {
        self.underlying.remove_directory_if_empty(path)
    }

    fn ensure_file_length(&self, file_id: usize, length: u64) -> anyhow::Result<()> {
        self.underlying.ensure_file_length(file_id, length)
    }

    /// On pause: pieces part arrived go along to the storage that takes over.
    fn take(&self) -> anyhow::Result<Box<dyn TorrentStorage>> {
        let held = std::mem::take(&mut *self.held.lock().expect("held lock"));
        Ok(Box::new(WholePiecesStorage {
            lengths: self.lengths,
            files: self.files.clone(),
            held: Mutex::new(held),
            underlying: self.underlying.take()?,
        }))
    }
}
