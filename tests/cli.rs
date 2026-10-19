#![cfg(feature = "cli")]

use image::{Rgba, RgbaImage};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "spritefusion-sprite-normals-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        RgbaImage::from_fn(16, 16, |x, y| {
            Rgba([(x * 12) as u8, 100, 150, (y * 16) as u8])
        })
        .save(dir.join("source.png"))
        .unwrap();
        Self { dir }
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_spritefusion-sprite-normals"))
            .current_dir(&self.dir)
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn cli_output_matches_the_library_and_requires_force_to_replace() {
    let fixture = Fixture::new();
    let input = fs::read(fixture.dir.join("source.png")).unwrap();
    let run = fixture.run(&["source.png"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let output = fixture.dir.join("source_n.png");
    let actual = image::open(&output).unwrap().to_rgba8();
    let expected = spritefusion_sprite_normals::process_image(
        &input,
        &spritefusion_sprite_normals::NormalSettings::default(),
    )
    .unwrap();
    assert_eq!(actual, expected.normal);
    assert!(!fixture.run(&["source.png"]).status.success());
    assert!(fixture.run(&["source.png", "--force"]).status.success());
    assert_eq!(actual, image::open(output).unwrap().to_rgba8());
}

#[test]
fn input_paths_and_aliases_are_protected_even_with_force() {
    let fixture = Fixture::new();
    let before = fs::read(fixture.dir.join("source.png")).unwrap();
    for path in ["source.png", "missing/../source.png"] {
        assert!(
            !fixture
                .run(&["source.png", "-o", path, "--force"])
                .status
                .success()
        );
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            fixture.dir.join("source.png"),
            fixture.dir.join("alias.png"),
        )
        .unwrap();
        assert!(
            !fixture
                .run(&["source.png", "-o", "alias.png", "--force"])
                .status
                .success()
        );
    }
    assert_eq!(before, fs::read(fixture.dir.join("source.png")).unwrap());
}

#[test]
fn batch_names_and_invalid_options_are_checked_before_writing() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.dir.join("second")).unwrap();
    fs::copy(
        fixture.dir.join("source.png"),
        fixture.dir.join("second/source.png"),
    )
    .unwrap();
    for args in [
        vec!["source.png", "second/source.png", "--output-dir", "out"],
        vec!["source.png", "second/source.png", "-o", "out.png"],
        vec!["source.png", "--normal-strength", "NaN"],
        vec!["source.png", "--detail-scale", "0"],
        vec!["source.png", "--shape-radius", "0"],
        vec!["source.png", "--frame-size", "0x8"],
        vec!["source.png", "--frame-size", "7x7"],
        vec!["source.png", "-o", "normal.jpg"],
    ] {
        assert!(!fixture.run(&args).status.success(), "accepted {args:?}");
    }
    assert!(!fixture.dir.join("out").exists());
    assert!(!fixture.dir.join("source_n.png").exists());
    fs::copy(
        fixture.dir.join("source.png"),
        fixture.dir.join("other.png"),
    )
    .unwrap();
    assert!(
        fixture
            .run(&["source.png", "other.png", "--output-dir", "out"])
            .status
            .success()
    );
    assert!(fixture.dir.join("out/source_n.png").exists());
    assert!(fixture.dir.join("out/other_n.png").exists());
}

#[test]
fn animation_sidecars_preserve_timing_and_cannot_overwrite_inputs() {
    let fixture = Fixture::new();
    {
        let file = fs::File::create(fixture.dir.join("walk.gif")).unwrap();
        let mut encoder = image::codecs::gif::GifEncoder::new(file);
        for value in [60, 190] {
            encoder
                .encode_frame(image::Frame::from_parts(
                    RgbaImage::from_pixel(16, 16, Rgba([value, 100, 150, 255])),
                    0,
                    0,
                    image::Delay::from_numer_denom_ms(80, 1),
                ))
                .unwrap();
        }
    }
    let run = fixture.run(&["walk.gif", "-o", "walk_n.png"]);
    assert!(
        run.status.success(),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let metadata: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.dir.join("walk_n.frames.json")).unwrap()).unwrap();
    assert_eq!(metadata["animation"]["frame_count"], 2);
    assert_eq!(
        metadata["animation"]["delays_ms"],
        serde_json::json!([[80, 1], [80, 1]])
    );
    assert_eq!(
        image::open(fixture.dir.join("walk_n.png")).unwrap().width(),
        32
    );
    assert!(fixture.dir.join("walk_n.color.png").exists());
    fs::copy(
        fixture.dir.join("source.png"),
        fixture.dir.join("walk_n.color.png"),
    )
    .unwrap();
    let before = fs::read(fixture.dir.join("walk_n.color.png")).unwrap();
    assert!(
        !fixture
            .run(&["walk.gif", "walk_n.color.png", "--force"])
            .status
            .success()
    );
    assert_eq!(
        before,
        fs::read(fixture.dir.join("walk_n.color.png")).unwrap()
    );
    assert!(
        !fixture
            .run(&["walk.gif", "--frame-size", "8x8", "-o", "wrong.png"])
            .status
            .success()
    );
    assert!(!fixture.dir.join("wrong.png").exists());
}
