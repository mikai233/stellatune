//! Manual Windows integration check with silence and an isolated plugin directory.
//! cargo run -p stellatune-backend-api --example asio_smoke -- PLUGIN.zip "asio:USB DAC ASIO"
//! Append an upgraded plugin ZIP to also exercise installation during ASIO playback.
//! Set STELLATUNE_TEST_UNAVAILABLE_ASIO_DEVICE to exercise failed-open rollback.
use std::{io::Write, path::Path, sync::Arc, time::Duration};
use stellatune_audio::playback::{control::SwitchOptions, event::PlaybackState};
use stellatune_audio_builtin_adapters::factories::FileSourceFactory;
use stellatune_audio_core::{
    playback::{MediaTime, PlaybackItem, PlaybackItemId},
    source::MediaHints,
};
use stellatune_backend_api::runtime::{self, OutputBackend};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().collect();
    anyhow::ensure!(
        (3..=4).contains(&args.len()),
        "expected plugin ZIP, ASIO device ID and optional upgrade ZIP"
    );
    let directory = tempfile::tempdir()?;
    let result = run(
        Path::new(&args[1]),
        &args[2],
        directory.path(),
        args.get(3).map(Path::new),
    )
    .await;
    runtime::runtime_shutdown().await;
    result
}
async fn run(
    artifact: &Path,
    device: &str,
    directory: &Path,
    upgrade: Option<&Path>,
) -> anyhow::Result<()> {
    let manager = runtime::shared_plugin_manager(&directory.join("plugins"));
    let installed = manager.install(artifact).await?;
    let plugin_id = installed.id;
    anyhow::ensure!(
        runtime::native_output_types()
            .await
            .iter()
            .any(|t| t.plugin_id == plugin_id)
    );
    let targets = runtime::native_output_targets(plugin_id.clone(), "asio".into(), "{}".into())
        .await
        .map_err(anyhow::Error::msg)?;
    println!("Targets: {targets}");
    let target = serde_json::json!({"id": device}).to_string();
    runtime::runtime_set_output_sink_route(
        plugin_id.clone(),
        "asio".into(),
        "{}".into(),
        target.clone(),
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let file = directory.join("silence.wav");
    write_silence(&file)?;
    let player = runtime::shared_playback_controller();
    player
        .switch_to(
            PlaybackItem {
                id: PlaybackItemId::new(1).unwrap(),
                source: Arc::new(FileSourceFactory::new(file, MediaHints::default())?),
                required_decoder: None,
                segment: None,
            },
            SwitchOptions {
                autoplay: false,
                ..Default::default()
            },
        )
        .await?;
    player.play().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    if let Ok(unavailable) = std::env::var("STELLATUNE_TEST_UNAVAILABLE_ASIO_DEVICE") {
        wait_for_playback(&player).await?;
        let before = player.snapshot().await?;
        let result = runtime::runtime_set_output_sink_route(
            plugin_id.clone(),
            "asio".into(),
            "{}".into(),
            serde_json::json!({"id": unavailable}).to_string(),
        )
        .await;
        let error = result.expect_err("test device must fail to open");
        anyhow::ensure!(!error.contains("runtime is closed"), "{error}");
        wait_for_playback(&player).await?;
        let restored = player.snapshot().await?;
        anyhow::ensure!(restored.current_item_id == before.current_item_id);
        anyhow::ensure!(
            restored.consumed_position.as_millis() >= before.consumed_position.as_millis()
        );
        println!("Unavailable device rejected ({error}); previous ASIO output restored");
    }
    if let Some(upgrade) = upgrade {
        wait_for_playback(&player).await?;
        let installed = manager.install(upgrade).await?;
        anyhow::ensure!(installed.id == plugin_id);
        runtime::runtime_set_output_sink_route(
            plugin_id.clone(),
            "asio".into(),
            "{}".into(),
            target.clone(),
        )
        .await
        .map_err(anyhow::Error::msg)?;
        wait_for_playback(&player).await?;
        println!("Active ASIO plugin upgrade and driver reselection passed");
    }
    player.pause().await?;
    player.seek(MediaTime::from_millis(1000)).await?;
    runtime::runtime_set_output_device(OutputBackend::Shared, None)
        .await
        .map_err(anyhow::Error::msg)?;
    let snapshot = player.snapshot().await?;
    anyhow::ensure!(snapshot.state == PlaybackState::Paused);
    anyhow::ensure!(snapshot.consumed_position.as_millis().abs_diff(1000) < 10);
    runtime::runtime_set_output_sink_route(plugin_id.clone(), "asio".into(), "{}".into(), target)
        .await
        .map_err(anyhow::Error::msg)?;
    // Mutating the package while its native sink is open must release it first.
    manager.uninstall(&plugin_id).await?;
    anyhow::ensure!(runtime::native_output_types().await.is_empty());
    anyhow::ensure!(!directory.join("plugins").join(&plugin_id).exists());
    player.play().await?;
    tokio::time::sleep(Duration::from_millis(100)).await;
    anyhow::ensure!(player.snapshot().await?.consumed_position.as_millis() > 1000);
    player.stop().await?;
    println!("ASIO playback, pause, seek, system-output switch and active-plugin uninstall passed");
    Ok(())
}
async fn wait_for_playback(
    player: &stellatune_audio::playback::control::PlaybackController,
) -> anyhow::Result<()> {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let snapshot = player.snapshot().await?;
            anyhow::ensure!(
                snapshot.state != PlaybackState::Failed,
                "ASIO playback failed"
            );
            if snapshot.state == PlaybackState::Playing
                && snapshot.consumed_position.as_millis() > 0
            {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await??;
    Ok(())
}
fn write_silence(path: &Path) -> std::io::Result<()> {
    let length = 48_000u32 * 2 * 2 * 5;
    let mut file = std::fs::File::create(path)?;
    file.write_all(b"RIFF")?;
    file.write_all(&(length + 36).to_le_bytes())?;
    file.write_all(b"WAVEfmt ")?;
    file.write_all(&16u32.to_le_bytes())?;
    file.write_all(&1u16.to_le_bytes())?;
    file.write_all(&2u16.to_le_bytes())?;
    file.write_all(&48_000u32.to_le_bytes())?;
    file.write_all(&(48_000u32 * 4).to_le_bytes())?;
    file.write_all(&4u16.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?;
    file.write_all(b"data")?;
    file.write_all(&length.to_le_bytes())?;
    file.write_all(&vec![0; length as usize])
}
