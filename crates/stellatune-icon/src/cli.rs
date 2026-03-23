use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};

use crate::{
    BackgroundPreset, DEFAULT_EXPORT_SIZE, ExportMask, IconLayer, PixelSize, RenderRequest,
    write_png,
};

#[derive(Debug, Parser)]
#[command(name = "stellatune-icon")]
#[command(about = "Render layered Stellatune application icons with Skia")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Render(args) => args.run(),
            Command::RenderBundle(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Render one layer or the composed icon to a single PNG.
    Render(RenderArgs),
    /// Render both background and foreground layers into an output directory.
    RenderBundle(RenderBundleArgs),
}

#[derive(Debug, Args)]
pub struct RenderArgs {
    /// Background palette preset.
    #[arg(long, value_enum, default_value_t = BackgroundPreset::Teal)]
    pub preset: BackgroundPreset,

    /// Export mask shape.
    #[arg(long, value_enum, default_value_t = ExportMask::Square)]
    pub mask: ExportMask,

    /// Which layer to render.
    #[arg(long, value_enum, default_value_t = IconLayer::Composite)]
    pub layer: IconLayer,

    /// Output width in pixels.
    #[arg(long, default_value_t = DEFAULT_EXPORT_SIZE)]
    pub width: i32,

    /// Output height in pixels.
    #[arg(long, default_value_t = DEFAULT_EXPORT_SIZE)]
    pub height: i32,

    /// Output PNG path.
    #[arg(long)]
    pub output: PathBuf,
}

impl RenderArgs {
    fn run(self) -> Result<()> {
        let size = PixelSize::new(self.width, self.height)?;
        let request = RenderRequest::with_style(self.layer, size, self.preset, self.mask);
        write_png(&request, self.output)
    }
}

#[derive(Debug, Args)]
pub struct RenderBundleArgs {
    /// Background palette preset.
    #[arg(long, value_enum, default_value_t = BackgroundPreset::Teal)]
    pub preset: BackgroundPreset,

    /// Square export size in pixels.
    #[arg(long, default_value_t = DEFAULT_EXPORT_SIZE)]
    pub size: i32,

    /// Output directory for layered assets.
    #[arg(long)]
    pub output_dir: PathBuf,
}

impl RenderBundleArgs {
    fn run(self) -> Result<()> {
        let size = PixelSize::square(self.size)?;
        write_png(
            &RenderRequest::with_background_preset(IconLayer::Background, size, self.preset),
            self.output_dir.join("background.png"),
        )?;
        write_png(
            &RenderRequest::with_background_preset(IconLayer::Foreground, size, self.preset),
            self.output_dir.join("foreground.png"),
        )?;
        Ok(())
    }
}
