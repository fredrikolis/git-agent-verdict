// Concern: what a run says when something kills it, and whether its reviewer dies with it | Non-concern: why it was killed | IO: (signal) -> stderr, exit status

use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

// A run reaped by the shell holding it writes nothing on its way out, and the kill is then indistinguishable from the reviewer having stopped. These hold what to say and what to drop, rendered while there is still a formatter to render them: a handler may call almost nothing, so everything it needs is prepared before the signal arrives.
static WORDS: AtomicPtr<u8> = AtomicPtr::new(std::ptr::null_mut());
static LENGTH: AtomicUsize = AtomicUsize::new(0);

// The reviewer's process group, ended by an interrupt and left alone by a reaper: a person at a terminal is not coming back for that review, while a reaper's signal leaves a review that holds the claim and a round the next run takes up.
static REVIEWING: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

// The judge's, ended by any of them: it holds nothing, its one answer is read by this run alone, and left behind it would spend a ceiling nobody is watching.
static JUDGING: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

// A round process answers to nobody at a terminal, so any signal it receives was sent on purpose and ends its reviewer with it. A caller's reaper is not on purpose, and leaves the round alone.
static DELIBERATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub enum Posture {
    Caller,
    Round,
}

// Leaked deliberately: the handler reads these when nothing may be freed, and one run sets each a handful of times.
fn park(bytes: Vec<u8>) -> *mut u8 {
    Box::into_raw(bytes.into_boxed_slice()).cast()
}

// Everything here is async-signal-safe and nothing else may be: no allocation, no formatting, no lock. Rust's own printing is none of those and would deadlock against a thread already inside it. Every sentence is a fixed buffer or one rendered before the signal arrived, and the joining between them is done here rather than by the caller.
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
    // Killed here rather than left: a judge answers one question for this run alone, and a reviewer is worth keeping unless the person waiting for it has just said otherwise.
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
    // Died rather than exited, because a supervisor tells the two apart and this run was killed. Unblocked first: the signal being handled is held off for as long as the handler runs, so raising it without this only queues one that never arrives. The status is the fallback for a signal that somehow does not land.
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
        // Through a pointer rather than straight to an integer: the cast clippy refuses is the one that silently takes the wrong thing when a signature moves.
        let handler = dying as extern "C" fn(i32) as *const () as libc::sighandler_t;
        unsafe { libc::signal(signal, handler) };
    }
}

// What this run will say if it is killed from here on. Rendered now, written verbatim then, and never ended: the handler joins it to what comes before and after, and a line broken here breaks the sentence in two.
pub fn say(text: &str) {
    let bytes = text.as_bytes().to_vec();
    // Published last and read first, so a signal arriving mid-update finds the whole of one sentence or none, never a length describing another buffer.
    WORDS.store(std::ptr::null_mut(), Ordering::Release);
    LENGTH.store(bytes.len(), Ordering::Relaxed);
    WORDS.store(park(bytes), Ordering::Release);
}

// Nothing is under review, so a kill from here on has nothing to say about a round. The groups are not this function's: they are cleared where their leader is reaped.
pub fn quiet() {
    WORDS.store(std::ptr::null_mut(), Ordering::Release);
}

// The group an agent leads, which a signal ends or spares by what the agent was asked for.
pub fn spawned(role: crate::agent::Role, pid: u32) {
    match role {
        crate::agent::Role::JudgeIntent => JUDGING.store(pid as i32, Ordering::Relaxed),
        crate::agent::Role::Review => REVIEWING.store(pid as i32, Ordering::Relaxed),
    }
}

// Cleared the moment the leader is reaped, because a reaped leader frees its pid and the kernel gives that number to somebody else. Held past that, a signal would end a group this run never started.
pub fn done() {
    REVIEWING.store(0, Ordering::Relaxed);
    JUDGING.store(0, Ordering::Relaxed);
}
