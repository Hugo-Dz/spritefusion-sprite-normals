use image::{Rgba, RgbaImage};
use spritefusion_sprite_normals::{
    NormalFormat, NormalSettings, generate_normal, media, process_image,
};

fn disk(size: u32) -> RgbaImage {
    let center = (size / 2) as f32;
    RgbaImage::from_fn(size, size, |x, y| {
        Rgba([
            145,
            99,
            63,
            if (x as f32 - center).hypot(y as f32 - center) <= center - 3.0 {
                255
            } else {
                0
            },
        ])
    })
}

#[test]
fn normals_point_outward_and_decode_to_unit_vectors() {
    let result = generate_normal(&disk(31), &NormalSettings::default()).unwrap();
    assert!(result.get_pixel(7, 15)[0] < 100);
    assert!(result.get_pixel(23, 15)[0] > 155);
    assert!(result.get_pixel(15, 7)[1] > 155);
    assert!(result.get_pixel(15, 23)[1] < 100);
    assert_eq!(result.get_pixel(15, 15).0, [128, 128, 255, 255]);
    for pixel in result.pixels() {
        let length = pixel.0[..3]
            .iter()
            .map(|&c| (f32::from(c) / 127.5 - 1.0).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!((length - 1.0).abs() < 0.01);
    }
}

#[test]
fn zero_strength_and_uniform_shading_produce_neutral_normals() {
    let input = disk(31);
    for settings in [
        NormalSettings {
            normal_strength: 0.0,
            ..NormalSettings::default()
        },
        NormalSettings {
            shape_strength: 0.0,
            ..NormalSettings::default()
        },
    ] {
        let result = generate_normal(&input, &settings).unwrap();
        for (source, normal) in input.pixels().zip(result.pixels()) {
            assert_eq!(normal.0, [128, 128, 255, source[3]]);
        }
    }
}

#[test]
fn broad_painted_shading_retains_relief() {
    let input = RgbaImage::from_fn(64, 32, |x, _| {
        let value = (40 + x * 3) as u8;
        Rgba([value, value, value, 255])
    });
    let result = generate_normal(
        &input,
        &NormalSettings {
            shape_strength: 0.0,
            ..NormalSettings::default()
        },
    )
    .unwrap();
    for x in 12..52 {
        assert!(result.get_pixel(x, 16)[0] < 123);
        assert!((i16::from(result.get_pixel(x, 16)[1]) - 128).abs() <= 1);
    }
}

#[test]
fn alpha_is_preserved_and_hidden_rgb_does_not_affect_normals() {
    let input = RgbaImage::from_fn(16, 16, |x, y| Rgba([90, 160, 210, (y * 16 + x) as u8]));
    let result = generate_normal(&input, &NormalSettings::default()).unwrap();
    for (source, normal) in input.pixels().zip(result.pixels()) {
        assert_eq!(source[3], normal[3]);
        if source[3] <= 7 {
            assert_eq!(normal.0[..3], [128, 128, 255]);
        }
    }
    let original = disk(31);
    let mut changed = original.clone();
    for pixel in changed.pixels_mut().filter(|p| p[3] == 0) {
        *pixel = Rgba([255, 20, 200, 0]);
    }
    assert_eq!(
        generate_normal(&original, &NormalSettings::default()).unwrap(),
        generate_normal(&changed, &NormalSettings::default()).unwrap(),
    );
    let empty = RgbaImage::from_pixel(3, 9, Rgba([255, 80, 10, 0]));
    assert!(
        generate_normal(&empty, &NormalSettings::default())
            .unwrap()
            .pixels()
            .all(|p| p.0 == [128, 128, 255, 0])
    );
}

#[test]
fn padding_and_nearby_islands_do_not_change_a_sprite() {
    let input = RgbaImage::from_fn(9, 9, |x, _| Rgba([(30 + x * 20) as u8, 80, 120, 255]));
    let expected = generate_normal(&input, &NormalSettings::default()).unwrap();
    let mut sheet = RgbaImage::new(42, 34);
    image::imageops::replace(&mut sheet, &input, 5, 5);
    image::imageops::replace(
        &mut sheet,
        &RgbaImage::from_pixel(21, 23, Rgba([255; 4])),
        15,
        5,
    );
    let result = generate_normal(&sheet, &NormalSettings::default()).unwrap();
    assert_eq!(
        expected,
        image::imageops::crop_imm(&result, 5, 5, 9, 9).to_image()
    );
}

#[test]
fn explicit_sheet_cells_are_isolated_and_keep_their_layout() {
    let sheet = RgbaImage::from_fn(32, 16, |x, _| {
        if x < 16 {
            Rgba([220, 70, 40, 255])
        } else {
            Rgba([30, 80, 200, 255])
        }
    });
    let settings = NormalSettings {
        frame_size: Some((16, 16)),
        ..NormalSettings::default()
    };
    let result = process_image(&media::png(&sheet).unwrap(), &settings).unwrap();
    for x in [0, 16] {
        let cell = image::imageops::crop_imm(&sheet, x, 0, 16, 16).to_image();
        assert_eq!(
            generate_normal(&cell, &NormalSettings::default()).unwrap(),
            image::imageops::crop_imm(&result.normal, x, 0, 16, 16).to_image(),
        );
    }
    assert_eq!(result.color, sheet);
    let animation = result.metadata.animation.unwrap();
    assert_eq!((animation.columns, animation.frame_count), (2, 2));
    assert_eq!(animation.delays_ms, [[100, 1], [100, 1]]);
}

#[test]
fn tiny_and_diagonal_shapes_are_deterministic() {
    for (w, h) in [(1, 1), (1, 19), (23, 1), (20, 20)] {
        let input = RgbaImage::from_fn(w, h, |x, y| {
            Rgba([80, 80, 80, if w == 1 || h == 1 || x == y { 255 } else { 0 }])
        });
        let a = generate_normal(&input, &NormalSettings::default()).unwrap();
        assert_eq!(
            a,
            generate_normal(&input, &NormalSettings::default()).unwrap()
        );
    }
}

#[test]
fn normal_conventions_only_flip_the_requested_channels() {
    let input = disk(31);
    let gl = generate_normal(&input, &NormalSettings::default()).unwrap();
    let dx = generate_normal(
        &input,
        &NormalSettings {
            normal_format: NormalFormat::Directx,
            ..NormalSettings::default()
        },
    )
    .unwrap();
    let flipped = generate_normal(
        &input,
        &NormalSettings {
            invert_x: true,
            invert_y: true,
            ..NormalSettings::default()
        },
    )
    .unwrap();
    for ((a, b), c) in gl.pixels().zip(dx.pixels()).zip(flipped.pixels()) {
        assert_eq!([a[0], a[2], a[3]], [b[0], b[2], b[3]]);
        assert!((i16::from(a[1]) + i16::from(b[1]) - 255).abs() <= 1);
        assert!((i16::from(a[0]) + i16::from(c[0]) - 255).abs() <= 1);
        assert_eq!(b[1], c[1]);
    }
}

#[test]
fn invalid_settings_and_dimensions_return_errors() {
    let input = disk(31);
    for value in [f32::NAN, f32::INFINITY, -1.0, 9.0] {
        assert!(
            generate_normal(
                &input,
                &NormalSettings {
                    normal_strength: value,
                    ..NormalSettings::default()
                }
            )
            .is_err()
        );
    }
    for size in [(0, 1), (1, 0), (16, 16), (32, 32)] {
        assert!(
            generate_normal(
                &input,
                &NormalSettings {
                    frame_size: Some(size),
                    ..NormalSettings::default()
                }
            )
            .is_err()
        );
    }
    assert!(
        generate_normal(
            &input,
            &NormalSettings {
                shape_radius: 0.0,
                ..NormalSettings::default()
            }
        )
        .is_err()
    );
    assert!(generate_normal(&RgbaImage::new(0, 0), &NormalSettings::default()).is_err());
    assert!(process_image(b"invalid", &NormalSettings::default()).is_err());
}
