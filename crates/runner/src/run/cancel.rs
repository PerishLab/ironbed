use std::sync::atomic::{AtomicBool, Ordering};
use std::{fs::File, io::Read, path::Path, thread};

static REQUESTED: AtomicBool = AtomicBool::new(false);

pub(super) struct Cancel {
    #[cfg(unix)]
    prior: libc::sighandler_t,
}

impl Cancel {
    pub(super) fn new(control: Option<&Path>) -> Result<Self, String> {
        REQUESTED.store(false, Ordering::SeqCst);
        if let Some(path) = control {
            if !path.is_absolute() || !path.exists() {
                return Err("cancel path must be one existing absolute path".to_string());
            }
            let input =
                File::open(path).map_err(|error| format!("cannot open cancel path: {error}"))?;
            thread::spawn(move || listen(input));
        }
        #[cfg(unix)]
        {
            let prior =
                unsafe { libc::signal(libc::SIGTERM, request as *const () as libc::sighandler_t) };
            if prior == libc::SIG_ERR {
                return Err(format!(
                    "cannot install cancellation signal: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(Self { prior })
        }
        #[cfg(not(unix))]
        {
            Ok(Self {})
        }
    }

    pub(super) fn requested(&self) -> bool {
        REQUESTED.load(Ordering::SeqCst)
    }
}

#[cfg(unix)]
impl Drop for Cancel {
    fn drop(&mut self) {
        unsafe {
            libc::signal(libc::SIGTERM, self.prior);
        }
    }
}

#[cfg(unix)]
extern "C" fn request(_: libc::c_int) {
    REQUESTED.store(true, Ordering::SeqCst);
}

fn listen(mut input: File) {
    let mut byte = [0_u8];
    if input.read_exact(&mut byte).is_ok() {
        REQUESTED.store(true, Ordering::SeqCst);
    }
}
