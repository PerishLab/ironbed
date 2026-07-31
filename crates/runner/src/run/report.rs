use super::{
    cancel::Cancel,
    child,
    pipe::{Chunk, Stream},
};
use serde_json::{Value, json};
use std::io::Write;
use std::process::Child;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

const TICK: Duration = Duration::from_millis(20);

pub(super) struct Drain {
    pub(super) sequence: u64,
    pub(super) kept: Count,
    pub(super) lost: Count,
    pub(super) limit: u64,
    pub(super) failed: Option<String>,
    pub(super) termination: Termination,
    pub(super) signalled: bool,
}

#[derive(Clone, Copy)]
pub(super) enum Termination {
    Process,
    Timeout,
    Limit,
    Cancel,
}

impl Termination {
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Process => "process",
            Self::Timeout => "timeout",
            Self::Limit => "output_limit",
            Self::Cancel => "cancelled",
        }
    }
}

#[derive(Clone, Copy, Default)]
pub(super) struct Count {
    pub(super) out: u64,
    pub(super) err: u64,
}

struct State {
    deadline: Option<Instant>,
    termination: Termination,
    signalled: bool,
}

impl State {
    fn new(timeout: Duration) -> Self {
        Self {
            deadline: Some(Instant::now() + timeout),
            termination: Termination::Process,
            signalled: false,
        }
    }

    fn expire(&mut self, child: &mut Child) -> Result<(), String> {
        self.signalled = child::stop(child)?;
        self.termination = Termination::Timeout;
        self.deadline = None;
        Ok(())
    }

    fn limit(&mut self, child: &mut Child, crossed: bool) -> Result<(), String> {
        if crossed && matches!(self.termination, Termination::Process) {
            self.signalled = child::stop(child)?;
            self.termination = Termination::Limit;
            self.deadline = None;
        }
        Ok(())
    }

    fn cancel(&mut self, child: &mut Child) -> Result<(), String> {
        self.signalled = child::stop(child)?;
        self.termination = Termination::Cancel;
        self.deadline = None;
        Ok(())
    }
}

pub(super) struct Reporter<'a, W> {
    writer: &'a mut W,
    attempt: &'a str,
    sequence: u64,
    kept: Count,
    lost: Count,
    limit: u64,
    cancel: Cancel,
}

impl<'a, W: Write> Reporter<'a, W> {
    pub(super) fn new(writer: &'a mut W, attempt: &'a str, cancel: Cancel) -> Self {
        Self {
            writer,
            attempt,
            sequence: 1,
            kept: Count::default(),
            lost: Count::default(),
            limit: 0,
            cancel,
        }
    }

    pub(super) fn emit(&mut self, value: &Value) -> Result<(), String> {
        serde_json::to_writer(&mut *self.writer, value).map_err(|error| error.to_string())?;
        self.writer
            .write_all(b"\n")
            .map_err(|error| error.to_string())?;
        self.writer.flush().map_err(|error| error.to_string())
    }

    pub(super) fn drain(
        &mut self,
        receive: Receiver<Chunk>,
        child: &mut Child,
        timeout: Duration,
        limit: u64,
    ) -> Result<Drain, String> {
        self.limit = limit;
        let mut state = State::new(timeout);
        let mut done = 0_u8;
        let mut failed = None;
        while done < 2 {
            let cancel = matches!(state.termination, Termination::Process).then_some(&self.cancel);
            match next(&receive, state.deadline, cancel)? {
                Wake::Expire => state.expire(child)?,
                Wake::Cancel => state.cancel(child)?,
                Wake::Chunk(Chunk::Data(stream, data)) => {
                    let crossed = self.output(stream, data)?;
                    state.limit(child, crossed)?;
                }
                Wake::Chunk(Chunk::Done) => done += 1,
                Wake::Chunk(Chunk::Failed(stream, error)) => {
                    failed = Some(format!("cannot read {}: {error}", stream.name()));
                    done += 1;
                }
            }
        }
        Ok(Drain {
            sequence: self.sequence,
            kept: self.kept,
            lost: self.lost,
            limit,
            failed,
            termination: state.termination,
            signalled: state.signalled,
        })
    }

    fn output(&mut self, stream: Stream, mut bytes: Vec<u8>) -> Result<bool, String> {
        let retained = self.kept.out + self.kept.err;
        let remaining = self.limit.saturating_sub(retained);
        let discarded = bytes.len().saturating_sub(remaining as usize);
        if discarded > 0 {
            bytes.truncate(remaining as usize);
            match stream {
                Stream::Out => self.lost.out += discarded as u64,
                Stream::Err => self.lost.err += discarded as u64,
            }
        }
        if bytes.is_empty() {
            return Ok(discarded > 0);
        }
        let offset = match stream {
            Stream::Out => &mut self.kept.out,
            Stream::Err => &mut self.kept.err,
        };
        let prior = *offset;
        *offset += bytes.len() as u64;
        let frame = json!({
            "schema": "ironbed.frame/v0",
            "attempt": self.attempt,
            "sequence": self.sequence,
            "kind": "output",
            "stream": stream.name(),
            "offset": prior,
            "bytes": bytes
        });
        self.emit(&frame)?;
        self.sequence += 1;
        Ok(discarded > 0)
    }
}

enum Wake {
    Chunk(Chunk),
    Expire,
    Cancel,
}

fn next(
    receive: &Receiver<Chunk>,
    deadline: Option<Instant>,
    cancel: Option<&Cancel>,
) -> Result<Wake, String> {
    loop {
        if cancel.is_some_and(Cancel::requested) {
            return Ok(Wake::Cancel);
        }
        let wait = match (deadline, cancel) {
            (None, None) => {
                return receive
                    .recv()
                    .map(Wake::Chunk)
                    .map_err(|error| error.to_string());
            }
            (None, Some(_)) => TICK,
            (Some(deadline), _) => deadline.saturating_duration_since(Instant::now()).min(TICK),
        };
        match receive.recv_timeout(wait) {
            Ok(chunk) => return Ok(Wake::Chunk(chunk)),
            Err(RecvTimeoutError::Timeout)
                if deadline.is_some_and(|value| Instant::now() >= value) =>
            {
                return Ok(Wake::Expire);
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                return Err("output readers disconnected".to_string());
            }
        }
    }
}
