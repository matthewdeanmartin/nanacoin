use super::*;
use crate::journal::FRAME_SIZE;

#[derive(Default)]
struct Rows(Vec<Vec<u8>>);
impl Journal for Rows {
    fn read(&mut self, _: usize, _: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        Ok(false)
    }
    fn append(&mut self, _: usize, _: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        unreachable!()
    }
    fn checkpoint_rows(&self) -> usize {
        self.0.len()
    }
    fn begin_checkpoint(&mut self) -> Result<(), Error> {
        self.0.clear();
        Ok(())
    }
    fn write_checkpoint(&mut self, i: usize, bytes: &[u8]) -> Result<(), Error> {
        if i == self.0.len() {
            self.0.push(bytes.to_vec());
        } else {
            self.0[i] = bytes.to_vec();
        }
        Ok(())
    }
    fn commit_checkpoint(&mut self, _: usize) -> Result<(), Error> {
        Ok(())
    }
    fn read_checkpoint(&mut self, i: usize, out: &mut [u8; ROW_BYTES]) -> Result<usize, Error> {
        let row = self.0.get(i).ok_or(Error::CorruptJournal)?;
        out[..row.len()].copy_from_slice(row);
        Ok(row.len())
    }
}
fn empty() -> Rows {
    let mut j = Rows::default();
    save_empty(&mut j, archive::ArchiveHead::default()).unwrap();
    j
}
#[test]
fn crc_valid_hostile_counts_are_errors_never_arithmetic_panics() {
    for field in [
        "corrections",
        "epochs",
        "audits",
        "requests",
        "artworks",
        "fulfillments",
        "members",
        "listings",
        "history",
        "offers",
        "quotes",
        "things",
        "loans",
        "lottos",
        "keys",
    ] {
        for count in [usize::MAX, usize::MAX - 1, MAX_ROWS + 1] {
            let mut j = empty();
            let h: Header = read(&mut j, &mut 0, 0).unwrap();
            let mut value = serde_json::to_value(h).unwrap();
            value[field] = serde_json::json!(count);
            let h: Header = serde_json::from_value(value).unwrap();
            write(&mut j, &mut 0, 0, &h).unwrap();
            let mut state = Box::new(State::default());
            assert_eq!(
                restore(&mut j, &mut state, &mut VecDeque::new()),
                Err(Error::CorruptJournal),
                "{field}={count}"
            );
        }
    }
}
#[test]
fn checkpoint_header_truncations_and_byte_mutations_fail_closed() {
    let original = empty().0[0].clone();
    for end in 0..original.len() {
        let mut j = Rows(vec![original[..end].to_vec()]);
        assert!(
            restore(
                &mut j,
                &mut Box::new(State::default()),
                &mut VecDeque::new()
            )
            .is_err(),
            "truncation {end}"
        );
    }
    for index in 0..original.len() {
        let mut bytes = original.clone();
        bytes[index] ^= 1;
        let mut j = Rows(vec![bytes]);
        assert!(
            restore(
                &mut j,
                &mut Box::new(State::default()),
                &mut VecDeque::new()
            )
            .is_err(),
            "mutation {index}"
        );
    }
}
#[test]
fn publication_head_checks_every_byte_length_and_semantic_bounds() {
    let valid = head(1, 1);
    assert_eq!(parse_head(&valid), Ok((1, 1)));
    for end in 0..valid.len() {
        assert_eq!(parse_head(&valid[..end]), Err(Error::CorruptJournal));
    }
    for index in 0..valid.len() {
        let mut changed = valid;
        changed[index] ^= 1;
        assert_eq!(parse_head(&changed), Err(Error::CorruptJournal));
    }
    for generation in [0, MAX_SEQUENCE + 1, u64::MAX] {
        assert_eq!(parse_head(&head(generation, 1)), Err(Error::CorruptJournal));
    }
    for rows in [0, MAX_ROWS + 1, u32::MAX as usize] {
        assert_eq!(parse_head(&head(1, rows)), Err(Error::CorruptJournal));
    }
}
