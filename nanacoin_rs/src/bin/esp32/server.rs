//! Core 0 establishes TLS; core 1 multiplexes established HTTP(S) connections.
//! No socket read, write, or handshake waits inside the ledger mutex. All queues,
//! connections and request buffers are bounded; partial I/O yields to peers.
use super::{diagnostics, nvs_journal::NvsJournal, Diagnostics};
use esp_idf_svc::{
    hal::{cpu::Core, task::thread::ThreadSpawnConfiguration},
    sys,
};
use nanacoin::incidents::{Kind, LOG};
use nanacoin::{
    api,
    domain::Error,
    http_transport::{self as http, Body, Request, Response},
    journal::Service,
};
use std::{
    io::{self, Read, Write},
    net::{TcpListener, TcpStream},
    os::fd::AsRawFd,
    ptr::NonNull,
    sync::{atomic::Ordering, mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

const TLS_CLIENTS: usize = 8;
const HTTP_CLIENTS: usize = 4;
const HANDSHAKES: usize = 2;
const IDLE: Duration = Duration::from_secs(60);
const IO_DEADLINE: Duration = Duration::from_secs(5);
// Pause dispatch before another potentially 512 KiB response is constructed.
// At most this budget plus one maximum reply is retained, even with slow peers.
const RESPONSE_BUDGET: usize = 2 * 1024 * 1024;

pub struct Context {
    pub shared: Arc<Mutex<Service<NvsJournal>>>,
    pub diagnostics: Arc<Diagnostics>,
}

/// The socket owns its fd. IDF 5.5.3 server_session_delete frees only the TLS
/// context (unlike conn_destroy); TcpStream closes the descriptor exactly once.
struct Socket {
    tcp: TcpStream,
    tls: Option<NonNull<sys::esp_tls_t>>,
}

// SAFETY: a Socket has a single owner, never shared access. The handshake task
// transfers the completed TLS context through a channel and never touches it
// again. All subsequent TLS calls and destruction happen on the receiving task.
unsafe impl Send for Socket {}

impl Drop for Socket {
    fn drop(&mut self) {
        if let Some(tls) = self.tls {
            // SAFETY: exclusively owned, live context; TCP closes afterward.
            unsafe { sys::esp_tls_server_session_delete(tls.as_ptr()) };
        }
    }
}

fn tls_result(n: isize) -> io::Result<usize> {
    match n as i32 {
        sys::ESP_TLS_ERR_SSL_WANT_READ | sys::ESP_TLS_ERR_SSL_WANT_WRITE => {
            Err(io::ErrorKind::WouldBlock.into())
        }
        n if n < 0 => {
            incident(Kind::SocketError, n, 0);
            Err(io::ErrorKind::ConnectionAborted.into())
        }
        _ => Ok(n as usize),
    }
}

impl Read for Socket {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        match self.tls {
            // SAFETY: the context is exclusively owned and out is writable.
            Some(tls) => tls_result(unsafe {
                sys::esp_tls_conn_read(tls.as_ptr(), out.as_mut_ptr().cast(), out.len())
            }),
            None => self.tcp.read(out),
        }
    }
}

impl Write for Socket {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.tls {
            // SAFETY: exclusive TLS context and valid input slice. A retry after
            // WANT_WRITE uses the same response slice and length until progress.
            Some(tls) => tls_result(unsafe {
                sys::esp_tls_conn_write(tls.as_ptr(), bytes.as_ptr().cast(), bytes.len())
            }),
            None => self.tcp.write(bytes),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn socket(tcp: TcpStream) -> io::Result<Socket> {
    tcp.set_nodelay(true)?;
    tcp.set_nonblocking(true)?;
    Ok(Socket { tcp, tls: None })
}

/// The TLS configuration and ticket keys live for this task's lifetime. Only
/// this task creates sessions; no pointer to the config crosses the channel.
fn handshakes(listener: TcpListener, ready: mpsc::SyncSender<Socket>) {
    // SAFETY: zero is IDF's documented default; PEM inputs are static and NUL
    // terminated. Config remains alive until after every pending handshake.
    let mut cfg: sys::esp_tls_cfg_server_t = unsafe { core::mem::zeroed() };
    let cert = concat!(include_str!("../../../certs/nanacoin-ca-signed.crt"), "\0").as_bytes();
    let key = concat!(include_str!("../../../certs/nanacoin-ca-signed.key"), "\0").as_bytes();
    cfg.__bindgen_anon_3.servercert_buf = cert.as_ptr();
    cfg.__bindgen_anon_4.servercert_bytes = cert.len() as _;
    cfg.__bindgen_anon_5.serverkey_buf = key.as_ptr();
    cfg.__bindgen_anon_6.serverkey_bytes = key.len() as _;
    // SAFETY: valid config owned on this task. Ticket context intentionally lives
    // until reboot, including after a session moves to the serving task.
    let tickets = unsafe { sys::esp_tls_cfg_server_session_tickets_init(&mut cfg) };
    if tickets != sys::ESP_OK {
        incident(Kind::TlsInitFailed, tickets, 0);
        log::error!("TLS ticket initialization failed: {tickets}");
    }
    let mut pending: Vec<(Socket, Instant)> = Vec::with_capacity(HANDSHAKES);
    loop {
        LOG.beat(0, incident_now());
        LOG.connections(0, pending.len());
        if pending.len() < HANDSHAKES {
            if let Ok((tcp, _)) = listener.accept() {
                if let Ok(mut stream) = socket(tcp) {
                    // SAFETY: init allocates an exclusively owned context. On
                    // every failure Socket's Drop releases context and fd.
                    if let Some(tls) = NonNull::new(unsafe { sys::esp_tls_init() }) {
                        stream.tls = Some(tls);
                        let result = unsafe {
                            sys::esp_tls_server_session_init(
                                &mut cfg,
                                stream.tcp.as_raw_fd(),
                                tls.as_ptr(),
                            )
                        };
                        if result == sys::ESP_OK {
                            pending.push((stream, Instant::now()));
                            LOG.connections(0, pending.len());
                        } else {
                            incident(Kind::TlsInitFailed, result, 0);
                        }
                    } else {
                        incident(Kind::AllocationFailed, 1, 0);
                    }
                }
            }
        }
        let mut i = 0;
        while i < pending.len() {
            let (stream, began) = &pending[i];
            if began.elapsed() >= Duration::from_secs(4) {
                incident(Kind::TlsTimeout, 0, began.elapsed().as_millis() as u32);
                pending.swap_remove(i);
                continue;
            }
            // SAFETY: context exclusively owned here; socket is nonblocking.
            let result =
                unsafe { sys::esp_tls_server_session_continue_async(stream.tls.unwrap().as_ptr()) };
            match result {
                0 => {
                    let (stream, began) = pending.swap_remove(i);
                    log::debug!(
                        "TLS established in {} ms on core 0",
                        began.elapsed().as_millis()
                    );
                    // Bounded handoff; a full queue drops the newly established
                    // session rather than retaining unbounded sockets/memory.
                    LOG.handshake(began.elapsed().as_millis() as u32);
                    if ready.try_send(stream).is_err() {
                        incident(Kind::HandoffFull, 0, 0);
                    }
                }
                sys::ESP_TLS_ERR_SSL_WANT_READ | sys::ESP_TLS_ERR_SSL_WANT_WRITE => i += 1,
                _ => {
                    incident(Kind::TlsFailed, result, began.elapsed().as_millis() as u32);
                    pending.swap_remove(i);
                }
            }
        }
        // IDF's libc usleep busy-waits below one tick. Explicit FreeRTOS delay
        // rounds up and lets IDLE0 run/feed its watchdog even with no traffic.
        esp_idf_svc::hal::delay::FreeRtos::delay_ms(1);
    }
}

struct Client {
    socket: Socket,
    input: Vec<u8>,
    used: usize,
    response: Option<Response>,
    active: Instant,
    request_started: Option<Instant>,
    send_started: Option<Instant>,
}

impl Client {
    fn new(socket: Socket) -> Self {
        Self {
            socket,
            input: vec![0; http::INPUT_LIMIT],
            used: 0,
            response: None,
            active: Instant::now(),
            request_started: None,
            send_started: None,
        }
    }

    /// One bounded turn per client. A stalled reader/writer never sleeps here.
    fn poll(&mut self, ctx: &Context, output: &mut [u8], may_dispatch: bool) -> bool {
        if self.active.elapsed() > IDLE {
            incident(Kind::IdleExpired, 0, 0);
            return false;
        }
        if self
            .request_started
            .is_some_and(|t| t.elapsed() > IO_DEADLINE)
            || self.send_started.is_some_and(|t| t.elapsed() > IO_DEADLINE)
        {
            incident(
                Kind::RequestTimeout,
                if self.send_started.is_some() { 1 } else { 0 },
                IO_DEADLINE.as_millis() as u32,
            );
            return false;
        }
        if self.response.is_none() {
            if !may_dispatch {
                return true;
            }
            // Process an already buffered pipelined request before touching the
            // socket again (the peer may have half-closed after sending it).
            let mut parsed = http::parse(&self.input[..self.used]);
            if matches!(parsed, Ok(None)) {
                match self.socket.read(&mut self.input[self.used..]) {
                    Ok(0) => return false,
                    Ok(n) => {
                        self.used += n;
                        self.active = Instant::now();
                        self.request_started.get_or_insert(self.active);
                        parsed = http::parse(&self.input[..self.used]);
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        if self.socket.tls.is_none() {
                            incident(Kind::SocketError, e.raw_os_error().unwrap_or(0), 0);
                        }
                        return false;
                    }
                }
            }
            match parsed {
                Ok(Some(request)) => {
                    let consumed = request.consumed;
                    self.response = Some(respond(ctx, &request, self.socket.tls.is_some(), output));
                    self.input.copy_within(consumed..self.used, 0);
                    self.used -= consumed;
                    self.request_started = if self.used == 0 {
                        None
                    } else {
                        Some(Instant::now())
                    };
                    self.send_started = Some(Instant::now());
                }
                Ok(None) if self.used < self.input.len() => {}
                other => {
                    let status = other.err().unwrap_or(413);
                    incident(Kind::InvalidRequest, status as i32, 0);
                    self.response = Some(Response::new(
                        status,
                        &[("Content-Type", "application/json")],
                        Body::Flash(b"{\"error\":\"invalid_http_request\"}"),
                        false,
                        true,
                    ));
                    self.request_started = None;
                    self.send_started = Some(Instant::now());
                }
            }
        }
        if let Some(response) = &mut self.response {
            if !response.next().is_empty() {
                match self.socket.write(response.next()) {
                    Ok(0) => return false,
                    Ok(n) => {
                        response.advance(n);
                        self.active = Instant::now();
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        if self.socket.tls.is_none() {
                            incident(Kind::SocketError, e.raw_os_error().unwrap_or(0), 0);
                        }
                        return false;
                    }
                }
            }
            if response.next().is_empty() {
                if let Some(started) = self.send_started {
                    if started.elapsed().as_millis() >= 500 {
                        incident(Kind::SlowRequest, 0, started.elapsed().as_millis() as u32);
                    }
                }
                if response.close {
                    return false;
                }
                self.response = None;
                self.send_started = None;
            }
        }
        true
    }
}

fn add_client(clients: &mut Vec<Client>, stream: Socket) {
    let secure = stream.tls.is_some();
    let limit = if secure { TLS_CLIENTS } else { HTTP_CLIENTS };
    if clients
        .iter()
        .filter(|c| c.socket.tls.is_some() == secure)
        .count()
        >= limit
    {
        // Reclaim only idle sessions of the same transport, never an in-flight
        // response or partial request. Allow active clients to finish.
        if let Some(index) = clients
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                c.socket.tls.is_some() == secure
                    && c.response.is_none()
                    && c.used == 0
                    && c.active.elapsed() > Duration::from_secs(1)
            })
            .max_by_key(|(_, c)| c.active.elapsed())
            .map(|(i, _)| i)
        {
            clients.swap_remove(index);
        } else {
            incident(Kind::AdmissionRejected, i32::from(secure), 0);
            return;
        }
    }
    clients.push(Client::new(stream));
}

pub fn start(ctx: Context) -> Result<(), Box<dyn std::error::Error>> {
    let tls_listener = TcpListener::bind(("0.0.0.0", 443))?;
    let http_listener = TcpListener::bind(("0.0.0.0", 80))?;
    tls_listener.set_nonblocking(true)?;
    http_listener.set_nonblocking(true)?;
    let (ready, completed) = mpsc::sync_channel(2);
    ThreadSpawnConfiguration {
        name: Some(c"nanacoin-tls"),
        priority: 4,
        pin_to_core: Some(Core::Core0),
        ..Default::default()
    }
    .set()?;
    // Rust supplies pthread attributes, whose stack size overrides IDF's
    // ThreadSpawnConfiguration. Set it on Builder or this gets only 3 KiB.
    std::thread::Builder::new()
        .stack_size(24 * 1024)
        .spawn(move || handshakes(tls_listener, ready))?;
    ThreadSpawnConfiguration {
        name: Some(c"nanacoin-http"),
        priority: 5,
        pin_to_core: Some(Core::Core1),
        ..Default::default()
    }
    .set()?;
    std::thread::Builder::new()
        .stack_size(32 * 1024)
        .spawn(move || {
            let mut clients = Vec::with_capacity(TLS_CLIENTS + HTTP_CLIENTS);
            let mut output = vec![0; api::RESPONSE_LIMIT];
            loop {
                if let Ok(stream) = completed.try_recv() {
                    add_client(&mut clients, stream);
                }
                if let Ok((tcp, _)) = http_listener.accept() {
                    if let Ok(stream) = socket(tcp) {
                        add_client(&mut clients, stream);
                    }
                }
                let mut queued: usize = clients
                    .iter()
                    .filter_map(|c| c.response.as_ref())
                    .map(Response::retained_bytes)
                    .sum();
                LOG.beat(1, incident_now());
                LOG.connections(1, clients.iter().filter(|c| c.socket.tls.is_some()).count());
                LOG.connections(2, clients.iter().filter(|c| c.socket.tls.is_none()).count());
                clients.retain_mut(|client| {
                    let before = client.response.as_ref().map_or(0, Response::retained_bytes);
                    let keep = client.poll(&ctx, &mut output, queued < RESPONSE_BUDGET);
                    queued -= before;
                    if keep {
                        queued += client.response.as_ref().map_or(0, Response::retained_bytes);
                    }
                    keep
                });
                esp_idf_svc::hal::delay::FreeRtos::delay_ms(1);
            }
        })?;
    ThreadSpawnConfiguration::default().set()?;
    Ok(())
}

fn respond(ctx: &Context, request: &Request, secure: bool, output: &mut [u8]) -> Response {
    let began = Instant::now();
    let method = request.method.as_str();
    let uri = request.uri.as_str();
    let locked_http = !secure && ctx.shared.lock().unwrap().https_only();
    let static_reply = if locked_http {
        nanacoin::web::onboarding(method, uri, true)
    } else {
        nanacoin::web::respond(
            method,
            uri,
            request.header("Accept-Encoding"),
            request.header("If-None-Match"),
        )
    };
    if let Some(reply) = static_reply {
        return Response::new(
            reply.status,
            &reply.headers,
            Body::Flash(reply.bytes),
            method == "HEAD",
            request.close,
        );
    }
    let origin = request.header("Origin");
    let allowed = origin.len() <= 256
        && (origin == "https://nanacoin.local"
            || origin == "http://nanacoin.local"
            || api::origin_allowed(
                origin,
                option_env!("NANACOIN_ORIGINS").unwrap_or(api::DEFAULT_ORIGINS),
            ));
    ctx.diagnostics.requests.fetch_add(1, Ordering::Relaxed);
    let mut generation = None;
    let mut lock_ms = 0.0;
    let mut app_ms = 0.0;
    let path = uri.split('?').next().unwrap_or("");
    let (status, len) = if !allowed {
        api::error_response(Error::Forbidden, output)
    } else if method == "OPTIONS" {
        output[..2].copy_from_slice(b"{}");
        (200, 2)
    } else if method == "GET"
        && matches!(
            path,
            "/api/v1/diag" | "/api/v1/diag/static" | "/api/v1/diag/events"
        )
    {
        if path.ends_with("/events") {
            let limit = output.len().min(nanacoin::incidents::RESPONSE_BYTES);
            nanacoin::diagnostics::response(&LOG.snapshot(), &mut output[..limit])
        } else if path.ends_with("/static") {
            nanacoin::diagnostics::response(&diagnostics::system_info(), output)
        } else {
            nanacoin::diagnostics::response(&ctx.diagnostics.snapshot(), output)
        }
    } else {
        let waiting = Instant::now();
        let mut service = ctx.shared.lock().unwrap();
        let failed_before = service.storage_failed();
        lock_ms = waiting.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let reply = api::handle_keyed_on(
            &mut service,
            method,
            uri,
            request.header("Authorization"),
            request.header("Idempotency-Key"),
            &request.body,
            output,
            secure,
        );
        app_ms = started.elapsed().as_secs_f64() * 1000.0;
        generation = Some(service.generation());
        if !failed_before && service.storage_failed() {
            incident(Kind::StorageFailed, 0, 0);
        }
        reply
    };
    if began.elapsed().as_millis() >= 500 {
        incident(
            Kind::SlowRequest,
            status as i32,
            began.elapsed().as_millis() as u32,
        );
    }
    let generation = generation.map(|g| g.to_string());
    let timing = format!(
        "app;dur={app_ms:.3}, lock;dur={lock_ms:.3}, handler;dur={:.3}",
        began.elapsed().as_secs_f64() * 1000.0
    );
    let mut headers = vec![
        ("Content-Type", "application/json"),
        ("Cache-Control", "no-store"),
        ("Vary", "Origin"),
        ("Server-Timing", timing.as_str()),
    ];
    if allowed && !origin.is_empty() {
        headers.extend_from_slice(&[
            ("Access-Control-Allow-Origin", origin),
            ("Access-Control-Allow-Methods", "GET, POST, PATCH, OPTIONS"),
            (
                "Access-Control-Allow-Headers",
                "Authorization, Content-Type, Idempotency-Key",
            ),
            (
                "Access-Control-Expose-Headers",
                "X-Nanacoin-Generation, Server-Timing",
            ),
            ("Access-Control-Max-Age", "600"),
        ]);
    }
    if let Some(generation) = generation.as_deref() {
        headers.push(("X-Nanacoin-Generation", generation));
    }
    if status >= 400 {
        ctx.diagnostics.errors.fetch_add(1, Ordering::Relaxed);
    }
    Response::new(
        status,
        &headers,
        Body::Owned(output[..len].to_vec()),
        method == "HEAD",
        request.close,
    )
}

fn incident_now() -> u64 {
    unsafe { (sys::esp_timer_get_time() / 1000) as u64 }
}
fn incident(kind: Kind, code: i32, ms: u32) {
    LOG.record(incident_now(), kind, code, ms);
}
