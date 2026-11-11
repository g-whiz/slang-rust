use slang_slint::View;
use slango::{Device, DeviceOptions, RenderPipeline, RenderPipelineOptions, TextureFormat};

const FILL: &str = r#"
uniform float4 colour;

struct VOut { float4 position : SV_Position; };

[shader("vertex")]
VOut vsMain(uint vertex : SV_VulkanVertexID)
{
    float2 corner = float2(float((vertex << 1) & 2), float(vertex & 2));
    VOut output;
    output.position = float4(corner * 2.0 - 1.0, 0.0, 1.0);
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return colour;
}
"#;

pub(crate) const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
pub(crate) const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];

pub(crate) fn device() -> Device {
    Device::new(&DeviceOptions::default()).expect("a device on the default adapter")
}

pub(crate) fn pipeline_with(device: &Device, colour_format: TextureFormat) -> RenderPipeline {
    let mut options = RenderPipelineOptions::default();
    options.colour_format = colour_format;
    device
        .load_module_from_source("fill", FILL)
        .expect("the module loads")
        .render_pipeline("vsMain", "fsMain", &options)
        .expect("the render pipeline")
}

pub(crate) fn view(size: [u32; 2]) -> View {
    let device = device();
    let pipeline = pipeline_with(&device, TextureFormat::Rgba8Unorm);
    View::new(device, pipeline, size).expect("the view")
}

pub(crate) fn fill(view: &View, colour: [f32; 4]) {
    view.draw()
        .set("colour", colour)
        .submit(3, 1)
        .expect("the draw");
}

pub(crate) fn assert_filled(view: &View, pixel: [u8; 4]) {
    let [width, height] = view.size();
    let pixels = view.texture().read().expect("the pixels of the view");
    assert_eq!(
        pixels.len(),
        usize::try_from(width * height).expect("a count")
    );
    assert!(
        pixels.iter().all(|read| *read == pixel),
        "expected every pixel to be {pixel:?}, read {pixels:?}"
    );
}

pub(crate) fn wgpu_texture(view: &View) -> slint::wgpu_30::wgpu::Texture {
    slango::wgpu::texture(view.texture()).clone()
}
