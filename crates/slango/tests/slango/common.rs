use std::path::PathBuf;

use slango::{
    AdapterSelection, Buffer, Device, DeviceOptions, FilterMode, Kernel, Module, RenderPipeline,
    RenderPipelineOptions, Sampler, SamplerOptions, Texture, TextureFormat, VertexElement,
};

pub(crate) fn device_with(adapter: AdapterSelection) -> Device {
    let mut options = DeviceOptions::default();
    options.adapter = adapter;
    let device = Device::new(&options).expect("a device on the selected adapter");
    let wgpu_device = slango::wgpu::device(&device);
    println!(
        "slango gpu test adapter: {:?}, immediates: {}, max immediate size: {}",
        wgpu_device.adapter_info(),
        wgpu_device.features().contains(wgpu::Features::IMMEDIATES),
        wgpu_device.limits().max_immediate_size
    );
    device
}

pub(crate) fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

pub(crate) fn assert_immediates_follow_the_adapter(device: &Device) {
    let wgpu_device = slango::wgpu::device(device);
    let info = wgpu_device.adapter_info();
    let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::PRIMARY))
        .into_iter()
        .find(|adapter| adapter.get_info() == info)
        .expect("the adapter of the device among the adapters of a new instance");
    let offered = adapter.features().contains(wgpu::Features::IMMEDIATES);
    assert_eq!(
        wgpu_device.features().contains(wgpu::Features::IMMEDIATES),
        offered
    );
    let expected_size = if offered {
        adapter.limits().max_immediate_size
    } else {
        0
    };
    assert_eq!(wgpu_device.limits().max_immediate_size, expected_size);
}

pub(crate) fn device_without_immediates() -> Device {
    let adapter =
        pollster::block_on(instance().request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device without optional features");
    assert!(!device.features().contains(wgpu::Features::IMMEDIATES));
    slango::wgpu::from_device_and_queue(device, queue)
}

pub(crate) fn load(device: &Device, source: &str) -> Module {
    device
        .load_module_from_source("kernels", source)
        .expect("the module loads")
}

pub(crate) fn shader(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("slango")
        .join("shaders")
        .join(name)
}

pub(crate) fn kernel(device: &Device, source: &str, name: &str) -> Kernel {
    load(device, source)
        .kernel(name)
        .expect("the kernel exists")
        .clone()
}

/// Returns the pixel at column `x` and row `y`, counted from the top-left,
/// of the pixels of a texture `width` pixels wide.
pub(crate) fn pixel_at(pixels: &[[u8; 4]], width: u32, x: u32, y: u32) -> [u8; 4] {
    let index = usize::try_from(y * width + x).expect("an index of a pixel");
    *pixels.get(index).expect("the pixel lies in the texture")
}

/// Asserts that each channel of `actual` differs from `expected` by at most
/// `tolerance` steps of 1/255.
pub(crate) fn assert_near(actual: [u8; 4], expected: [u8; 4], tolerance: u8, what: &str) {
    let near = actual
        .iter()
        .zip(&expected)
        .all(|(&actual, &expected)| actual.abs_diff(expected) <= tolerance);
    assert!(
        near,
        "{what}: expected {expected:?} within {tolerance}, found {actual:?}"
    );
}

/// A module whose vertex entry point `vsMain` takes a position in clip space
/// and a colour for each vertex, and whose fragment entry point `fsMain`
/// writes the interpolated colour.
pub(crate) const FLAT: &str = r#"
struct VIn { float3 position : POSITION; float4 colour : COLOR; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position, 1.0);
    output.colour = input.colour;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return input.colour;
}
"#;

/// The vertex buffers of triangles of [`FLAT`], with three vertices for
/// each triangle and one colour for all three.
pub(crate) struct Triangles {
    pub(crate) positions: Buffer<[f32; 3]>,
    pub(crate) colours: Buffer<[f32; 4]>,
    pub(crate) vertices: u32,
}

impl Triangles {
    pub(crate) fn new(device: &Device, triangles: &[([[f32; 3]; 3], [f32; 4])]) -> Self {
        let positions: Vec<[f32; 3]> = triangles.iter().flat_map(|(corners, _)| *corners).collect();
        let colours: Vec<[f32; 4]> = triangles
            .iter()
            .flat_map(|(_, colour)| [*colour; 3])
            .collect();
        Self {
            positions: Buffer::from_slice(device, &positions).expect("the positions"),
            colours: Buffer::from_slice(device, &colours).expect("the colours"),
            vertices: u32::try_from(positions.len()).expect("a count of vertices"),
        }
    }

    /// Draws the triangles with `pipeline`, a pipeline of [`FLAT`], into
    /// `target`, cleared to `clear` first when it is given.
    pub(crate) fn draw(
        &self,
        pipeline: &RenderPipeline,
        target: &Texture,
        clear: Option<[f32; 4]>,
    ) -> slango::Result<()> {
        let mut draw = pipeline.draw();
        draw.vertex("input.position", &self.positions)
            .vertex("input.colour", &self.colours)
            .target(target);
        if let Some(colour) = clear {
            draw.clear(colour);
        }
        draw.submit(self.vertices, 1)
    }
}

pub(crate) const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
pub(crate) const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
pub(crate) const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];
pub(crate) const RED_PIXEL: [u8; 4] = [255, 0, 0, 255];
pub(crate) const GREEN_PIXEL: [u8; 4] = [0, 255, 0, 255];
pub(crate) const BLUE_PIXEL: [u8; 4] = [0, 0, 255, 255];

pub(crate) fn render_pipeline(
    device: &Device,
    source: &str,
    options: &RenderPipelineOptions,
) -> RenderPipeline {
    load(device, source)
        .render_pipeline("vsMain", "fsMain", options)
        .expect("the render pipeline")
}

/// Returns the pipeline of the usage illustration of Section 5.4 of
/// `docs/design/slango.md`, from the shader file `quad.slang`.
pub(crate) fn quad_pipeline(device: &Device) -> RenderPipeline {
    device
        .load_module(shader("quad.slang"))
        .expect("the module loads")
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the render pipeline")
}

pub(crate) fn texture(device: &Device, size: [u32; 2], format: TextureFormat) -> Texture {
    Texture::new(device, size, format).expect("a texture")
}

pub(crate) fn buffer<T: VertexElement>(device: &Device, elements: &[T]) -> Buffer<T> {
    Buffer::from_slice(device, elements).expect("a buffer")
}

pub(crate) fn read(texture: &Texture) -> Vec<[u8; 4]> {
    texture.read().expect("the pixels")
}

pub(crate) fn sampler(device: &Device, filter: FilterMode) -> Sampler {
    let mut options = SamplerOptions::default();
    options.filter = filter;
    Sampler::new(device, &options).expect("a sampler")
}
