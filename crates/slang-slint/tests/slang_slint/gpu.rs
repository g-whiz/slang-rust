use std::error::Error as _;

use slang_slint::{ErrorKind, View};
use slango::TextureFormat;

use crate::common::{GREEN, RED, assert_filled, device, fill, pipeline_with, view, wgpu_texture};

#[test]
fn a_new_view_has_an_rgba8_unorm_texture_of_its_size() {
    let view = view([5, 3]);
    assert_eq!(view.size(), [5, 3]);
    assert_eq!(view.texture().size(), [5, 3]);
    assert_eq!(view.texture().format(), TextureFormat::Rgba8Unorm);
}

#[test]
fn a_draw_of_the_view_fills_its_texture() {
    let view = view([5, 3]);
    fill(&view, RED);
    assert_filled(&view, [255, 0, 0, 255]);
}

#[test]
fn a_resized_view_draws_into_a_new_texture_of_the_new_size() {
    let mut view = view([5, 3]);
    fill(&view, RED);
    let old = view.texture().clone();
    assert!(view.resize([7, 2]).expect("the resize"));
    assert_eq!(view.size(), [7, 2]);
    assert_eq!(view.texture().size(), [7, 2]);
    assert_ne!(
        slango::wgpu::texture(&old),
        slango::wgpu::texture(view.texture())
    );
    assert_filled(&view, [0, 0, 0, 0]);
    fill(&view, GREEN);
    assert_filled(&view, [0, 255, 0, 255]);
}

#[test]
fn a_resize_to_the_current_size_keeps_the_texture_and_its_pixels() {
    let mut view = view([5, 3]);
    fill(&view, RED);
    let before = wgpu_texture(&view);
    assert!(!view.resize([5, 3]).expect("the resize"));
    assert_eq!(wgpu_texture(&view), before);
    assert_filled(&view, [255, 0, 0, 255]);
}

#[test]
fn a_size_without_area_is_an_invalid_size_for_a_new_view() {
    let device = device();
    let pipeline = pipeline_with(&device, TextureFormat::Rgba8Unorm);
    for size in [[0, 3], [5, 0], [0, 0]] {
        let error = View::new(device.clone(), pipeline.clone(), size)
            .expect_err("a view without area was created");
        assert_eq!(error.kind(), ErrorKind::InvalidSize);
        let [width, height] = size;
        assert_eq!(
            error.to_string(),
            format!(
                "cannot create a view: the size is {width} by {height} pixels, which has no area"
            )
        );
    }
}

#[test]
fn a_resize_to_a_size_without_area_is_an_invalid_size_and_keeps_the_texture() {
    let mut view = view([5, 3]);
    fill(&view, RED);
    let before = wgpu_texture(&view);
    let error = view
        .resize([0, 3])
        .expect_err("a resize without area succeeded");
    assert_eq!(error.kind(), ErrorKind::InvalidSize);
    assert_eq!(
        error.to_string(),
        "cannot resize a view: the size is 0 by 3 pixels, which has no area"
    );
    assert_eq!(view.size(), [5, 3]);
    assert_eq!(wgpu_texture(&view), before);
    assert_filled(&view, [255, 0, 0, 255]);
}

#[test]
fn a_size_over_the_limit_of_the_device_is_an_error_of_slango() {
    let device = device();
    let limit = slango::wgpu::device(&device)
        .limits()
        .max_texture_dimension_2d;
    let pipeline = pipeline_with(&device, TextureFormat::Rgba8Unorm);
    let error = View::new(device.clone(), pipeline.clone(), [limit + 1, 1])
        .expect_err("a view over the limit was created");
    assert_eq!(error.kind(), ErrorKind::Slango);
    assert_eq!(error.to_string(), "cannot create a view");
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<slango::Error>())
        .expect("the error of slango as the source");
    assert_eq!(source.kind(), slango::ErrorKind::InvalidArgument);

    let mut view = View::new(device, pipeline, [5, 3]).expect("the view");
    fill(&view, RED);
    let before = wgpu_texture(&view);
    let error = view
        .resize([1, limit + 1])
        .expect_err("a resize over the limit succeeded");
    assert_eq!(error.kind(), ErrorKind::Slango);
    assert_eq!(error.to_string(), "cannot resize a view");
    assert_eq!(view.size(), [5, 3]);
    assert_eq!(wgpu_texture(&view), before);
    assert_filled(&view, [255, 0, 0, 255]);
}

#[test]
fn a_pipeline_of_another_colour_format_fails_at_the_draw() {
    let device = device();
    let pipeline = pipeline_with(&device, TextureFormat::Bgra8Unorm);
    let view = View::new(device, pipeline, [5, 3]).expect("the view");
    let error = view
        .draw()
        .set("colour", RED)
        .submit(3, 1)
        .expect_err("a draw of another colour format succeeded");
    assert_eq!(error.kind(), slango::ErrorKind::InvalidArgument, "{error}");
    assert_filled(&view, [0, 0, 0, 0]);
}

#[test]
fn the_image_of_a_view_shares_its_texture_and_has_its_size() {
    let mut view = view([5, 3]);
    let image = view.to_image().expect("the image of the view");
    assert_eq!([image.size().width, image.size().height], [5, 3]);
    assert_eq!(image.to_wgpu_30_texture(), Some(wgpu_texture(&view)));

    assert!(view.resize([7, 2]).expect("the resize"));
    let resized = view.to_image().expect("the image of the resized view");
    assert_eq!([resized.size().width, resized.size().height], [7, 2]);
    assert_eq!(resized.to_wgpu_30_texture(), Some(wgpu_texture(&view)));
    assert_ne!(image.to_wgpu_30_texture(), resized.to_wgpu_30_texture());
}
