mod output;

use anyhow::{Context, Result, bail};
use clap::Parser;
use spritefusion_sprite_normals::{NormalFormat, NormalSettings, media, process_image};
use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

/// Generate normal maps from PNG sprites, sprite sheets, and GIF/APNG animations.
#[derive(Debug, Parser)]
#[command(name = "spritefusion-sprite-normals", version)]
struct Args {
    /// Input PNG, GIF, or APNG files.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    /// Output normal PNG (single input only).
    #[arg(short, long, conflicts_with = "output_dir")]
    output: Option<PathBuf>,
    /// Write <input-stem>_n.png here. Defaults to beside each input.
    #[arg(long)]
    output_dir: Option<PathBuf>,
    /// Replace existing outputs.
    #[arg(long)]
    force: bool,
    /// Isolate PNG sprite-sheet cells, for example 32x32.
    #[arg(long, value_parser = parse_frame_size)]
    frame_size: Option<(u32, u32)>,
    /// Final normal strength (0..8). Default: 1.5.
    #[arg(long)]
    normal_strength: Option<f32>,
    /// Painted-shading relief (0..8). Default: 6.
    #[arg(long)]
    detail_strength: Option<f32>,
    /// Shading smoothing in pixels (0.6..8). Default: 0.6.
    #[arg(long)]
    detail_scale: Option<f32>,
    /// Silhouette volume (0..8). Default: 1.15.
    #[arg(long)]
    shape_strength: Option<f32>,
    /// Silhouette falloff reach in pixels (0.5..512). Default: 4.
    #[arg(long)]
    shape_radius: Option<f32>,
    /// Silhouette smoothing in pixels (0..8). Default: 1.15.
    #[arg(long)]
    shape_softness: Option<f32>,
    /// Opacity cutoff for geometry (0..0.99). Default: 0.03.
    #[arg(long)]
    alpha_threshold: Option<f32>,
    /// DirectX reverses the green channel.
    #[arg(long, value_enum, default_value_t = NormalFormat::Opengl)]
    normal_format: NormalFormat,
    /// Reverse the red channel direction.
    #[arg(long)]
    invert_x: bool,
    /// Reverse the green channel direction.
    #[arg(long)]
    invert_y: bool,
}

fn parse_frame_size(value: &str) -> Result<(u32, u32), String> {
    let (w, h) = value
        .split_once('x')
        .ok_or("expected WIDTHxHEIGHT, for example 32x32")?;
    let w = w.parse::<u32>().map_err(|_| "invalid frame width")?;
    let h = h.parse::<u32>().map_err(|_| "invalid frame height")?;
    if w == 0 || h == 0 {
        return Err("frame dimensions must be positive".into());
    }
    Ok((w, h))
}

impl Args {
    fn settings(&self) -> NormalSettings {
        let mut settings = NormalSettings {
            frame_size: self.frame_size,
            normal_format: self.normal_format,
            invert_x: self.invert_x,
            invert_y: self.invert_y,
            ..NormalSettings::default()
        };
        macro_rules! overrides {
            ($($name:ident),*) => { $(if let Some(value) = self.$name { settings.$name = value; })* };
        }
        overrides!(
            normal_strength,
            detail_strength,
            detail_scale,
            shape_strength,
            shape_radius,
            shape_softness,
            alpha_threshold
        );
        settings
    }
}

struct Job {
    input: PathBuf,
    output: PathBuf,
}

fn prepare_jobs(args: &Args) -> Result<Vec<Job>> {
    if args.output.is_some() && args.inputs.len() != 1 {
        bail!("--output requires exactly one input; use --output-dir for a batch");
    }
    let sources = args
        .inputs
        .iter()
        .map(fs::canonicalize)
        .collect::<std::io::Result<HashSet<_>>>()
        .context("cannot find an input file")?;
    let mut outputs = HashSet::new();
    let mut jobs = Vec::new();
    for input in &args.inputs {
        let stem = input.file_stem().context("input filename has no stem")?;
        let output = args.output.clone().unwrap_or_else(|| {
            let mut filename = stem.to_os_string();
            filename.push("_n.png");
            args.output_dir
                .as_deref()
                .unwrap_or_else(|| input.parent().unwrap_or(Path::new(".")))
                .join(filename)
        });
        if output
            .extension()
            .is_none_or(|ext| !ext.eq_ignore_ascii_case("png"))
        {
            bail!("normal output must have a .png extension");
        }
        let mut paths = vec![output.clone()];
        if media::is_animation(&read_asset_bytes(input)?)? || args.frame_size.is_some() {
            paths.push(output.with_extension("color.png"));
            paths.push(output.with_extension("frames.json"));
        }
        for path in paths {
            let absolute = normalized_output(&path)?;
            if sources.contains(&absolute) {
                bail!("refusing to overwrite input {}", path.display());
            }
            if !outputs.insert(absolute) {
                bail!("duplicate output name: {}", path.display());
            }
            if path.is_dir() {
                bail!("output is a directory: {}", path.display());
            }
            if path.exists() && !args.force {
                bail!(
                    "output already exists: {}; use --force to replace outputs",
                    path.display()
                );
            }
        }
        jobs.push(Job {
            input: input.clone(),
            output,
        });
    }
    Ok(jobs)
}

fn main() -> Result<()> {
    let args = Args::parse();
    let settings = args.settings();
    settings.validate()?;
    for job in prepare_jobs(&args)? {
        let sprite = process_image(&read_asset_bytes(&job.input)?, &settings)
            .with_context(|| format!("processing {}", job.input.display()))?;
        output::save_png(&sprite.normal, &job.output)?;
        if sprite.metadata.animation.is_some() {
            output::save_png(&sprite.color, &job.output.with_extension("color.png"))?;
            output::save_metadata(&sprite.metadata, &job.output.with_extension("frames.json"))?;
        }
        eprintln!("{} -> {}", job.input.display(), job.output.display());
    }
    Ok(())
}

fn read_asset_bytes(path: &Path) -> Result<Vec<u8>> {
    if fs::metadata(path)?.len() > media::MAX_INPUT_BYTES as u64 {
        bail!("{} exceeds the 64 MiB input limit", path.display());
    }
    fs::read(path).with_context(|| format!("reading {}", path.display()))
}

/// Resolve parent symlinks and '..' even when the output directory does not exist yet.
fn normalized_output(path: &Path) -> Result<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in std::path::absolute(path)?.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                resolved.pop();
            }
            other => {
                resolved.push(other.as_os_str());
                if resolved.exists() {
                    resolved = fs::canonicalize(&resolved)?;
                }
            }
        }
    }
    Ok(resolved)
}
