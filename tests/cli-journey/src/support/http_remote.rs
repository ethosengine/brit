//! HttpRemote — a local smart-HTTP remote that can fail the way servers and networks do.
//!
//! An HTTP/1.1 server on `127.0.0.1:0`, one thread per connection, that answers each request
//! by running `git http-backend` over the repositories below a project root. Connections stay
//! open between requests, like a real server's keep-alive, so a client can reuse one across
//! the gap a `pre-push` hook leaves between the ref advertisement and the pack upload.
//!
//! Modes reproduce the failures seen against real remotes:
//! - [`Mode::DropIdle`]: forget a connection idle longer than a limit, and answer the next
//!   bytes on it with a TCP reset (what load balancers do to stale keep-alive connections).
//! - [`Mode::EarlyStatus`]: answer a receive-pack POST with an error status before reading
//!   its body (401 for expired credentials, 413 for a body too large), then drain the body.
//! - [`Mode::EarlyStatusReset`]: the same answer, then close without reading the body.
//! - [`Mode::ResetAfter`]: read part of the receive-pack upload, then reset the connection.
//! - [`Mode::GoneAfterGet`]: close the listener and the connection after the first GET.
//! - [`Mode::HoldResponse`]: read the whole upload, then wait before answering.
//! - [`Mode::HoldGet`]: wait before answering each GET.
//! - [`Mode::StallFetch`]: send part of an upload-pack response, then stall.
//! - [`Mode::LoseReport`]: apply a push, then close without answering, as a connection cut
//!   after the server finished does.
//! - [`Mode::MoveOnPost`]: move `refs/heads/main` when the receive-pack POST arrives, so the
//!   server's own compare-and-swap rejects the push.
//!
//! Dropping the fixture shuts every connection down, wakes every pause and joins every thread.

use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{Shutdown, SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

/// How long one `git http-backend` run may take before the fixture kills it.
const BACKEND_LIMIT: Duration = Duration::from_secs(60);

/// How the fixture misbehaves.
#[derive(Debug, Clone, Copy)]
pub enum Mode {
    /// Serve every request.
    Serve,
    /// A connection idle for longer than this after a response is forgotten; the next bytes
    /// the client sends on it are answered with a reset.
    DropIdle(Duration),
    /// Answer receive-pack POSTs with this status before reading the body, then drain the
    /// body and close.
    EarlyStatus(u16),
    /// Answer receive-pack POSTs with this status before reading the body, then close with
    /// the body unread, which resets the connection.
    EarlyStatusReset(u16),
    /// Read this many bytes of a receive-pack POST, then reset the connection.
    ResetAfter(usize),
    /// After answering the first GET, close the listener and that connection.
    GoneAfterGet,
    /// Read a receive-pack POST completely, then wait this long before answering it.
    HoldResponse(Duration),
    /// Wait this long before answering each GET.
    HoldGet(Duration),
    /// Answer an upload-pack POST with only this many bytes of its body, then stall.
    StallFetch(usize),
    /// Apply each receive-pack POST, then close the connection without answering.
    LoseReport,
    /// Move `refs/heads/main` to a new commit when a receive-pack POST arrives, then apply it.
    MoveOnPost,
}

/// A running fixture; dropping it stops the server.
pub struct HttpRemote {
    addr: SocketAddr,
    shared: Arc<Shared>,
    accept: Option<JoinHandle<()>>,
}

struct Shared {
    root: PathBuf,
    mode: Mode,
    stop: AtomicBool,
    listener: Mutex<Option<TcpListener>>,
    /// Each connection's socket, to shut it down on drop, and its thread, to join. The socket is
    /// held weakly so that a connection closes as soon as its thread lets go of it, which is
    /// what makes closing with unread data a reset.
    connections: Mutex<Vec<(Weak<TcpStream>, JoinHandle<()>)>>,
    log: Mutex<Vec<String>>,
}

impl Shared {
    fn note(&self, line: String) {
        self.log.lock().expect("log lock").push(line);
    }

    /// Sleep for `duration` unless the fixture stops first; `false` if it stopped.
    fn pause(&self, duration: Duration) -> bool {
        let until = Instant::now() + duration;
        while Instant::now() < until {
            if self.stop.load(Ordering::SeqCst) {
                return false;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        !self.stop.load(Ordering::SeqCst)
    }
}

impl HttpRemote {
    /// Serve the repositories below `project_root` in `mode`.
    pub fn start(project_root: &Path, mode: Mode) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;
        let shared = Arc::new(Shared {
            root: project_root.to_owned(),
            mode,
            stop: AtomicBool::new(false),
            listener: Mutex::new(Some(listener)),
            connections: Mutex::new(Vec::new()),
            log: Mutex::new(Vec::new()),
        });
        let accept = std::thread::spawn({
            let shared = shared.clone();
            move || accept_loop(&shared)
        });
        Ok(HttpRemote {
            addr,
            shared,
            accept: Some(accept),
        })
    }

    /// The URL of the repository at `path` below the project root.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}/{}", self.addr, path.trim_start_matches('/'))
    }

    /// What the fixture saw and did, one line per event.
    pub fn log(&self) -> Vec<String> {
        self.shared.log.lock().expect("log lock").clone()
    }
}

impl Drop for HttpRemote {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(accept) = self.accept.take() {
            accept.join().ok();
        }
        let connections = std::mem::take(&mut *self.shared.connections.lock().expect("connections lock"));
        for (stream, thread) in connections {
            // Wakes reads, peeks and writes blocked on this socket.
            if let Some(stream) = stream.upgrade() {
                stream.shutdown(Shutdown::Both).ok();
            }
            thread.join().ok();
        }
    }
}

fn accept_loop(shared: &Arc<Shared>) {
    while !shared.stop.load(Ordering::SeqCst) {
        let accepted = {
            let listener = shared.listener.lock().expect("listener lock");
            match listener.as_ref() {
                Some(listener) => listener.accept(),
                None => return,
            }
        };
        match accepted {
            Ok((stream, _)) => {
                let stream = Arc::new(stream);
                let handle = Arc::downgrade(&stream);
                let thread = std::thread::spawn({
                    let shared = shared.clone();
                    move || {
                        if let Err(err) = serve_connection(&shared, &stream) {
                            shared.note(format!("connection error: {err}"));
                        }
                    }
                });
                shared
                    .connections
                    .lock()
                    .expect("connections lock")
                    .push((handle, thread));
            }
            Err(err) if err.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(5)),
            Err(err) => {
                shared.note(format!("accept error: {err}"));
                return;
            }
        }
    }
}

struct Request {
    method: String,
    target: String,
    headers: Vec<(String, String)>,
}

impl Request {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    fn is_receive_pack_post(&self) -> bool {
        self.method == "POST" && self.target.ends_with("/git-receive-pack")
    }

    fn is_upload_pack_post(&self) -> bool {
        self.method == "POST" && self.target.ends_with("/git-upload-pack")
    }

    /// The repository directory below `root` that this request addresses.
    fn repository(&self, root: &Path) -> PathBuf {
        let path = self.target.split('?').next().unwrap_or("");
        let repo = path.rsplit_once('/').map_or(path, |(repo, _service)| repo);
        root.join(repo.trim_start_matches('/'))
    }
}

fn early_answer(writer: &mut impl Write, code: u16) -> io::Result<()> {
    let body = b"refused before reading the request\n";
    let extra = if code == 401 {
        "WWW-Authenticate: Basic realm=\"fixture\"\r\n"
    } else {
        ""
    };
    write!(
        writer,
        "HTTP/1.1 {code} {}\r\n{extra}Content-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        reason(code),
        body.len()
    )?;
    writer.write_all(body)?;
    writer.flush()
}

/// Move `refs/heads/main` of the bare repository `repo` to a new commit on top of it.
fn move_main(repo: &Path) -> io::Result<String> {
    let run = |args: &[&str]| -> io::Result<String> {
        let out = Command::new("git")
            .arg("--git-dir")
            .arg(repo)
            .args(["-c", "user.name=fixture", "-c", "user.email=fixture@example.invalid"])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .output()?;
        if !out.status.success() {
            return Err(io::Error::other(String::from_utf8_lossy(&out.stderr).into_owned()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let tree = run(&["rev-parse", "refs/heads/main^{tree}"])?;
    let moved = run(&["commit-tree", &tree, "-p", "refs/heads/main", "-m", "competing writer"])?;
    run(&["update-ref", "refs/heads/main", &moved])?;
    Ok(moved)
}

fn serve_connection(shared: &Shared, stream: &TcpStream) -> io::Result<()> {
    stream.set_nonblocking(false)?;
    let mut writer = stream.try_clone()?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut served = 0usize;
    loop {
        if served > 0 && reader.buffer().is_empty() {
            if let Mode::DropIdle(limit) = shared.mode {
                let mut probe = [0u8; 1];
                stream.set_read_timeout(Some(limit))?;
                let early = stream.peek(&mut probe);
                stream.set_read_timeout(None)?;
                match early {
                    Ok(0) => return Ok(()),
                    Ok(_) => {}
                    Err(err) if matches!(err.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut) => {
                        // Forgotten. The client's next bytes stay unread, so closing resets.
                        let next = stream.peek(&mut probe)?;
                        if next > 0 {
                            shared.note("reset a connection reused after idling".into());
                        }
                        return Ok(());
                    }
                    Err(err) => return Err(err),
                }
            }
        }
        let Some(request) = read_head(&mut reader)? else {
            return Ok(());
        };
        served += 1;
        let framing = if request
            .header("Transfer-Encoding")
            .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
        {
            " (chunked)".to_owned()
        } else {
            request
                .header("Content-Length")
                .map(|length| format!(" (content-length {length})"))
                .unwrap_or_default()
        };
        shared.note(format!("{} {}{framing}", request.method, request.target));

        if request.is_receive_pack_post() {
            match shared.mode {
                Mode::EarlyStatus(code) => {
                    early_answer(&mut writer, code)?;
                    shared.note(format!("answered {code} before reading the body"));
                    // Drain what the client still sends so closing does not reset the answer away.
                    read_body(&mut reader, &request, &mut io::sink()).ok();
                    stream.shutdown(Shutdown::Both).ok();
                    return Ok(());
                }
                Mode::EarlyStatusReset(code) => {
                    early_answer(&mut writer, code)?;
                    shared.note(format!("answered {code}, closing with the body unread"));
                    // Give the answer a moment on the wire, then close with unread data: a reset.
                    shared.pause(Duration::from_millis(200));
                    return Ok(());
                }
                Mode::ResetAfter(limit) => {
                    let mut seen = vec![0u8; limit];
                    reader.read_exact(&mut seen)?;
                    shared.note(format!("read {limit} bytes of the upload, resetting"));
                    return Ok(());
                }
                _ => {}
            }
        }

        if request
            .header("Expect")
            .is_some_and(|value| value.eq_ignore_ascii_case("100-continue"))
        {
            writer.write_all(b"HTTP/1.1 100 Continue\r\n\r\n")?;
        }
        let mut body = Vec::new();
        read_body(&mut reader, &request, &mut body)?;
        match (shared.mode, request.method.as_str()) {
            (Mode::HoldResponse(hold), _) if request.is_receive_pack_post() => {
                shared.note(format!("holding the response for {}s", hold.as_secs()));
                if !shared.pause(hold) {
                    return Ok(());
                }
            }
            (Mode::HoldGet(hold), "GET") => {
                shared.note(format!("holding the GET for {}s", hold.as_secs()));
                if !shared.pause(hold) {
                    return Ok(());
                }
            }
            (Mode::MoveOnPost, _) if request.is_receive_pack_post() => {
                let moved = move_main(&request.repository(&shared.root))?;
                shared.note(format!("moved refs/heads/main to {moved} before applying"));
            }
            _ => {}
        }
        let (status, headers, payload) = http_backend(&shared.root, &request, &body)?;
        if matches!(shared.mode, Mode::LoseReport) && request.is_receive_pack_post() {
            stream.shutdown(Shutdown::Both).ok();
            shared.note(format!("applied the push ({status}) and closed without answering"));
            return Ok(());
        }
        let sent = match shared.mode {
            Mode::StallFetch(limit) if request.is_upload_pack_post() => limit.min(payload.len()),
            _ => payload.len(),
        };
        write!(writer, "HTTP/1.1 {status}\r\n")?;
        for header in &headers {
            write!(writer, "{header}\r\n")?;
        }
        write!(writer, "Content-Length: {}\r\n\r\n", payload.len())?;
        writer.write_all(&payload[..sent])?;
        writer.flush()?;
        if sent < payload.len() {
            shared.note(format!(
                "sent {sent} of {} bytes of the fetch response, stalling",
                payload.len()
            ));
            shared.pause(Duration::from_secs(3600));
            return Ok(());
        }
        shared.note(format!("{} {} -> {status}", request.method, request.target));

        if matches!(shared.mode, Mode::GoneAfterGet) && request.method == "GET" {
            // Close the listener before the connection, so any reconnect is refused.
            drop(shared.listener.lock().expect("listener lock").take());
            stream.shutdown(Shutdown::Both).ok();
            shared.note("closed the listener and the connection".into());
            return Ok(());
        }
    }
}

fn reason(code: u16) -> &'static str {
    match code {
        401 => "Unauthorized",
        413 => "Payload Too Large",
        500 => "Internal Server Error",
        _ => "Error",
    }
}

fn read_line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut line = String::new();
    if reader.read_line(&mut line)? == 0 {
        return Ok(None);
    }
    Ok(Some(line.trim_end_matches(['\r', '\n']).to_owned()))
}

fn read_head(reader: &mut impl BufRead) -> io::Result<Option<Request>> {
    let Some(line) = read_line(reader)? else {
        return Ok(None);
    };
    let mut parts = line.splitn(3, ' ');
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("bad request line {line:?}"),
        ));
    };
    let mut request = Request {
        method: method.to_owned(),
        target: target.to_owned(),
        headers: Vec::new(),
    };
    while let Some(line) = read_line(reader)? {
        if line.is_empty() {
            return Ok(Some(request));
        }
        if let Some((key, value)) = line.split_once(':') {
            request.headers.push((key.trim().to_owned(), value.trim().to_owned()));
        }
    }
    Err(io::ErrorKind::UnexpectedEof.into())
}

fn read_body(reader: &mut impl BufRead, request: &Request, out: &mut impl Write) -> io::Result<()> {
    if request
        .header("Transfer-Encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"))
    {
        loop {
            let size_line = read_line(reader)?.ok_or(io::ErrorKind::UnexpectedEof)?;
            let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            if size == 0 {
                while read_line(reader)?.is_some_and(|line| !line.is_empty()) {}
                return Ok(());
            }
            io::copy(&mut reader.take(size as u64), out)?;
            read_line(reader)?;
        }
    }
    let length = request
        .header("Content-Length")
        .map(str::parse::<u64>)
        .transpose()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?
        .unwrap_or(0);
    io::copy(&mut reader.take(length), out)?;
    Ok(())
}

/// Run one CGI request through `git http-backend`, killing it after [`BACKEND_LIMIT`];
/// returns the status line, headers and body.
fn http_backend(root: &Path, request: &Request, body: &[u8]) -> io::Result<(String, Vec<String>, Vec<u8>)> {
    let (path, query) = request.target.split_once('?').unwrap_or((&request.target, ""));
    let mut command = Command::new("git");
    command
        .arg("http-backend")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_PROJECT_ROOT", root)
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("PATH_INFO", path)
        .env("QUERY_STRING", query)
        .env("REQUEST_METHOD", &request.method)
        .env("CONTENT_TYPE", request.header("Content-Type").unwrap_or(""))
        .env("CONTENT_LENGTH", body.len().to_string())
        .env("REMOTE_USER", "fixture")
        .env("REMOTE_ADDR", "127.0.0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(protocol) = request.header("Git-Protocol") {
        command.env("GIT_PROTOCOL", protocol);
    }
    if let Some(encoding) = request.header("Content-Encoding") {
        command.env("HTTP_CONTENT_ENCODING", encoding);
    }
    let mut child = command.spawn()?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let input = body.to_owned();
    let feeder = std::thread::spawn(move || stdin.write_all(&input));
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            pipe.read_to_end(&mut buf).map(|_| buf)
        })
    };
    let stdout = drain(Box::new(child.stdout.take().expect("piped stdout")));
    let stderr = drain(Box::new(child.stderr.take().expect("piped stderr")));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > BACKEND_LIMIT {
            child.kill().ok();
            child.wait().ok();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "git http-backend ran past its limit and was killed",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    feeder.join().expect("stdin feeder").ok();
    let stdout = stdout.join().expect("stdout reader")?;
    let stderr = stderr.join().expect("stderr reader")?;
    if !status.success() && stdout.is_empty() {
        return Err(io::Error::other(format!(
            "git http-backend failed ({status}): {}",
            String::from_utf8_lossy(&stderr)
        )));
    }

    let split = stdout
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|at| (at, 4))
        .or_else(|| stdout.windows(2).position(|window| window == b"\n\n").map(|at| (at, 2)))
        .ok_or_else(|| {
            io::Error::other(format!(
                "git http-backend gave no CGI header: {}",
                String::from_utf8_lossy(&stderr)
            ))
        })?;
    let (head, payload) = (&stdout[..split.0], &stdout[split.0 + split.1..]);
    let mut status = "200 OK".to_owned();
    let mut headers = Vec::new();
    for line in String::from_utf8_lossy(head).lines() {
        match line.split_once(':') {
            Some((key, value)) if key.eq_ignore_ascii_case("Status") => status = value.trim().to_owned(),
            Some(_) => headers.push(line.trim_end().to_owned()),
            None => {}
        }
    }
    Ok((status, headers, payload.to_vec()))
}
