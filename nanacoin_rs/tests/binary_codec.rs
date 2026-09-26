use nanacoin::{
    domain::{Command, Error, Event, MemberId},
    journal::{decode, encode, FRAME_SIZE},
};

fn event(timestamp: u64, client_key: Option<[u8; 32]>) -> Event {
    Event {
        timestamp,
        client_key,
        version: 1,
        sequence: 42,
        actor: MemberId(1),
        request_id: 41,
        command: Command::Issue {
            to: MemberId(2),
            amount: 123_456,
            memo: "A gift: 🎁 \"hello\"\n".try_into().unwrap(),
        },
    }
}

fn resign(frame: &mut [u8; FRAME_SIZE]) {
    let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
    let mut crc = crc32fast::Hasher::new();
    crc.update(&frame[..8]);
    crc.update(&frame[12..12 + len]);
    frame[8..12].copy_from_slice(&crc.finalize().to_le_bytes());
}

#[test]
fn ncr2_golden_frame() {
    let expected_event = Event {
        timestamp: 1,
        client_key: None,
        version: 2,
        sequence: 2,
        actor: MemberId(1),
        request_id: 2,
        command: Command::Issue {
            to: MemberId(1),
            amount: 7,
            memo: "gift".try_into().unwrap(),
        },
    };
    // NCR2 header + CRC + postcard schema fixture. Enum order and payload
    // layout changes must deliberately update this current-schema contract.
    const GOLDEN: &[u8] = &[
        78, 67, 82, 50, 14, 0, 0, 0, 92, 59, 61, 213, 1, 0, 2, 2, 1, 2, 17, 1, 14, 4, 103, 105,
        102, 116,
    ];
    let mut expected_frame = [0; FRAME_SIZE];
    expected_frame[..GOLDEN.len()].copy_from_slice(GOLDEN);
    assert_eq!(encode(&expected_event).unwrap(), expected_frame);
    assert_eq!(
        serde_json::to_value(decode(&expected_frame).unwrap()).unwrap(),
        serde_json::to_value(expected_event).unwrap()
    );
}

#[test]
fn binary_event_round_trip_handles_zero_time_and_optional_client_key() {
    for timestamp in [0, 1, 1_800_000_000, u64::MAX] {
        for client_key in [None, Some([0; 32]), Some([255; 32])] {
            let original = event(timestamp, client_key);
            let frame = encode(&original).unwrap();
            assert_eq!(&frame[..4], b"NCR2");
            let restored = decode(&frame).unwrap();
            assert_eq!(restored.timestamp, original.timestamp);
            assert_eq!(restored.client_key, original.client_key);
            assert_eq!(restored.version, original.version);
            assert_eq!(restored.sequence, original.sequence);
            assert_eq!(restored.actor, original.actor);
            assert_eq!(restored.request_id, original.request_id);
            assert_eq!(restored.command, original.command);
        }
    }
}

#[test]
fn binary_frame_rejects_corruption_in_header_payload_crc_and_padding() {
    let original = encode(&event(1, Some([7; 32]))).unwrap();
    let len = u32::from_le_bytes(original[4..8].try_into().unwrap()) as usize;
    for index in [
        0,
        3,
        4,
        7,
        8,
        11,
        12,
        12 + len - 1,
        12 + len,
        FRAME_SIZE - 1,
    ] {
        let mut frame = original;
        frame[index] ^= 1;
        assert!(
            matches!(decode(&frame), Err(Error::CorruptJournal)),
            "byte {index}"
        );
    }
    for invalid_len in [0, (FRAME_SIZE - 11) as u32, u32::MAX] {
        let mut frame = original;
        frame[4..8].copy_from_slice(&invalid_len.to_le_bytes());
        assert!(matches!(decode(&frame), Err(Error::CorruptJournal)));
    }
}

#[test]
fn binary_frame_rejects_extra_serialized_bytes_even_with_valid_crc() {
    let mut frame = encode(&event(1, None)).unwrap();
    let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
    frame[4..8].copy_from_slice(&((len + 1) as u32).to_le_bytes());
    resign(&mut frame);
    assert!(matches!(decode(&frame), Err(Error::CorruptJournal)));
}

#[test]
fn binary_frame_rejects_truncated_serialization_even_with_valid_crc() {
    let mut frame = encode(&event(1, None)).unwrap();
    let len = u32::from_le_bytes(frame[4..8].try_into().unwrap()) as usize;
    frame[4..8].copy_from_slice(&((len - 1) as u32).to_le_bytes());
    frame[12 + len - 1] = 0;
    resign(&mut frame);
    assert!(matches!(decode(&frame), Err(Error::CorruptJournal)));
}
