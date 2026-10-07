use super::*;
use crate::journal::FRAME_SIZE;

const NOW: u64 = 1_800_000_000;
fn clock() -> u64 {
    NOW
}
fn notice(id: &str, read: bool) -> Notification {
    Notification {
        id: id.try_into().unwrap(),
        recipient: "Alice".try_into().unwrap(),
        text: "hello 🎁".try_into().unwrap(),
        expires_at: NOW + 100,
        read,
    }
}
#[derive(Default)]
struct Store {
    writes: usize,
    fail: bool,
    saved: Outbox,
}
impl Journal for Store {
    fn read(&mut self, _: usize, _: &mut [u8; FRAME_SIZE]) -> Result<bool, Error> {
        Ok(false)
    }
    fn append(&mut self, _: usize, _: &[u8; FRAME_SIZE]) -> Result<(), Error> {
        panic!("screen delivery must never append a bank event")
    }
    fn set_screen_outbox(&mut self, outbox: &Outbox) -> Result<(), Error> {
        self.writes += 1;
        if self.fail {
            return Err(Error::Storage);
        }
        self.saved = outbox.clone();
        Ok(())
    }
}
type Harness = (
    Service<Store>,
    Worker,
    mpsc::Receiver<Notification>,
    mpsc::SyncSender<(Notification, bool)>,
    Instant,
);
fn harness() -> Harness {
    let service = Service::open_with_clock(Store::default(), clock).unwrap();
    let (send, receive) = mpsc::sync_channel(1);
    let (done, results) = mpsc::sync_channel(1);
    let at = Instant::now();
    let worker = Worker {
        send,
        results,
        busy: false,
        next_attempt: at,
        next_stats: at + Duration::from_secs(3600),
    };
    (service, worker, receive, done, at)
}

#[test]
fn crc_valid_outbox_rejects_trailing_bytes_and_every_truncation() {
    let mut outbox = Outbox::default();
    outbox.pending.push(notice("mail", false)).unwrap();
    let mut bytes = [0; STORAGE_BYTES];
    let valid = outbox.encode(&mut bytes).unwrap().to_vec();
    for end in 4..valid.len() {
        let mut truncated = valid[..end].to_vec();
        let crc = crc32fast::hash(&truncated[4..]);
        truncated[..4].copy_from_slice(&crc.to_le_bytes());
        assert!(Outbox::decode(&truncated).is_err(), "truncation {end}");
    }
    for suffix in [vec![0], vec![255; 32], valid[4..].to_vec()] {
        let mut trailing = valid.clone();
        trailing.extend(suffix);
        let crc = crc32fast::hash(&trailing[4..]);
        trailing[..4].copy_from_slice(&crc.to_le_bytes());
        assert!(Outbox::decode(&trailing).is_err(), "accepted trailing data");
    }
}

struct Fragmented<'a>(&'a [u8]);
impl Read for Fragmented<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        if self.0.is_empty() {
            return Ok(0);
        }
        out[0] = self.0[0];
        self.0 = &self.0[1..];
        Ok(1)
    }
}
#[test]
fn only_complete_http_success_status_lines_acknowledge_delivery() {
    for status in ["HTTP/1.1 200 OK\r\n", "HTTP/1.0 202 Accepted\r\n"] {
        assert!(read_status(&mut Fragmented(status.as_bytes())).is_ok());
        for end in 0..status.len() {
            assert!(
                read_status(&mut Fragmented(&status.as_bytes()[..end])).is_err(),
                "incomplete status {end}"
            );
        }
    }
    for status in [
        "garbage 200 garbage\r\n",
        "HTTP/2 200 OK\r\n",
        "HTTP/1.1\t200 OK\r\n",
        "HTTP/1.1 200 OK\n",
        "HTTP/1.1 2000 OK\r\n",
        "HTTP/1.1 500 Failed\r\n",
        "HTTP/1.1 200 O\0K\r\n",
    ] {
        assert!(
            read_status(&mut Fragmented(status.as_bytes())).is_err(),
            "accepted {status:?}"
        );
    }
    assert!(read_status(&mut Fragmented(&[255; 256])).is_err());
}

#[test]
fn loopback_delivery_sends_the_correct_notice_and_requires_a_real_acknowledgment() {
    use std::net::TcpListener;
    for (read, response, success) in [
        (false, "HTTP/1.1 202 Accepted\r\n\r\n", true),
        (true, "HTTP/1.1 200 OK\r\n\r\n", true),
        (false, "garbage 200 garbage\r\n", false),
        (false, "HTTP/1.1 503 Unavailable\r\n\r\n", false),
    ] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::scope(|scope| {
            let server = scope.spawn(|| {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                loop {
                    let mut buf = [0; 128];
                    let n = socket.read(&mut buf).unwrap();
                    assert_ne!(n, 0);
                    request.extend_from_slice(&buf[..n]);
                    if let Some(end) = request.windows(4).position(|b| b == b"\r\n\r\n") {
                        let headers = std::str::from_utf8(&request[..end]).unwrap();
                        let length = headers
                            .lines()
                            .find_map(|l| l.strip_prefix("Content-Length: "))
                            .unwrap()
                            .parse::<usize>()
                            .unwrap();
                        if request.len() >= end + 4 + length {
                            assert!(headers.starts_with(if read {
                                "POST /api/screen/read HTTP/1.1"
                            } else {
                                "POST /api/screen/notify HTTP/1.1"
                            }));
                            let payload: serde_json::Value =
                                serde_json::from_slice(&request[end + 4..]).unwrap();
                            assert_eq!(
                                payload["event_id"],
                                if read { "mail-read" } else { "mail-notify" }
                            );
                            assert_eq!(payload["text"], "hello 🎁");
                            assert_eq!(payload["expires_at"], NOW + 100);
                            socket.write_all(response.as_bytes()).unwrap();
                            break;
                        }
                    }
                }
            });
            assert_eq!(deliver(&url, &notice("mail", read)).is_ok(), success);
            server.join().unwrap();
        });
    }
    for url in [
        "https://127.0.0.1",
        "http://",
        "http://user@127.0.0.1",
        "http://127.0.0.1/path",
        "http://127.0.0.1?x",
        "http://127.0.0.1#x",
        "http://127.0.0.1:0",
        "http://127.0.0.1:bad",
        "http://127.0.0.1:65536",
        "http://127.0.0.1\r\nInjected: bad",
    ] {
        assert!(deliver(url, &notice("mail", false)).is_err(), "{url:?}");
    }
}

#[test]
fn failed_delivery_waits_thirty_seconds_and_success_removes_durably() {
    let (mut s, mut w, receive, done, at) = harness();
    let n = notice("one", false);
    s.queue_screen(n.clone()).unwrap();
    w.pump_at(&mut s, at);
    assert_eq!(receive.try_recv().unwrap(), n);
    w.pump_at(&mut s, at); // no duplicate while busy
    assert!(receive.try_recv().is_err());
    done.send((n.clone(), false)).unwrap();
    w.pump_at(&mut s, at);
    w.pump_at(&mut s, at + Duration::from_secs(29));
    assert!(receive.try_recv().is_err());
    assert_eq!(s.screen_pending(), std::slice::from_ref(&n));
    w.pump_at(&mut s, at + Duration::from_secs(30));
    assert_eq!(receive.try_recv().unwrap(), n);
    done.send((n, true)).unwrap();
    w.pump_at(&mut s, at + Duration::from_secs(30));
    assert!(s.screen_pending().is_empty());
    assert!(s.journal.saved.pending.is_empty());
}

#[test]
fn successful_delivery_with_failed_removal_retries_the_same_notice() {
    let (mut s, mut w, receive, done, at) = harness();
    let n = notice("one", false);
    s.queue_screen(n.clone()).unwrap();
    w.pump_at(&mut s, at);
    receive.try_recv().unwrap();
    s.journal.fail = true;
    done.send((n.clone(), true)).unwrap();
    w.pump_at(&mut s, at);
    assert_eq!(s.screen_pending(), std::slice::from_ref(&n));
    assert_eq!(s.journal.saved.pending.as_slice(), std::slice::from_ref(&n));
    s.journal.fail = false;
    w.pump_at(&mut s, at + Duration::from_secs(30));
    assert_eq!(receive.try_recv().unwrap(), n);
}

#[test]
fn stale_copy_result_cannot_remove_a_superseding_read() {
    let (mut s, mut w, receive, done, at) = harness();
    let copy = notice("mail", false);
    let read = notice("mail", true);
    s.queue_screen(copy.clone()).unwrap();
    w.pump_at(&mut s, at);
    receive.try_recv().unwrap();
    s.queue_screen(read.clone()).unwrap();
    done.send((copy, true)).unwrap();
    w.pump_at(&mut s, at);
    assert_eq!(s.screen_pending(), std::slice::from_ref(&read));
    assert_eq!(receive.try_recv().unwrap(), read);
}

#[test]
fn full_queue_failed_writes_and_expiry_never_partially_publish() {
    let (mut s, mut w, receive, _, at) = harness();
    for i in 0..CAPACITY {
        s.queue_screen(notice(&i.to_string(), false)).unwrap();
    }
    let before = s.screen_pending().to_vec();
    assert_eq!(
        s.queue_screen(notice("overflow", false)),
        Err(Error::Capacity)
    );
    assert_eq!(s.screen_pending(), before);
    s.journal.fail = true;
    assert_eq!(s.queue_screen(notice("0", true)), Err(Error::Storage));
    assert_eq!(s.screen_pending(), before);
    s.screen_outbox.pending[0].expires_at = NOW;
    w.pump_at(&mut s, at);
    assert!(receive.try_recv().is_err());
    assert_eq!(s.screen_pending().len(), CAPACITY);
    s.journal.fail = false;
    w.pump_at(&mut s, at);
    assert_eq!(receive.try_recv().unwrap().id.as_str(), "1");
    assert_eq!(s.screen_pending().len(), CAPACITY - 1);
}

#[test]
fn channel_backpressure_and_invalid_clock_do_not_lose_notices() {
    let (mut s, mut w, receive, _, at) = harness();
    let n = notice("one", false);
    s.queue_screen(n.clone()).unwrap();
    w.send.try_send(notice("occupies-channel", false)).unwrap();
    w.pump_at(&mut s, at);
    assert!(!w.busy);
    receive.try_recv().unwrap();
    fn invalid_clock() -> u64 {
        0
    }
    let mut invalid = Service::open_with_clock(Store::default(), invalid_clock).unwrap();
    invalid.screen_outbox = s.screen_outbox.clone();
    w.pump_at(&mut invalid, at);
    assert!(receive.try_recv().is_err());
    assert_eq!(invalid.screen_pending(), std::slice::from_ref(&n));
    drop(receive);
    w.pump_at(&mut s, at);
    assert!(!w.busy);
    assert_eq!(s.screen_pending(), &[n]);
}
