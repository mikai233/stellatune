//! cargo run -p stellatune-audio-asio-adapter --example probe -- HOST.exe [DEVICE_ID] [RATE]
//! Lists drivers, or opens the selected one and validates clocks with silence.
use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};
use stellatune_audio_asio_adapter::{AsioConfig, AsioSinkFactory, list_devices};
use stellatune_audio_core::{format::AudioBlock, sink::SinkFactory};

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().collect();
    let executable = PathBuf::from(args.get(1).ok_or("expected host executable")?);
    let devices = list_devices(&executable)?;
    for device in &devices {
        println!("{}: {}", device.id, device.name);
    }
    let Some(device) = args.get(2) else {
        return Ok(());
    };
    let sample_rate = args
        .get(3)
        .map(|rate| rate.parse())
        .transpose()
        .map_err(|e| format!("invalid rate: {e}"))?;
    let factory = AsioSinkFactory::discover(
        executable,
        device.clone(),
        AsioConfig {
            sample_rate,
            ..Default::default()
        },
        1,
    )?;
    let format = factory.format();
    println!("Opening {format:?}");
    let mut sink = factory.create().map_err(|e| e.to_string())?;
    sink.open(format).map_err(|e| e.to_string())?;
    let mut block = AudioBlock::new(format);
    block.samples = vec![
        0.0;
        (format.sample_rate / 100) as usize
            * usize::from(format.channel_layout.channel_count())
    ];
    sink.resume().map_err(|e| e.to_string())?;
    let start = Instant::now();
    let mut accepted = 0;
    while start.elapsed() < Duration::from_millis(500) {
        accepted += sink
            .write(&block)
            .map_err(|e| e.to_string())?
            .consumed_frames;
        thread::sleep(Duration::from_millis(2));
    }
    sink.drain().map_err(|e| e.to_string())?;
    let clock = sink.clock_snapshot();
    assert_eq!(clock.consumed_frames, accepted as u64);
    println!("Drained {clock:?}");
    sink.pause().map_err(|e| e.to_string())?;
    sink.write(&block).map_err(|e| e.to_string())?;
    sink.discard().map_err(|e| e.to_string())?;
    assert_eq!(sink.clock_snapshot().buffered_frames, 0);
    assert_eq!(sink.clock_snapshot().consumed_frames, 0);
    sink.close();
    println!("Pause, discard and close succeeded");
    Ok(())
}
