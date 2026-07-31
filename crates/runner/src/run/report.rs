use super::pipe::{Chunk, Stream};
use serde_json::{Value, json};
use std::io::Write;
use std::process::Child;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

pub(super) struct Drain {
    pub(super) sequence: u64,
    pub(super) out: u64,
    pub(super) err: u64,
    pub(super) failed: Option<String>,
    pub(super) expired: bool,
}

pub(super) struct Reporter<'a, W> {
    writer: &'a mut W,
    attempt: &'a str,
    sequence: u64,
    out: u64,
    err: u64,
}

impl<'a, W: Write> Reporter<'a, W> {
    pub(super) fn new(writer: &'a mut W, attempt: &'a str) -> Self {
        Self {
            writer,
            attempt,
            sequence: 1,
            out: 0,
            err: 0,
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
    ) -> Result<Drain, String> {
        let mut deadline = Some(Instant::now() + timeout);
        let mut expired = false;
        let mut done = 0_u8;
        let mut failed = None;
        while done < 2 {
            let Some(chunk) = next(&receive, deadline)? else {
                expired = expire(child)?;
                deadline = None;
                continue;
            };
            match chunk {
                Chunk::Data(stream, data) => self.output(stream, data)?,
                Chunk::Done => done += 1,
                Chunk::Failed(stream, error) => {
                    failed = Some(format!("cannot read {}: {error}", stream.name()));
                    done += 1;
                }
            }
        }
        Ok(Drain {
            sequence: self.sequence,
            out: self.out,
            err: self.err,
            failed,
            expired,
        })
    }

    fn output(&mut self, stream: Stream, bytes: Vec<u8>) -> Result<(), String> {
        let offset = match stream {
            Stream::Out => &mut self.out,
            Stream::Err => &mut self.err,
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
        Ok(())
    }
}

fn next(receive: &Receiver<Chunk>, deadline: Option<Instant>) -> Result<Option<Chunk>, String> {
    let Some(deadline) = deadline else {
        return receive.recv().map(Some).map_err(|error| error.to_string());
    };
    let remaining = deadline.saturating_duration_since(Instant::now());
    match receive.recv_timeout(remaining) {
        Ok(chunk) => Ok(Some(chunk)),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => Err("output readers disconnected".to_string()),
    }
}

fn expire(child: &mut Child) -> Result<bool, String> {
    match child
        .try_wait()
        .map_err(|error| format!("cannot inspect child at timeout: {error}"))?
    {
        Some(_) => Ok(false),
        None => child
            .kill()
            .map(|_| true)
            .map_err(|error| format!("cannot terminate child at timeout: {error}")),
    }
}
