use std::fs;

use slango::{Device, ErrorKind, Texture, TextureFormat};

#[path = "../../../src/bin/windowed-renderer/scene.rs"]
mod scene;

use scene::Scene;

use crate::common::{device, rewrite, scratch_copy, shader};

const SHADER: &str = "textured-quad.slang";
const SIZE: u32 = 64;
const LIGHT: [u8; 4] = [240, 240, 230, 255];
const DARK: [u8; 4] = [230, 120, 30, 255];

struct Image {
    width: u32,
    pixels: Vec<[u8; 4]>,
}

impl Image {
    fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels[usize::try_from(y * self.width + x).expect("the index fits")]
    }

    fn covered_in_row(&self, y: u32) -> Vec<u32> {
        (0..self.width)
            .filter(|&x| !is_clear(self.pixel(x, y)))
            .collect()
    }

    fn covered_in_column(&self, x: u32) -> Vec<u32> {
        let height = u32::try_from(self.pixels.len()).expect("the size fits") / self.width;
        (0..height)
            .filter(|&y| !is_clear(self.pixel(x, y)))
            .collect()
    }
}

fn draw_sized(device: &Device, scene: &Scene, size: [u32; 2], angle: f32) -> Image {
    let target =
        Texture::new(device, size, TextureFormat::Rgba8Unorm).expect("the target is created");
    scene.draw(&target, angle).expect("the scene draws");
    Image {
        width: size[0],
        pixels: target.read().expect("the target is read"),
    }
}

fn draw(device: &Device, scene: &Scene, angle: f32) -> Image {
    draw_sized(device, scene, [SIZE, SIZE], angle)
}

fn is_clear(colour: [u8; 4]) -> bool {
    colour[..3].iter().all(|&channel| channel < 32) && colour[3] == 255
}

/// At angle 0 on a square target of 64 pixels, the quad spans 0.7 of the
/// normalized device coordinates on each side of the centre, so its edges sit
/// at pixel coordinate 32 - 22.4 = 9.6 and 32 + 22.4 = 54.4, and it covers the
/// pixels 10 to 53 on each axis. Each of its 8 texels spans 44.8 / 8 = 5.6
/// pixels. The centre of pixel 12 lies at 12.5 - 9.6 = 2.9 pixels into the
/// quad, in texel 0, and the centre of pixel 18 at 8.9 pixels, in texel 1.
/// The texture coordinate v is 0 at the top of the quad and u is 0 at its
/// left, so pixel (12, 12) shows texel (0, 0), which is light, and pixel
/// (18, 12) shows texel (1, 0), which is dark.
///
/// At angle 0.3, a vertex of the quad at (x, y) moves to
/// 0.7 * (x cos 0.3 - y sin 0.3, x sin 0.3 + y cos 0.3). The centre of pixel
/// (44, 8) lies at (0.3906, 0.7344) in normalized device coordinates, which
/// the inverse rotation takes back to (0.843, 0.837) on the quad, inside it.
/// Its mirror image, pixel (19, 8) at (-0.3906, 0.7344), goes back to
/// (-0.223, 1.167), outside it. A rotation by -0.3 swaps the two.
#[test]
fn draws_the_rotated_textured_quad_over_the_clear_colour() {
    let device = device();
    let scene = Scene::new(&device, &shader(SHADER), TextureFormat::Rgba8Unorm)
        .expect("the scene is created");

    let upright = draw(&device, &scene, 0.0);
    assert!(is_clear(upright.pixel(0, 0)), "{:?}", upright.pixel(0, 0));
    assert_eq!(upright.pixel(12, 12), LIGHT);
    assert_eq!(upright.pixel(18, 12), DARK);
    assert_eq!(upright.pixel(12, 18), DARK);
    assert_eq!(
        upright.covered_in_row(SIZE / 2),
        (10..=53).collect::<Vec<_>>()
    );
    assert_eq!(
        upright.covered_in_column(SIZE / 2),
        (10..=53).collect::<Vec<_>>()
    );

    let rotated = draw(&device, &scene, 0.3);
    assert!(
        !is_clear(rotated.pixel(44, 8)),
        "the upper right corner of the quad turns up: {:?}",
        rotated.pixel(44, 8)
    );
    assert!(
        is_clear(rotated.pixel(19, 8)),
        "the upper left corner of the quad turns down: {:?}",
        rotated.pixel(19, 8)
    );
}

/// On a target of 128 by 64 pixels, the aspect ratio is 2, so the quad spans
/// 0.7 / 2 = 0.35 of the normalized device coordinates on each side of the
/// centre along x and 0.7 along y. Along x, its edges sit at pixel coordinate
/// 64 - 0.35 * 64 = 41.6 and 64 + 22.4 = 86.4, so it covers the pixels 42 to
/// 85. Along y, they sit at 32 - 0.7 * 32 = 9.6 and 54.4, so it covers the
/// pixels 10 to 53. Both are 44 pixels, centred on the target.
#[test]
fn keeps_the_quad_square_on_a_wide_target() {
    let device = device();
    let scene = Scene::new(&device, &shader(SHADER), TextureFormat::Rgba8Unorm)
        .expect("the scene is created");

    let wide = draw_sized(&device, &scene, [128, 64], 0.0);
    assert_eq!(wide.covered_in_row(32), (42..=85).collect::<Vec<_>>());
    assert_eq!(wide.covered_in_column(64), (10..=53).collect::<Vec<_>>());
    assert!(is_clear(wide.pixel(20, 32)), "{:?}", wide.pixel(20, 32));
    assert!(is_clear(wide.pixel(107, 32)), "{:?}", wide.pixel(107, 32));
    assert!(!is_clear(wide.pixel(64, 12)), "{:?}", wide.pixel(64, 12));
    assert!(!is_clear(wide.pixel(64, 51)), "{:?}", wide.pixel(64, 51));
}

/// On a target of 64 by 128 pixels, the aspect ratio is 0.5, so the quad
/// spans 0.7 of the normalized device coordinates on each side of the centre
/// along x and 0.7 * 0.5 = 0.35 along y. Along x, it covers the pixels 10 to
/// 53, and along y, the pixels 42 to 85, as on the wide target with the axes
/// swapped. Both are 44 pixels, centred on the target.
#[test]
fn keeps_the_quad_square_on_a_tall_target() {
    let device = device();
    let scene = Scene::new(&device, &shader(SHADER), TextureFormat::Rgba8Unorm)
        .expect("the scene is created");

    let tall = draw_sized(&device, &scene, [64, 128], 0.0);
    assert_eq!(tall.covered_in_row(64), (10..=53).collect::<Vec<_>>());
    assert_eq!(tall.covered_in_column(32), (42..=85).collect::<Vec<_>>());
    assert!(is_clear(tall.pixel(32, 20)), "{:?}", tall.pixel(32, 20));
    assert!(is_clear(tall.pixel(32, 107)), "{:?}", tall.pixel(32, 107));
    assert!(!is_clear(tall.pixel(12, 64)), "{:?}", tall.pixel(12, 64));
    assert!(!is_clear(tall.pixel(51, 64)), "{:?}", tall.pixel(51, 64));
}

#[test]
fn keeps_its_pipeline_when_a_reload_fails() {
    let device = device();
    let path = scratch_copy("windowed-renderer", SHADER, "reload-fails");
    let mut scene =
        Scene::new(&device, &path, TextureFormat::Rgba8Unorm).expect("the scene is created");
    assert!(!scene.reload_if_changed().expect("an unchanged file loads"));
    let before = draw(&device, &scene, 0.0);

    rewrite(&path, "this is not Slang");
    let error = scene
        .reload_if_changed()
        .expect_err("a file that does not compile fails to reload");
    assert_eq!(error.kind(), ErrorKind::Compile, "{error}");
    assert_eq!(draw(&device, &scene, 0.0).pixels, before.pixels);
    assert!(
        !scene
            .reload_if_changed()
            .expect("a failed file is not loaded again until it changes")
    );
}

#[test]
fn replaces_its_pipeline_when_a_reload_succeeds() {
    let device = device();
    let path = scratch_copy("windowed-renderer", SHADER, "reload-succeeds");
    let mut scene =
        Scene::new(&device, &path, TextureFormat::Rgba8Unorm).expect("the scene is created");
    let source = fs::read_to_string(&path).expect("the shader is read");
    let red = source.replace(
        "return image.Sample(imageSampler, input.uv);",
        "return float4(1.0, 0.0, 0.0, 1.0);",
    );
    assert_ne!(red, source);

    rewrite(&path, &red);
    assert!(scene.reload_if_changed().expect("the changed file loads"));
    let pixels = draw(&device, &scene, 0.0);
    assert_eq!(pixels.pixel(SIZE / 2, SIZE / 2), [255, 0, 0, 255]);
    assert!(is_clear(pixels.pixel(0, 0)), "{:?}", pixels.pixel(0, 0));
}
