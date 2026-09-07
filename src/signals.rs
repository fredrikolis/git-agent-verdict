// Concern: what a run says when something kills it, and whether its reviewer dies with it | Non-concern: why it was killed | IO: (signal) -> stderr, exit status

use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

static WORDS: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static LENGTH: AtomicUsize = AtomicUsize::new(0);
static REVIEWING: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
static JUDGING: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

static DELIBERATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub enum Posture {
    Caller,
    Round,
}

fn park(bytes: Vec<u8>) -> *mut u8 {
    Box::into_raw(bytes.into_boxed_slice()).cast()
}

// Async-signal-safe only: no alloc, no formatting, no lock — Rust's own printing can deadlock here.
extern "C" fn dying(signal: i32) {
    let named: &[u8] = match signal {
        libc::SIGTERM => b"git-agent-verdict: killed by SIGTERM",
        libc::SIGINT => b"git-agent-verdict: killed by SIGINT",
        libc::SIGHUP => b"git-agent-verdict: killed by SIGHUP",
        _ => b"git-agent-verdict: killed by a signal",
    };
    let say = |bytes: &[u8]| unsafe { libc::write(2, bytes.as_ptr().cast(), bytes.len()) };
    say(named);
    let words = WORDS.load(Ordering::Acquire);
    let length = LENGTH.load(Ordering::Relaxed);
    let round = !words.is_null() && length > 0;
    if round {
        say(b" ");
        unsafe { libc::write(2, words.cast(), length) };
    }
    let judging = JUDGING.load(Ordering::Relaxed);
    if judging > 0 {
        unsafe { libc::kill(-judging, libc::SIGKILL) };
    }
    let reviewing = REVIEWING.load(Ordering::Relaxed);
    let ended = (signal == libc::SIGINT || DELIBERATE.load(Ordering::Relaxed)) && reviewing > 0;
    if ended {
        unsafe { libc::kill(-reviewing, libc::SIGKILL) };
    }
    say(match (round, ended) {
        (_, true) => b" The reviewer was terminated with it.\n",
        (true, false) => b" The reviewer was not terminated; the review is resumable.\n",
        (false, false) => b"\n",
    });
    // Unblocked before re-raising: it's held off for the handler's duration, so raising it would else queue forever.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        let mut alone: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut alone);
        libc::sigaddset(&mut alone, signal);
        libc::sigprocmask(libc::SIG_UNBLOCK, &alone, std::ptr::null_mut());
        libc::raise(signal);
        libc::_exit(128 + signal);
    }
}

pub fn arm(posture: Posture) {
    DELIBERATE.store(matches!(posture, Posture::Round), Ordering::Relaxed);
    for signal in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
        let handler = dying as extern "C" fn(i32) as *const () as libc::sighandler_t;
        unsafe { libc::signal(signal, handler) };
    }
}

pub fn say(text: &str) {
    let bytes = text.as_bytes().to_vec();
    WORDS.store(std::ptr::null_mut(), Ordering::Release);
    LENGTH.store(bytes.len(), Ordering::Relaxed);
    WORDS.store(park(bytes), Ordering::Release);
}

pub fn quiet() {
    WORDS.store(std::ptr::null_mut(), Ordering::Release);
}

pub fn spawned(role: crate::agent::Role, pid: u32) {
    match role {
        crate::agent::Role::JudgeIntent => JUDGING.store(pid as i32, Ordering::Relaxed),
        crate::agent::Role::Review => REVIEWING.store(pid as i32, Ordering::Relaxed),
    }
}

// Cleared on reap: the kernel can reuse that pid, and a stale one would kill a stranger's group.
pub fn done() {
    REVIEWING.store(0, Ordering::Relaxed);
    JUDGING.store(0, Ordering::Relaxed);
}
