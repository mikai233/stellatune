use std::error::Error;
use std::io::{ErrorKind, stdin, stdout};

#[cfg(windows)]
use windows::Win32::System::Threading::{
    GetCurrentProcess, HIGH_PRIORITY_CLASS, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
    PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
    PROCESS_POWER_THROTTLING_STATE, ProcessPowerThrottling, SetPriorityClass,
    SetProcessInformation,
};

use stellatune_asio_proto::{ProtoError, Request, Response, read_frame, write_frame};

mod data_channel;
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

    configure_windows_audio_process();

    let data_ingress = match data_channel::DataIngressPump::from_env() {
        Ok(data_ingress) => data_ingress,
        Err(error) => {
            eprintln!("asio host data ingress unavailable: {error}");
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

#[cfg(windows)]
fn configure_windows_audio_process() {
    unsafe {
        let _ = SetPriorityClass(GetCurrentProcess(), HIGH_PRIORITY_CLASS);
        let state = PROCESS_POWER_THROTTLING_STATE {
            Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
            ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED
                | PROCESS_POWER_THROTTLING_IGNORE_TIMER_RESOLUTION,
            StateMask: 0,
        };
        let _ = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&state as *const PROCESS_POWER_THROTTLING_STATE).cast(),
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn configure_windows_audio_process() {}
