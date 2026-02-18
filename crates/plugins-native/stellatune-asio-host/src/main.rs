use std::error::Error;
use std::io::{ErrorKind, stdin, stdout};

#[cfg(windows)]
use windows::Win32::System::Threading::{GetCurrentProcess, HIGH_PRIORITY_CLASS, SetPriorityClass};

use stellatune_asio_proto::{ProtoError, Request, Response, read_frame, write_frame};

mod device;
mod request_handler;
mod state;
mod stream;

use request_handler::dispatch_request;
use state::RuntimeState;

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = stdin();
    let stdout = stdout();
    let mut r = stdin.lock();
    let mut w = stdout.lock();

    set_process_priority_class();

    let mut state = RuntimeState::default();

    loop {
        let req: Request = match read_frame(&mut r) {
            Ok(v) => v,
            Err(e) => {
                // EOF / broken pipe => exit.
                if matches!(e, ProtoError::Io(ref io) if io.kind() == ErrorKind::UnexpectedEof) {
                    break;
                }
                let _ = write_frame(
                    &mut w,
                    &Response::Err {
                        message: e.to_string(),
                    },
                );
                continue;
            },
        };

        if !dispatch_request(req, &mut state, &mut w)? {
            break;
        }
    }

    Ok(())
}

#[cfg(windows)]
fn set_process_priority_class() {
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
    }
}

#[cfg(not(windows))]
fn set_process_priority_class() {}
