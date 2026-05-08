//! PAM authentication thread.
//!
//! `pam` crate's `Authenticator` is created with our service file
//! (`/etc/pam.d/nullxes-lock`) and supplies the password via a
//! `PasswordConv` that returns the byte slice for `PAM_PROMPT_ECHO_OFF`.

use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::{spawn, JoinHandle};

use pam::client::conv_mock::PasswordConv;
use pam::client::Client;

pub struct AuthRequest {
    pub username: String,
    pub password: String,
    pub respond:  std::sync::mpsc::Sender<AuthResult>,
}

#[derive(Debug, Clone)]
pub enum AuthResult {
    Ok,
    Fail(String),
}

pub struct AuthWorker {
    pub tx:     Sender<AuthRequest>,
    pub _handle: JoinHandle<()>,
}

impl AuthWorker {
    pub fn spawn(service: &'static str) -> Self {
        let (tx, rx): (Sender<AuthRequest>, Receiver<AuthRequest>) = channel();
        let handle = spawn(move || worker_loop(rx, service));
        AuthWorker { tx, _handle: handle }
    }
}

fn worker_loop(rx: Receiver<AuthRequest>, service: &'static str) {
    while let Ok(req) = rx.recv() {
        let result = authenticate(service, &req.username, &req.password);
        let _ = req.respond.send(result);
    }
}

fn authenticate(service: &str, username: &str, password: &str) -> AuthResult {
    let mut auth = match Client::with_password(service) {
        Ok(a) => a,
        Err(e) => return AuthResult::Fail(format!("pam init: {e}")),
    };
    auth.conversation_mut().set_credentials(username, password);
    if let Err(e) = auth.authenticate() {
        return AuthResult::Fail(format!("authenticate: {e}"));
    }
    if let Err(e) = auth.open_session() {
        return AuthResult::Fail(format!("open_session: {e}"));
    }
    AuthResult::Ok
}

/// Resolves the user account NX-LOCK is running as via `getlogin_r` /
/// `getpwuid`. We never trust `$USER` since the lock screen's identity must
/// be the kernel's view of the session, not the environment.
pub fn current_username() -> String {
    use std::ffi::CStr;
    use std::mem::MaybeUninit;

    // Safety: getuid() returns the real uid; never fails.
    let uid = unsafe { libc::getuid() };

    let mut pwd: MaybeUninit<libc::passwd> = MaybeUninit::uninit();
    let mut buf = vec![0u8; 4096];
    let mut result: *mut libc::passwd = std::ptr::null_mut();
    // Safety: getpwuid_r writes into our buffers; on success returns 0 and
    // sets *result to point inside `pwd`.
    let rc = unsafe {
        libc::getpwuid_r(
            uid,
            pwd.as_mut_ptr(),
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len(),
            &mut result,
        )
    };
    if rc == 0 && !result.is_null() {
        // Safety: pw_name points into our `buf` allocation.
        let cstr = unsafe { CStr::from_ptr((*result).pw_name) };
        return cstr.to_string_lossy().to_string();
    }
    "root".to_string()
}
