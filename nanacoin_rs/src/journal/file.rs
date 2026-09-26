use super::*;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

/// The original journal remains the lock anchor and generation-zero log.
/// Companion files belong to this journal; back them up together while stopped.
pub struct FileJournal {
    logs: [File; 2],
    snapshots: [File; 2],
    archive: File,
    head_path: PathBuf,
    transport_path: PathBuf,
    https_only: bool,
    generation: u64,
    rows: usize,
    records: usize,
}
fn io<T>(result: std::io::Result<T>) -> Result<T, Error> {
    result.map_err(|_| Error::Storage)
}
fn open_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}
fn companion(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}
impl FileJournal {
    pub fn open(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let original_existed = path.exists();
        let original = open_file(path)?;
        fs2::FileExt::try_lock_exclusive(&original)?;
        let head_path = companion(path, ".head");
        let transport_path = companion(path, ".transport");
        let https_only = match std::fs::read(&transport_path) {
            Ok(v) if v == [0] => false,
            Ok(v) if v == [1] => true,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
            _ => return Err(std::io::Error::other("corrupt transport policy")),
        };
        let (generation, rows) = match std::fs::read(&head_path) {
            Ok(bytes) => checkpoint::parse_head(&bytes)
                .map_err(|_| std::io::Error::other("corrupt checkpoint head"))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (0, 0),
            Err(e) => return Err(e),
        };
        // Never silently create missing files named by a committed manifest.
        if generation > 0 {
            let bank = generation % 2;
            let log = if bank == 0 {
                path.to_path_buf()
            } else {
                companion(path, ".bank1")
            };
            if (bank == 0 && !original_existed)
                || !log.exists()
                || !companion(path, &format!(".checkpoint{bank}")).exists()
            {
                return Err(std::io::Error::other("missing committed checkpoint files"));
            }
        }
        let logs = [original, open_file(&companion(path, ".bank1"))?];
        let snapshots = [
            open_file(&companion(path, ".checkpoint0"))?,
            open_file(&companion(path, ".checkpoint1"))?,
        ];
        let archive = open_file(&companion(path, ".archive"))?;
        if archive.metadata()?.len() > (archive::SLOTS * archive::PAGE_BYTES) as u64 {
            return Err(std::io::Error::other("archive exceeds capacity"));
        }
        let active = (generation % 2) as usize;
        let len = logs[active].metadata()?.len();
        if len > (MAX_RECORDS * FRAME_SIZE) as u64 {
            return Err(std::io::Error::other("journal exceeds capacity"));
        }
        let valid = len / FRAME_SIZE as u64 * FRAME_SIZE as u64;
        if valid != len {
            logs[active].set_len(valid)?;
            logs[active].sync_all()?;
        }
        Ok(Self {
            logs,
            snapshots,
            archive,
            head_path,
            transport_path,
            https_only,
            generation,
            rows,
            records: (valid / FRAME_SIZE as u64) as usize,
        })
    }
    fn bank(&self) -> usize {
        (self.generation % 2) as usize
    }
}
impl Journal for FileJournal {
    fn supports_archive(&self) -> bool {
        true
    }
    fn read_archive(
        &mut self,
        slot: usize,
        out: &mut [u8; archive::PAGE_BYTES],
    ) -> Result<usize, Error> {
        if slot >= archive::SLOTS {
            return Err(Error::CorruptJournal);
        }
        io(self
            .archive
            .seek(SeekFrom::Start((slot * archive::PAGE_BYTES) as u64)))?;
        io(self.archive.read_exact(out))?;
        let used = u32::from_le_bytes(out[20..24].try_into().unwrap()) as usize;
        if !(32..=archive::PAGE_BYTES).contains(&used) {
            return Err(Error::CorruptJournal);
        }
        Ok(used)
    }
    fn write_archive(&mut self, slot: usize, bytes: &[u8]) -> Result<(), Error> {
        if slot >= archive::SLOTS || !(32..=archive::PAGE_BYTES).contains(&bytes.len()) {
            return Err(Error::Capacity);
        }
        let mut page = [0; archive::PAGE_BYTES];
        page[..bytes.len()].copy_from_slice(bytes);
        io(self
            .archive
            .seek(SeekFrom::Start((slot * archive::PAGE_BYTES) as u64)))?;
        io(self.archive.write_all(&page))?;
        io(self.archive.sync_all())?;
        let mut verify = [0; archive::PAGE_BYTES];
        io(self
            .archive
            .seek(SeekFrom::Start((slot * archive::PAGE_BYTES) as u64)))?;
        io(self.archive.read_exact(&mut verify))?;
        if verify != page {
            return Err(Error::Storage);
        }
        Ok(())
    }
    fn https_only(&self) -> bool {
        self.https_only
    }
    fn supports_transport(&self) -> bool {
        true
    }
    fn set_https_only(&mut self, value: bool) -> Result<(), Error> {
        let temporary = companion(&self.transport_path, ".next");
        let mut file = io(File::create(&temporary))?;
        io(file.write_all(&[u8::from(value)]))?;
        io(file.sync_all())?;
        drop(file);
        io(publish(&temporary, &self.transport_path))?;
        self.https_only = value;
        Ok(())
    }
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        if index >= self.records {
            return Ok(false);
        }
        let bank = self.bank();
        let log = &mut self.logs[bank];
        io(log.seek(SeekFrom::Start((index * FRAME_SIZE) as u64)))?;
        io(log.read_exact(frame))?;
        Ok(true)
    }
    fn append(&mut self, index: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        if index != self.records || index >= MAX_RECORDS {
            return Err(Error::Storage);
        }
        let bank = self.bank();
        let log = &mut self.logs[bank];
        io(log.seek(SeekFrom::Start((index * FRAME_SIZE) as u64)))?;
        io(log.write_all(frame))?;
        io(log.sync_all())?;
        self.records += 1;
        Ok(())
    }
    fn supports_checkpoint(&self) -> bool {
        true
    }
    fn generation(&self) -> u64 {
        self.generation
    }
    fn checkpoint_rows(&self) -> usize {
        self.rows
    }
    fn read_checkpoint(
        &mut self,
        index: usize,
        out: &mut [u8; checkpoint::ROW_BYTES],
    ) -> Result<usize, Error> {
        if index >= self.rows {
            return Err(Error::CorruptJournal);
        }
        let bank = self.bank();
        let file = &mut self.snapshots[bank];
        io(file.seek(SeekFrom::Start((index * checkpoint::ROW_BYTES) as u64)))?;
        io(file.read_exact(out))?;
        let len = u32::from_le_bytes(out[8..12].try_into().unwrap()) as usize;
        if len > checkpoint::ROW_BYTES - 16 {
            return Err(Error::CorruptJournal);
        }
        Ok(16 + len)
    }
    fn begin_checkpoint(&mut self) -> Result<(), Error> {
        if self.generation >= crate::domain::MAX_SEQUENCE {
            return Err(Error::Overflow);
        }
        let next = 1 - self.bank();
        io(self.logs[next].set_len(0))?;
        io(self.logs[next].sync_all())?;
        io(self.snapshots[next].set_len(0))?;
        Ok(())
    }
    fn write_checkpoint(&mut self, index: usize, bytes: &[u8]) -> Result<(), Error> {
        if index >= checkpoint::MAX_ROWS || bytes.len() > checkpoint::ROW_BYTES {
            return Err(Error::Capacity);
        }
        let next = 1 - self.bank();
        let file = &mut self.snapshots[next];
        let offset = (index * checkpoint::ROW_BYTES) as u64;
        let mut row = [0; checkpoint::ROW_BYTES];
        row[..bytes.len()].copy_from_slice(bytes);
        io(file.seek(SeekFrom::Start(offset)))?;
        io(file.write_all(&row))?;
        let mut verify = [0; checkpoint::ROW_BYTES];
        io(file.seek(SeekFrom::Start(offset)))?;
        io(file.read_exact(&mut verify))?;
        if row != verify {
            return Err(Error::Storage);
        }
        Ok(())
    }
    fn commit_checkpoint(&mut self, rows: usize) -> Result<(), Error> {
        let old = self.bank();
        let next = 1 - old;
        io(self.snapshots[next].sync_all())?;
        let generation = self.generation + 1;
        let temporary = companion(&self.head_path, ".next");
        let mut head = open_file(&temporary).map_err(|_| Error::Storage)?;
        io(head.set_len(0))?;
        io(head.write_all(&checkpoint::head(generation, rows)))?;
        io(head.sync_all())?;
        drop(head);
        io(publish(&temporary, &self.head_path))?;
        self.generation = generation;
        self.rows = rows;
        self.records = 0;
        // Publication is durable before any old bytes are reclaimed.
        io(self.logs[old].set_len(0))?;
        io(self.logs[old].sync_all())?;
        io(self.snapshots[old].set_len(0))?;
        io(self.snapshots[old].sync_all())?;
        Ok(())
    }
}

#[cfg(not(windows))]
fn publish(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)?;
    File::open(
        to.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )?
    .sync_all()
}
#[cfg(windows)]
fn publish(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    }
    let from: Vec<u16> = from.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = to.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both paths are NUL-terminated UTF-16, live for the call. Replace
    // atomically on the same filesystem, with MOVEFILE_WRITE_THROUGH.
    if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0x1 | 0x8) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
