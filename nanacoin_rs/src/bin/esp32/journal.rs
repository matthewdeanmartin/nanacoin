use esp_idf_svc::{
    handle::RawHandle,
    nvs::{EspNvs, EspNvsPartition, NvsCustom},
    sys,
};
use nanacoin::{
    domain::Error,
    journal::{
        checkpoint::{self, ROW_BYTES},
        Journal, FRAME_SIZE,
    },
};

pub struct NvsJournal {
    banks: [EspNvs<NvsCustom>; 2],
    archive: EspNvs<NvsCustom>,
    metadata: EspNvs<NvsCustom>,
    generation: u64,
    rows: usize,
    https_only: bool,
}
impl NvsJournal {
    pub fn open(partition: EspNvsPartition<NvsCustom>) -> Result<Self, Error> {
        let metadata =
            EspNvs::new(partition.clone(), "ncmeta", true).map_err(|_| Error::Storage)?;
        let mut head = [0; 32];
        let mut policy = [0; 1];
        let https_only = match metadata
            .get_blob("https_only", &mut policy)
            .map_err(|_| Error::Storage)?
        {
            None | Some([0]) => false,
            Some([1]) => true,
            _ => return Err(Error::CorruptJournal),
        };
        let (generation, rows) = match metadata
            .get_blob("head", &mut head)
            .map_err(|_| Error::Storage)?
        {
            Some(bytes) => checkpoint::parse_head(bytes)?,
            None => (0, 0),
        };
        // Bank zero keeps the legacy namespace; no partition-table change.
        let archive = EspNvs::new(partition.clone(), "ncarch", true).map_err(|_| Error::Storage)?;
        let banks = [
            EspNvs::new(partition.clone(), "nanacoin", true).map_err(|_| Error::Storage)?,
            EspNvs::new(partition, "ncnext", true).map_err(|_| Error::Storage)?,
        ];
        Ok(Self {
            banks,
            archive,
            metadata,
            generation,
            rows,
            https_only,
        })
    }
    fn bank(&self) -> usize {
        (self.generation % 2) as usize
    }
}
fn key(prefix: char, index: usize) -> Result<heapless::String<8>, Error> {
    let mut key = heapless::String::new();
    core::fmt::Write::write_fmt(&mut key, format_args!("{prefix}{index:04x}"))
        .map_err(|_| Error::Capacity)?;
    Ok(key)
}
impl Journal for NvsJournal {
    fn supports_archive(&self) -> bool {
        true
    }
    fn read_archive(
        &mut self,
        slot: usize,
        out: &mut [u8; nanacoin::journal::archive::PAGE_BYTES],
    ) -> Result<usize, Error> {
        if slot >= nanacoin::journal::archive::SLOTS {
            return Err(Error::CorruptJournal);
        }
        out.fill(0);
        self.archive
            .get_blob(&key('p', slot)?, out)
            .map_err(|_| Error::Storage)?
            .map(|b| b.len())
            .ok_or(Error::CorruptJournal)
    }
    fn write_archive(&mut self, slot: usize, bytes: &[u8]) -> Result<(), Error> {
        if slot >= nanacoin::journal::archive::SLOTS
            || !(32..=nanacoin::journal::archive::PAGE_BYTES).contains(&bytes.len())
        {
            return Err(Error::Capacity);
        }
        let key = key('p', slot)?;
        self.archive
            .set_blob(&key, bytes)
            .map_err(|_| Error::Storage)?;
        let mut verify = [0; nanacoin::journal::archive::PAGE_BYTES];
        if self
            .archive
            .get_blob(&key, &mut verify)
            .map_err(|_| Error::Storage)?
            != Some(bytes)
        {
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
        self.metadata
            .set_blob("https_only", &[u8::from(value)])
            .map_err(|_| Error::Storage)?;
        self.https_only = value;
        Ok(())
    }
    fn read(&mut self, index: usize, frame: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        frame.fill(0);
        match self.banks[self.bank()]
            .get_blob(&key('e', index)?, frame)
            .map_err(|_| Error::Storage)?
        {
            Some(data) => {
                if !(13..=FRAME_SIZE).contains(&data.len()) {
                    return Err(Error::CorruptJournal);
                }
                let len = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
                if len == 0 || len > FRAME_SIZE - 12 || data.len() != 12 + len {
                    return Err(Error::CorruptJournal);
                }
                Ok(true)
            }
            None => Ok(false),
        }
    }
    fn append(&mut self, index: usize, frame: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
        if &frame[..4] != b"NCR2" || len == 0 || len > FRAME_SIZE - 12 {
            return Err(Error::CorruptJournal);
        }
        let used = 12 + len;
        let key = key('e', index)?;
        let bank = &self.banks[self.bank()];
        if bank.blob_len(&key).map_err(|_| Error::Storage)?.is_some() {
            return Err(Error::Storage);
        }
        bank.set_blob(&key, &frame[..used])
            .map_err(|_| Error::Storage)
    }
    fn generation(&self) -> u64 {
        self.generation
    }
    fn checkpoint_rows(&self) -> usize {
        self.rows
    }
    fn supports_checkpoint(&self) -> bool {
        true
    }
    fn read_checkpoint(&mut self, index: usize, out: &mut [u8; ROW_BYTES]) -> Result<usize, Error> {
        self.banks[self.bank()]
            .get_blob(&key('c', index)?, out)
            .map_err(|_| Error::Storage)?
            .map(|v| v.len())
            .ok_or(Error::CorruptJournal)
    }
    fn begin_checkpoint(&mut self) -> Result<(), Error> {
        if self.generation >= nanacoin::domain::MAX_SEQUENCE {
            return Err(Error::Overflow);
        }
        self.banks[1 - self.bank()]
            .erase_all()
            .map_err(|_| Error::Storage)
    }
    fn write_checkpoint(&mut self, index: usize, bytes: &[u8]) -> Result<(), Error> {
        if index % 32 == 0 {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        if index >= checkpoint::MAX_ROWS || bytes.len() > ROW_BYTES {
            return Err(Error::Capacity);
        }
        let bank = &self.banks[1 - self.bank()];
        let key = key('c', index)?;
        let mut ckey = heapless::Vec::<u8, 9>::new();
        ckey.extend_from_slice(key.as_bytes())
            .map_err(|_| Error::Capacity)?;
        ckey.push(0).map_err(|_| Error::Capacity)?;
        // SAFETY: owned handle, bounded NUL-terminated key and live bytes.
        // Commit the entire checkpoint before publication. NVS can program
        // flash before commit; this is not a RAM-only transaction.
        let result = unsafe {
            sys::nvs_set_blob(
                bank.handle(),
                ckey.as_ptr().cast(),
                bytes.as_ptr().cast(),
                bytes.len(),
            )
        };
        if result != 0 {
            return Err(Error::Storage);
        }
        let mut verify = [0; ROW_BYTES];
        if bank
            .get_blob(&key, &mut verify)
            .map_err(|_| Error::Storage)?
            != Some(bytes)
        {
            return Err(Error::Storage);
        }
        Ok(())
    }
    fn commit_checkpoint(&mut self, rows: usize) -> Result<(), Error> {
        let old = self.bank();
        let next = 1 - old;
        // SAFETY: live NVS handle, exclusively mutated under service mutex.
        if unsafe { sys::nvs_commit(self.banks[next].handle()) } != 0 {
            return Err(Error::Storage);
        }
        let generation = self.generation + 1;
        self.metadata
            .set_blob("head", &checkpoint::head(generation, rows))
            .map_err(|_| Error::Storage)?;
        self.generation = generation;
        self.rows = rows;
        // Only now can the old journal and checkpoint be reclaimed.
        self.banks[old].erase_all().map_err(|_| Error::Storage)
    }
}
