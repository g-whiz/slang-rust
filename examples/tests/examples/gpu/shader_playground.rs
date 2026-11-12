use std::fs;

use slango::ErrorKind;

#[path = "../../../src/bin/shader-playground/scene.rs"]
mod scene;

use scene::{Scene, Uniforms};

use crate::common::{device, rewrite, scratch_copy, shader};

const SHADER: &str = "playground.slang";
const SIZE: [u32; 2] = [64, 32];

fn uniforms(time: f32, brightness: f32, scale: f32) -> Uniforms {
    Uniforms {
        time,
        brightness,
        scale,
    }
}

fn draw(scene: &Scene, uniforms: Uniforms) -> Vec<[u8; 4]> {
    scene.draw(uniforms).expect("the scene draws");
    scene.view().texture().read().expect("the view is read")
}

fn pixel(pixels: &[[u8; 4]], width: u32, x: u32, y: u32) -> [u8; 4] {
    pixels[usize::try_from(y * width + x).expect("the index fits")]
}

/// The colour that `playground.slang` gives the pixel `(x, y)` of a view of
/// `size`, as bytes of `Rgba8Unorm`. The texture coordinate of the centre of
/// the pixel is `uv = ((x + 0.5) / width, (y + 0.5) / height)`, the pattern
/// coordinate is `p = (2 uv - 1) scale`, and each channel `i` of red, green,
/// and blue is `brightness (0.5 + 0.5 cos(time + q_i + 2 i))` with
/// `q = (p.x, p.y, p.x)`.
fn expected(size: [u32; 2], x: u32, y: u32, uniforms: Uniforms) -> [f32; 3] {
    let [width, height] = size.map(f64::from);
    let px = ((f64::from(x) + 0.5) / width * 2.0 - 1.0) * f64::from(uniforms.scale);
    let py = ((f64::from(y) + 0.5) / height * 2.0 - 1.0) * f64::from(uniforms.scale);
    let time = f64::from(uniforms.time);
    let brightness = f64::from(uniforms.brightness);
    [(px, 0.0), (py, 2.0), (px, 4.0)].map(|(q, offset)| {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a channel of at most 255 fits in an f32"
        )]
        let channel = (brightness * (0.5 + 0.5 * (time + q + offset).cos()) * 255.0) as f32;
        channel
    })
}

fn assert_near(actual: [u8; 4], expected: [f32; 3], at: (u32, u32)) {
    let [red, green, blue, alpha] = actual;
    assert_eq!(alpha, 255, "alpha at {at:?}");
    for (channel, want) in [red, green, blue].into_iter().zip(expected) {
        assert!(
            (f32::from(channel) - want).abs() <= 2.0,
            "{actual:?} at {at:?} is not near {expected:?}"
        );
    }
}

fn assert_pattern(pixels: &[[u8; 4]], uniforms: Uniforms) {
    for y in 0..SIZE[1] {
        for x in 0..SIZE[0] {
            assert_near(
                pixel(pixels, SIZE[0], x, y),
                expected(SIZE, x, y, uniforms),
                (x, y),
            );
        }
    }
}

/// With a scale of 0, every pixel has the colour
/// `brightness (0.5 + 0.5 cos(time + 2 i))`, which at time 0 is
/// `(1.0, 0.292, 0.173)` times the brightness, or the bytes `(255, 74, 44)`
/// at brightness 1 and `(128, 37, 22)` at brightness 0.5, each within the
/// rounding of the conversion to bytes.
#[test]
fn the_brightness_slider_scales_every_colour() {
    let device = device();
    let scene = Scene::new(&device, &shader(SHADER), SIZE).expect("the scene is created");

    let full = draw(&scene, uniforms(0.0, 1.0, 0.0));
    assert_near(full[0], [255.0, 74.4, 44.2], (0, 0));
    assert_pattern(&full, uniforms(0.0, 1.0, 0.0));

    let half = draw(&scene, uniforms(0.0, 0.5, 0.0));
    assert_near(half[0], [127.5, 37.2, 22.1], (0, 0));
    assert_pattern(&half, uniforms(0.0, 0.5, 0.0));
}

/// With a scale of 0, the view has one colour. With a scale of 2, the
/// pattern coordinate runs from about -2 at the left and top edges to about
/// 2 at the right and bottom edges, so the colour changes across the view
/// and differs between its corners.
#[test]
fn the_scale_slider_sets_the_size_of_the_pattern() {
    let device = device();
    let scene = Scene::new(&device, &shader(SHADER), SIZE).expect("the scene is created");

    let flat = draw(&scene, uniforms(0.0, 1.0, 0.0));
    assert!(flat.iter().all(|colour| *colour == flat[0]), "{flat:?}");

    let wide = draw(&scene, uniforms(0.0, 1.0, 1.0));
    assert_pattern(&wide, uniforms(0.0, 1.0, 1.0));

    let narrow = draw(&scene, uniforms(0.0, 1.0, 2.0));
    assert_pattern(&narrow, uniforms(0.0, 1.0, 2.0));
    let [width, height] = SIZE;
    assert_ne!(
        pixel(&narrow, width, 0, 0),
        pixel(&narrow, width, width - 1, height - 1)
    );
    assert_ne!(narrow, wide);
}

/// With a scale of 0 at time 1, every pixel has the colour
/// `0.5 + 0.5 cos(1 + 2 i)`, which is `(0.770, 0.005, 0.642)`, or the bytes
/// `(196, 1, 164)`.
#[test]
fn the_time_shifts_the_colours() {
    let device = device();
    let scene = Scene::new(&device, &shader(SHADER), SIZE).expect("the scene is created");

    let later = draw(&scene, uniforms(1.0, 1.0, 0.0));
    assert_near(later[0], [196.4, 1.3, 163.7], (0, 0));
    assert_pattern(&later, uniforms(1.0, 1.0, 0.0));
}

#[test]
fn keeps_its_view_when_a_reload_fails() {
    let device = device();
    let path = scratch_copy("shader-playground", SHADER, "reload-fails");
    let mut scene = Scene::new(&device, &path, SIZE).expect("the scene is created");
    assert!(!scene.reload_if_changed().expect("an unchanged file loads"));
    let before = draw(&scene, uniforms(0.0, 1.0, 1.0));
    let texture = slango::wgpu::texture(scene.view().texture()).clone();

    rewrite(&path, "this is not Slang");
    let error = scene
        .reload_if_changed()
        .expect_err("a file that does not compile fails to reload");
    let error = error
        .downcast_ref::<slango::Error>()
        .expect("the error of slango");
    assert_eq!(error.kind(), ErrorKind::Compile, "{error}");
    assert_eq!(slango::wgpu::texture(scene.view().texture()), &texture);
    assert_eq!(draw(&scene, uniforms(0.0, 1.0, 1.0)), before);
    assert!(
        !scene
            .reload_if_changed()
            .expect("a failed file is not loaded again until it changes")
    );
}

#[test]
fn replaces_its_view_when_a_reload_succeeds() {
    let device = device();
    let path = scratch_copy("shader-playground", SHADER, "reload-succeeds");
    let mut scene = Scene::new(&device, &path, SIZE).expect("the scene is created");
    assert!(scene.resize([16, 8]).expect("the view is resized"));
    let texture = slango::wgpu::texture(scene.view().texture()).clone();
    let source = fs::read_to_string(&path).expect("the shader is read");
    let red = source.replace(
        "return float4(colour * brightness, 1.0);",
        "return float4(brightness, 0.0, 0.0, 1.0);",
    );
    assert_ne!(red, source);

    rewrite(&path, &red);
    assert!(scene.reload_if_changed().expect("the changed file loads"));
    assert_eq!(scene.view().size(), [16, 8]);
    assert_ne!(slango::wgpu::texture(scene.view().texture()), &texture);
    let pixels = draw(&scene, uniforms(0.0, 1.0, 1.0));
    assert_eq!(pixels.len(), 16 * 8);
    assert!(
        pixels.iter().all(|colour| *colour == [255, 0, 0, 255]),
        "{pixels:?}"
    );
}
