use std::error::Error;
use std::io::{ErrorKind, stdin, stdout};

use stellatune_asio_proto::{ProtoError, Request, Response, read_frame, write_frame};
use stellatune_sidecar_support::logging::init_daily_file_tracing_from_env;

mod data_channel;
mod device;
mod platform;
mod request_handler;
mod state;
mod stream;

use crate::platform::main::configure_audio_process;
use request_handler::dispatch_request;
use state::RuntimeState;

fn main() -> Result<(), Box<dyn Error>> {
    init_daily_file_tracing_from_env()?;

    let stdin = stdin();
    let stdout = stdout();
    let mut r = stdin.lock();
    let mut w = stdout.lock();

    configure_audio_process();

    let data_ingress = match data_channel::DataIngressPump::from_env() {
        Ok(data_ingress) => data_ingress,
        Err(error) => {
            tracing::warn!("asio host data ingress unavailable: {error}");
            None
        },
    };
    let mut state = RuntimeState::new(data_ingress);

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
