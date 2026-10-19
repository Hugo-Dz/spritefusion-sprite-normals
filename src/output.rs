use anyhow::{Context, Result};
use image::{ImageEncoder, RgbaImage};
use spritefusion_sprite_normals::Metadata;
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub fn save_png(image: &RgbaImage, path: &Path) -> Result<()> {
    write_output(path, |writer| {
        image::codecs::png::PngEncoder::new(writer).write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            image::ColorType::Rgba8,
        )?;
        Ok(())
    })
}

pub fn save_metadata(metadata: &Metadata, path: &Path) -> Result<()> {
    write_output(path, |writer| {
        serde_json::to_writer_pretty(writer, metadata)?;
        Ok(())
    })
}

/// Encode to a sibling temporary file so a failed write cannot truncate an existing output.
fn write_output(path: &Path, write: impl FnOnce(&mut BufWriter<File>) -> Result<()>) -> Result<()> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    let temporary = path.with_file_name(format!(
        ".spritefusion-sprite-normals-{}-{id}.tmp",
        std::process::id()
    ));
    let file = File::options()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let mut writer = BufWriter::new(file);
    let result = write(&mut writer).and_then(|()| {
        writer.flush()?;
        Ok(())
    });
    drop(writer);
    let result = result.and_then(|()| {
        fs::rename(&temporary, path)?;
        Ok(())
    });
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result.with_context(|| format!("saving {}", path.display()))
}
