use stellatune_plugins::{PluginManager, default_host_vtable};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let plugin_dir = args.next().unwrap_or_else(|| "plugins".to_string());
    let decode_path = args.next();

    let mut mgr = PluginManager::new(default_host_vtable());
    let report = unsafe { mgr.load_dir(&plugin_dir)? };

    if !report.errors.is_empty() {
        eprintln!("Load errors:");
        for e in report.errors {
            eprintln!("  - {e:#}");
        }
    }

    println!("Loaded plugins:");
    for p in &report.loaded {
        println!("  - {} ({})", p.id, p.name);
    }

    println!("DSP types:");
    let dsps = mgr.list_dsp_types();
    for t in &dsps {
        println!("  - {}::{} ({})", t.plugin_id, t.type_id, t.display_name);
    }

    println!("Decoder types:");
    for t in mgr.list_decoder_types() {
        println!("  - {}::{}", t.plugin_id, t.type_id);
    }

    if let Some(first) = dsps.first() {
        let mut dsp = mgr.create_dsp(first.key, 48_000, 2, r#"{ "gain": 2.0 }"#)?;
        let mut samples = vec![0.25f32, -0.25f32, 0.5f32, -0.5f32]; // 2 frames, stereo
        dsp.process_in_place(&mut samples, 2);
        println!("DSP smoke output: {samples:?}");
    }

    // Decoder smoke: either decode the provided path, or generate a tiny `.tone` file.
    let path = if let Some(p) = decode_path {
        std::path::PathBuf::from(p)
    } else {
        let tone_path = std::env::temp_dir().join("stellatune_example.tone");
        write_example_tone(&tone_path)?;
        tone_path
    };

    if let Some(mut dec) = mgr.open_best_decoder(path.to_string_lossy().as_ref())? {
        let spec = dec.spec();
        let duration = dec.duration_ms();
        let meta = dec.metadata_json().unwrap_or(None);
        let (samples, _eof) = dec.read_interleaved_f32(4)?;
        println!(
            "Decoder smoke: {}Hz ch={} duration_ms={:?} meta={:?} samples={:?}",
            spec.sample_rate, spec.channels, duration, meta, samples
        );
    } else {
        println!("Decoder smoke: no decoder selected for {}", path.display());
    }

    Ok(())
}

fn write_example_tone(path: &std::path::Path) -> anyhow::Result<()> {
    use std::io::Write;

    let mut f = std::fs::File::create(path)?;
    f.write_all(b"STTN")?;
    f.write_all(&48_000u32.to_le_bytes())?;
    f.write_all(&2u16.to_le_bytes())?;
    f.write_all(&0u16.to_le_bytes())?;

    // 4 frames stereo = 8 samples.
    let samples = [0.1f32, -0.1, 0.2, -0.2, 0.3, -0.3, 0.4, -0.4];
    for s in samples {
        f.write_all(&s.to_le_bytes())?;
    }
    Ok(())
}
