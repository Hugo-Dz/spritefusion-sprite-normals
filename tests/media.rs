use image::{Rgba, RgbaImage};
use spritefusion_sprite_normals::{NormalSettings, generate_normal, media};

fn animation() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 16, 16, &[]).unwrap();
        let mut red = RgbaImage::from_pixel(16, 16, Rgba([255, 0, 0, 255])).into_raw();
        let mut first = gif::Frame::from_rgba(16, 16, &mut red);
        first.delay = 5;
        encoder.write_frame(&first).unwrap();
        let mut green = RgbaImage::from_pixel(4, 4, Rgba([0, 255, 0, 255])).into_raw();
        let mut second = gif::Frame::from_rgba(4, 4, &mut green);
        second.left = 4;
        second.top = 4;
        second.delay = 9;
        second.dispose = gif::DisposalMethod::Previous;
        encoder.write_frame(&second).unwrap();
        let mut blue = RgbaImage::from_pixel(2, 2, Rgba([0, 0, 255, 255])).into_raw();
        let mut third = gif::Frame::from_rgba(2, 2, &mut blue);
        third.left = 10;
        third.top = 10;
        third.delay = 12;
        encoder.write_frame(&third).unwrap();
    }
    bytes
}

#[test]
fn gif_disposal_offsets_timing_and_blank_atlas_cells_are_preserved() {
    let bytes = animation();
    assert!(media::is_animation(&bytes).unwrap());
    let asset = media::decode(&bytes).unwrap();
    let animation = asset.animation.unwrap();
    assert_eq!(animation.frame_count, 3);
    assert_eq!(animation.delays_ms, [[50, 1], [90, 1], [120, 1]]);
    assert_eq!(asset.rgba.dimensions(), (32, 32));
    assert_eq!(asset.rgba.get_pixel(20, 4).0, [0, 255, 0, 255]);
    assert_eq!(asset.rgba.get_pixel(4, 20).0, [255, 0, 0, 255]);
    assert_eq!(asset.rgba.get_pixel(10, 26).0, [0, 0, 255, 255]);
    assert_eq!(asset.rgba.get_pixel(24, 24)[3], 0);
    let combined = generate_normal(
        &asset.rgba,
        &NormalSettings {
            frame_size: Some((16, 16)),
            ..NormalSettings::default()
        },
    )
    .unwrap();
    for (x, y) in [(0, 0), (16, 0), (0, 16)] {
        let source = image::imageops::crop_imm(&asset.rgba, x, y, 16, 16).to_image();
        let expected = generate_normal(&source, &NormalSettings::default()).unwrap();
        assert_eq!(
            expected,
            image::imageops::crop_imm(&combined, x, y, 16, 16).to_image()
        );
    }
}

#[test]
fn malformed_media_and_excessive_gif_dimensions_return_errors() {
    assert!(media::decode(b"not an image").is_err());
    let mut bytes = animation();
    bytes[6..10].copy_from_slice(&[255, 255, 255, 255]);
    assert!(media::decode(&bytes).is_err());
}

#[test]
fn apng_retains_frame_offsets_and_fractional_timing() {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 12, 10);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 0).unwrap();
        encoder.set_frame_delay(1, 12).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer
            .write_image_data(RgbaImage::from_pixel(12, 10, Rgba([200, 100, 50, 255])).as_raw())
            .unwrap();
        writer.set_frame_dimension(3, 2).unwrap();
        writer.set_frame_position(4, 3).unwrap();
        writer.set_frame_delay(1, 20).unwrap();
        writer
            .write_image_data(RgbaImage::from_pixel(3, 2, Rgba([50, 100, 200, 255])).as_raw())
            .unwrap();
    }
    assert!(media::is_animation(&bytes).unwrap());
    let asset = media::decode(&bytes).unwrap();
    let animation = asset.animation.unwrap();
    assert_eq!(animation.frame_count, 2);
    let [n, d] = animation.delays_ms[0];
    assert_eq!(n * 12, 1000 * d);
    assert_eq!(animation.delays_ms[1][0], 50 * animation.delays_ms[1][1]);
    assert_eq!(asset.rgba.get_pixel(16, 3).0, [50, 100, 200, 255]);
    assert_eq!(asset.rgba.get_pixel(12, 0).0, [200, 100, 50, 255]);
}
