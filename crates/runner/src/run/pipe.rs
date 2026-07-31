use std::io::Read;
use std::sync::mpsc;
use std::thread;

pub(super) enum Chunk {
    Data(Stream, Vec<u8>),
    Done,
    Failed(Stream, String),
}

#[derive(Clone, Copy)]
pub(super) enum Stream {
    Out,
    Err,
}

impl Stream {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Out => "stdout",
            Self::Err => "stderr",
        }
    }
}

pub(super) fn spawn<R>(mut reader: R, stream: Stream, send: mpsc::Sender<Chunk>)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || copy(&mut reader, stream, &send));
}

fn copy(reader: &mut impl Read, stream: Stream, send: &mpsc::Sender<Chunk>) {
    let mut buffer = [0_u8; 8192];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => {
                let _ = send.send(Chunk::Done);
                return;
            }
            Ok(size)
                if send
                    .send(Chunk::Data(stream, buffer[..size].to_vec()))
                    .is_err() =>
            {
                return;
            }
            Ok(_) => {}
            Err(error) => {
                let _ = send.send(Chunk::Failed(stream, error.to_string()));
                return;
            }
        }
    }
}
