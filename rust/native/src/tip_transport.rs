// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
//! Deadline-bound TIP subprocesses. No engine locks or document COM cross into
//! the caller. Job objects terminate worker descendants before reconnecting.
use super::{allocate, read, NEXT};
use std::{
    collections::HashMap,
    ffi::{c_char, c_void},
    io::{self, Read, Write},
    os::windows::{io::AsRawHandle, process::CommandExt},
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
        Arc, Mutex, OnceLock,
    },
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    },
};
const MAX_REQUEST: usize = 65536;
const MAX_RESPONSE: usize = 1024 * 1024;
struct Job(HANDLE);
unsafe impl Send for Job {}
unsafe impl Sync for Job {}
impl Job {
    fn new(child: &Child) -> io::Result<Self> {
        unsafe {
            let job = Self(CreateJobObjectW(None, PCWSTR::null()).map_err(io::Error::other)?);
            let info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
                BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION {
                    LimitFlags: JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
                    ..Default::default()
                },
                ..Default::default()
            };
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&info) as u32,
            )
            .map_err(io::Error::other)?;
            AssignProcessToJobObject(job.0, HANDLE(child.as_raw_handle()))
                .map_err(io::Error::other)?;
            Ok(job)
        }
    }
    fn terminate(&self) {
        unsafe {
            let _ = TerminateJobObject(self.0, 1);
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
#[derive(Default)]
struct Cancel {
    cancelled: AtomicBool,
    job: Mutex<Option<Arc<Job>>>,
}
impl Cancel {
    fn stop(&self) {
        self.cancelled.store(true, Ordering::Release);
        let job = self.job.lock().ok().and_then(|j| j.clone());
        if let Some(job) = job {
            job.terminate();
        }
    }
    fn attach(&self, job: Arc<Job>) {
        if let Ok(mut slot) = self.job.lock() {
            *slot = Some(job.clone());
        }
        if self.cancelled.load(Ordering::Acquire) {
            job.terminate();
        }
    }
}
struct Request {
    text: String,
    deadline: Instant,
    cancel: Arc<Cancel>,
    reply: SyncSender<io::Result<String>>,
}
struct Worker {
    child: Child,
    input: ChildStdin,
    output: Receiver<io::Result<String>>,
    job: Arc<Job>,
    last: Option<Arc<Cancel>>,
}
impl Worker {
    fn start(path: &PathBuf, cancel: &Cancel) -> io::Result<Self> {
        let mut child = Command::new(path)
            .arg("--stdio-tip-worker")
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let job = match Job::new(&child) {
            Ok(job) => Arc::new(job),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
        };
        cancel.attach(job.clone());
        let input = child
            .stdin
            .take()
            .ok_or_else(|| io::Error::other("worker stdin missing"))?;
        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("worker stdout missing"))?;
        let (sender, output) = mpsc::sync_channel(1);
        std::thread::spawn(move || loop {
            let result = (|| {
                let mut size = [0; 4];
                stdout.read_exact(&mut size)?;
                let size = u32::from_le_bytes(size) as usize;
                if size == 0 || size > MAX_RESPONSE {
                    return Err(io::Error::other("invalid worker response size"));
                }
                let mut bytes = vec![0; size];
                stdout.read_exact(&mut bytes)?;
                String::from_utf8(bytes).map_err(io::Error::other)
            })();
            let failed = result.is_err();
            if sender.send(result).is_err() || failed {
                break;
            }
        });
        Ok(Self {
            child,
            input,
            output,
            job,
            last: None,
        })
    }
    fn request(&mut self, r: &Request) -> io::Result<String> {
        self.last = Some(r.cancel.clone());
        r.cancel.attach(self.job.clone());
        if r.cancel.cancelled.load(Ordering::Acquire) || Instant::now() >= r.deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "request expired"));
        }
        // These writes run on the supervisor, never the TSF thread. The caller
        // can terminate the job even when the child stops reading its stdin.
        self.input.write_all(&(r.text.len() as u32).to_le_bytes())?;
        self.input.write_all(r.text.as_bytes())?;
        self.input.flush()?;
        self.output
            .recv_timeout(r.deadline.saturating_duration_since(Instant::now()))
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "worker response timed out"))?
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.job.terminate();
        let _ = self.child.wait();
    }
}
struct Transport {
    sender: SyncSender<Request>,
    gate: Mutex<()>,
    active: Mutex<Option<Arc<Cancel>>>,
    retry: Mutex<Option<Instant>>,
    closed: AtomicBool,
}
impl Transport {
    fn start(path: PathBuf) -> Arc<Self> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(1);
        std::thread::spawn(move || {
            let mut worker: Option<Worker> = None;
            while let Ok(r) = receiver.recv() {
                if r.cancel.cancelled.load(Ordering::Acquire) || Instant::now() >= r.deadline {
                    continue;
                }
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if worker.as_mut().is_some_and(|w| {
                        w.last
                            .as_ref()
                            .is_some_and(|c| c.cancelled.load(Ordering::Acquire))
                            || w.child.try_wait().ok().flatten().is_some()
                    }) {
                        worker = None;
                    }
                    if worker.is_none() {
                        worker = Some(Worker::start(&path, &r.cancel)?);
                    }
                    worker.as_mut().unwrap().request(&r)
                }))
                .unwrap_or_else(|_| Err(io::Error::other("worker supervisor failed")));
                if result.is_err() || r.cancel.cancelled.load(Ordering::Acquire) {
                    worker = None;
                }
                // A response racing timeout belongs only to this private channel.
                // It can never be mistaken for the next request's response.
                let _ = r.reply.try_send(result);
            }
        });
        Arc::new(Self {
            sender,
            gate: Mutex::new(()),
            active: Mutex::new(None),
            retry: Mutex::new(None),
            closed: AtomicBool::new(false),
        })
    }
    fn stop(&self) {
        self.closed.store(true, Ordering::Release);
        if let Some(cancel) = self.active.lock().ok().and_then(|a| a.clone()) {
            cancel.stop();
        }
    }
    fn request(&self, text: String, timeout: u32, optional: bool) -> Option<String> {
        let _gate = self.gate.try_lock().ok()?;
        if self.closed.load(Ordering::Acquire)
            || self.retry.lock().ok()?.is_some_and(|t| Instant::now() < t)
        {
            return None;
        }
        if timeout == 0 {
            if !optional {
                *self.retry.lock().ok()? = Some(Instant::now() + Duration::from_secs(3));
            }
            return None;
        }
        let deadline = Instant::now() + Duration::from_millis(timeout as u64);
        let cancel = Arc::new(Cancel::default());
        *self.active.lock().ok()? = Some(cancel.clone());
        // stop() can race publication of the active cancellation token.
        if self.closed.load(Ordering::Acquire) {
            cancel.stop();
            return None;
        }
        let (reply, receiver) = mpsc::sync_channel(1);
        if self
            .sender
            .try_send(Request {
                text,
                deadline,
                cancel: cancel.clone(),
                reply,
            })
            .is_err()
        {
            return None;
        }
        let response = receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()));
        let result = match response {
            Ok(Ok(text)) if Instant::now() < deadline => Some(text),
            _ => None,
        };
        if result.is_none() {
            cancel.stop();
            if !optional {
                if let Ok(mut retry) = self.retry.lock() {
                    *retry = Some(Instant::now() + Duration::from_secs(3));
                }
            }
        }
        // Close must still terminate an idle worker, so keep its job token here.
        result
    }
}
impl Drop for Transport {
    fn drop(&mut self) {
        self.stop();
    }
}
fn transports() -> &'static Mutex<HashMap<usize, Arc<Transport>>> {
    static ALL: OnceLock<Mutex<HashMap<usize, Arc<Transport>>>> = OnceLock::new();
    ALL.get_or_init(|| Mutex::new(HashMap::new()))
}
/// Create a lazy transport for an absolute, trusted sibling worker executable.
/// # Safety
/// path is borrowed NUL-terminated UTF-8. No process starts until the first request.
#[no_mangle]
pub unsafe extern "C" fn meltype_tip_transport_create(path: *const c_char) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        let path = PathBuf::from(read(path)?);
        if !path.is_absolute() || !path.is_file() {
            return None;
        }
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        let transport = Transport::start(path);
        transports().lock().ok()?.insert(id, transport);
        Some(id as *mut c_void)
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}
#[no_mangle]
pub extern "C" fn meltype_tip_transport_destroy(handle: *mut c_void) {
    let transport = transports()
        .lock()
        .ok()
        .and_then(|mut all| all.remove(&(handle as usize)));
    if let Some(transport) = transport {
        transport.stop();
    }
}
/// Returned UTF-8 uses meltype_free. Timeout/failure closes the worker tree and
/// discards its sessions; mandatory failures back off 3s, optional failures do not.
/// # Safety
/// request is borrowed NUL-terminated UTF-8, <=64 KiB. Concurrent calls to one
/// connection fail immediately; separate connections have independent deadlines.
#[no_mangle]
pub unsafe extern "C" fn meltype_tip_transport_request(
    handle: *mut c_void,
    request: *const c_char,
    timeout: u32,
    optional: i32,
) -> *mut c_char {
    catch_unwind(AssertUnwindSafe(|| {
        let text = read(request)?;
        if text.len() > MAX_REQUEST {
            return None;
        }
        let transport = transports().lock().ok()?.get(&(handle as usize)).cloned()?;
        transport
            .request(text, timeout, optional != 0)
            .map(|text| allocate(&text))
    }))
    .ok()
    .flatten()
    .unwrap_or(std::ptr::null_mut())
}
