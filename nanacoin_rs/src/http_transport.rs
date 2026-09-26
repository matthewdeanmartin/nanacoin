//! Bounded HTTP/1 framing for the board's nonblocking connection loop.
//! A partial request never blocks another client. Invalid framing always closes
//! the connection, so unread bytes cannot become a second, ambiguous request.
use crate::api::BODY_LIMIT;

pub const HEADER_LIMIT: usize = 4096;
pub const INPUT_LIMIT: usize = HEADER_LIMIT + BODY_LIMIT;

#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub uri: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub consumed: usize,
    pub close: bool,
}

impl Request {
    pub fn header(&self, name: &str) -> &str {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
            .unwrap_or("")
    }
}

pub fn parse(input: &[u8]) -> Result<Option<Request>, u16> {
    let mut headers = [httparse::EMPTY_HEADER; 32];
    let mut parsed = httparse::Request::new(&mut headers);
    let start = match parsed.parse(input).map_err(|_| 400u16)? {
        httparse::Status::Partial if input.len() >= HEADER_LIMIT => return Err(431),
        httparse::Status::Partial => return Ok(None),
        httparse::Status::Complete(n) if n > HEADER_LIMIT => return Err(431),
        httparse::Status::Complete(n) => n,
    };
    let method = parsed.method.ok_or(400u16)?;
    let uri = parsed.path.ok_or(400u16)?;
    if uri.len() > 256 {
        return Err(414);
    }
    if !uri.starts_with('/') || uri.starts_with("//") {
        return Err(400);
    }
    let mut length = None;
    let mut host = false;
    let mut close = parsed.version != Some(1);
    for (index, header) in parsed.headers.iter().enumerate() {
        // Do not let different layers interpret duplicate security/framing
        // headers differently. Repeated headers are unnecessary for this API.
        if parsed.headers[..index]
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case(header.name))
        {
            return Err(400);
        }
        let value = std::str::from_utf8(header.value)
            .map_err(|_| 400u16)?
            .trim();
        if value.bytes().any(|b| b < 32 && b != b'\t') || value.contains('\x7f') {
            return Err(400);
        }
        if header.name.eq_ignore_ascii_case("Transfer-Encoding") {
            return Err(400);
        }
        if header.name.eq_ignore_ascii_case("Expect") {
            return Err(417);
        }
        if header.name.eq_ignore_ascii_case("Host") {
            host = !value.is_empty();
        }
        if header.name.eq_ignore_ascii_case("Connection")
            && value
                .split(',')
                .any(|v| v.trim().eq_ignore_ascii_case("close"))
        {
            close = true;
        }
        if header.name.eq_ignore_ascii_case("Content-Length") {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(400);
            }
            let n = value.parse::<usize>().map_err(|_| 413u16)?;
            if n > BODY_LIMIT {
                return Err(413);
            }
            length = Some(n);
        }
    }
    if parsed.version == Some(1) && !host {
        return Err(400);
    }
    if matches!(method, "POST" | "PATCH") && length.is_none() {
        return Err(411);
    }
    let end = start + length.unwrap_or(0);
    if input.len() < end {
        return Ok(None);
    }
    Ok(Some(Request {
        method: method.to_owned(),
        uri: uri.to_owned(),
        headers: parsed
            .headers
            .iter()
            .map(|h| {
                (
                    h.name.to_owned(),
                    std::str::from_utf8(h.value).unwrap().trim().to_owned(),
                )
            })
            .collect(),
        body: input[start..end].to_vec(),
        consumed: end,
        close,
    }))
}

pub enum Body {
    Owned(Vec<u8>),
    Flash(&'static [u8]),
}

impl AsRef<[u8]> for Body {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Owned(b) => b,
            Self::Flash(b) => b,
        }
    }
}

/// Coalesce headers and small bodies into one write, retain large flash assets
/// by reference, and allow partial writes to resume on a later loop iteration.
pub struct Response {
    prefix: Vec<u8>,
    body: Body,
    skip: usize,
    sent: usize,
    pub close: bool,
}

impl Response {
    pub fn new(status: u16, headers: &[(&str, &str)], body: Body, head: bool, close: bool) -> Self {
        let mut prefix = format!("HTTP/1.1 {status} {}\r\n", reason(status)).into_bytes();
        for &(name, value) in headers {
            if name.eq_ignore_ascii_case("Content-Length")
                || name.eq_ignore_ascii_case("Transfer-Encoding")
                || name.eq_ignore_ascii_case("Connection")
                || name.contains(['\r', '\n'])
                || value.contains(['\r', '\n'])
            {
                continue;
            }
            prefix.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        // 304 has no message body; omit Content-Length (which otherwise would
        // have to describe the selected representation, not this empty reply).
        if status != 304 {
            prefix.extend_from_slice(
                format!("Content-Length: {}\r\n", body.as_ref().len()).as_bytes(),
            );
        }
        if close {
            prefix.extend_from_slice(b"Connection: close\r\n");
        }
        prefix.extend_from_slice(b"\r\n");
        let body = if head || status == 304 {
            Body::Flash(b"")
        } else {
            body
        };
        let skip = body.as_ref().len().min(2048);
        prefix.extend_from_slice(&body.as_ref()[..skip]);
        Self {
            prefix,
            body,
            skip,
            sent: 0,
            close,
        }
    }

    pub fn next(&self) -> &[u8] {
        if self.sent < self.prefix.len() {
            &self.prefix[self.sent..]
        } else {
            let rest = &self.body.as_ref()[self.skip + self.sent - self.prefix.len()..];
            &rest[..rest.len().min(8192)]
        }
    }

    pub fn advance(&mut self, count: usize) {
        self.sent += count;
    }

    pub fn retained_bytes(&self) -> usize {
        self.prefix.len()
            + match &self.body {
                Body::Owned(bytes) => bytes.len(),
                Body::Flash(_) => 0,
            }
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        410 => "Gone",
        411 => "Length Required",
        413 => "Content Too Large",
        414 => "URI Too Long",
        417 => "Expectation Failed",
        422 => "Unprocessable Content",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmented_body_and_pipelining_have_exact_boundaries() {
        let first =
            b"POST /api/v1/test HTTP/1.1\r\nHost: nanacoin.local\r\nContent-Length: 2\r\n\r\n{}";
        for end in 0..first.len() {
            assert!(parse(&first[..end]).unwrap().is_none());
        }
        let mut data = first.to_vec();
        data.extend_from_slice(
            b"GET /api/v1/status HTTP/1.1\r\nHost: nanacoin.local\r\nConnection: close\r\n\r\n",
        );
        let a = parse(&data).unwrap().unwrap();
        assert_eq!(a.body, b"{}");
        assert_eq!(a.consumed, first.len());
        assert!(!a.close);
        let b = parse(&data[a.consumed..]).unwrap().unwrap();
        assert!(b.close);
        assert_eq!(b.uri, "/api/v1/status");
    }

    #[test]
    fn reject_ambiguous_and_oversized_framing_before_body() {
        for extra in [
            "Content-Length: 0\r\nContent-Length: 1\r\n",
            "Transfer-Encoding: chunked\r\n",
            "Content-Length: +1\r\n",
            "Host: other\r\n",
        ] {
            assert_eq!(
                parse(format!("GET / HTTP/1.1\r\nHost: n\r\n{extra}\r\n").as_bytes()).unwrap_err(),
                400
            );
        }
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nHost: n\r\nContent-Length: 1025\r\n\r\n").unwrap_err(),
            413
        );
        assert_eq!(
            parse(b"POST / HTTP/1.1\r\nHost: n\r\n\r\n").unwrap_err(),
            411
        );
        assert_eq!(parse(b"GET / HTTP/1.1\r\n\r\n").unwrap_err(), 400);
        let huge = format!(
            "GET / HTTP/1.1\r\nHost: n\r\nX-Pad: {}",
            "x".repeat(HEADER_LIMIT)
        );
        assert_eq!(parse(huge.as_bytes()).unwrap_err(), 431);
    }

    fn wire(mut reply: Response) -> Vec<u8> {
        let mut out = Vec::new();
        while !reply.next().is_empty() {
            let count = reply.next().len().min(37);
            out.extend_from_slice(&reply.next()[..count]);
            reply.advance(count);
        }
        out
    }

    #[test]
    fn persistent_response_handles_partial_writes_and_large_bodies() {
        let body = vec![b'x'; 24000];
        let out = wire(Response::new(
            200,
            &[("Transfer-Encoding", "chunked"), ("Bad", "a\r\nb")],
            Body::Owned(body.clone()),
            false,
            false,
        ));
        let split = out.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
        let headers = std::str::from_utf8(&out[..split]).unwrap();
        assert!(headers.contains("Content-Length: 24000\r\n"));
        assert!(!headers.contains("Connection: close"));
        assert!(!headers.contains("Transfer-Encoding"));
        assert!(!headers.contains("Bad:"));
        assert_eq!(&out[split..], body);
    }

    #[test]
    fn head_and_not_modified_have_no_body() {
        let head = wire(Response::new(200, &[], Body::Flash(b"hello"), true, false));
        assert!(head.ends_with(b"Content-Length: 5\r\n\r\n"));
        let cached = wire(Response::new(304, &[], Body::Flash(b"hello"), false, false));
        assert_eq!(cached, b"HTTP/1.1 304 Not Modified\r\n\r\n");
    }
}
