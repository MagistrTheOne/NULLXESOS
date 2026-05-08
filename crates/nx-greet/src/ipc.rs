//! greetd IPC client — synchronous, simple. We use the `greetd_ipc` crate's
//! sync codec so the greeter can run on a single thread with the wayland
//! event loop without bringing in tokio.

use anyhow::{Context, Result};
use greetd_ipc::{
    codec::{Codec, SyncCodec},
    AuthMessageType, ErrorType, Request, Response,
};
use std::os::unix::net::UnixStream;

const SESSION_CMD: &[&str] = &["/usr/bin/frame"];

pub struct Greetd {
    stream: UnixStream,
}

impl Greetd {
    pub fn connect() -> Result<Self> {
        let path = std::env::var("GREETD_SOCK").context("GREETD_SOCK not set")?;
        let stream = UnixStream::connect(&path)
            .with_context(|| format!("connect greetd socket {path}"))?;
        Ok(Self { stream })
    }

    pub fn create_session(&mut self, username: &str) -> Result<Response> {
        Request::CreateSession { username: username.to_string() }
            .write_to(&mut self.stream)?;
        Ok(Response::read_from(&mut self.stream)?)
    }

    pub fn post_auth_response(&mut self, response: Option<String>) -> Result<Response> {
        Request::PostAuthMessageResponse { response }
            .write_to(&mut self.stream)?;
        Ok(Response::read_from(&mut self.stream)?)
    }

    pub fn start_session(&mut self) -> Result<Response> {
        let cmd: Vec<String> = SESSION_CMD.iter().map(|s| s.to_string()).collect();
        Request::StartSession { cmd, env: vec![] }
            .write_to(&mut self.stream)?;
        Ok(Response::read_from(&mut self.stream)?)
    }

    pub fn cancel(&mut self) -> Result<Response> {
        Request::CancelSession.write_to(&mut self.stream)?;
        Ok(Response::read_from(&mut self.stream)?)
    }
}

#[derive(Debug)]
pub enum FlowOutcome {
    Prompt(String, AuthMessageType),
    Success,
    Failure(String),
    Cancelled,
}

pub fn classify(resp: Response) -> FlowOutcome {
    match resp {
        Response::Success => FlowOutcome::Success,
        Response::AuthMessage { auth_message_type, auth_message } => {
            FlowOutcome::Prompt(auth_message, auth_message_type)
        }
        Response::Error { error_type, description } => {
            let kind = match error_type {
                ErrorType::AuthError => "auth",
                ErrorType::Error => "error",
            };
            FlowOutcome::Failure(format!("{kind}: {description}"))
        }
    }
}
