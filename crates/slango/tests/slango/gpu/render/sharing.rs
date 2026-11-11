use std::thread;

use slango::{AdapterSelection, Buffer, RenderPipelineOptions, Texture, TextureFormat};

use crate::common::{BLUE, FLAT, GREEN, GREEN_PIXEL, RED, RED_PIXEL, Triangles, device_with, load};

#[test]
fn clones_of_one_pipeline_draw_from_two_threads_into_their_own_targets() {
    let device = device_with(AdapterSelection::Default);
    let pipeline = load(&device, FLAT)
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let threads: Vec<_> = [(RED, RED_PIXEL), (GREEN, GREEN_PIXEL)]
        .into_iter()
        .map(|(colour, pixel)| {
            let pipeline = pipeline.clone();
            let device = device.clone();
            thread::spawn(move || {
                let triangle = Triangles::new(
                    &device,
                    &[(
                        [[-1.0, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]],
                        colour,
                    )],
                );
                let target =
                    Texture::new(&device, [8, 8], TextureFormat::Rgba8Unorm).expect("a target");
                for _ in 0..10 {
                    triangle
                        .draw(&pipeline, &target, Some(BLUE))
                        .expect("the draw");
                }
                (target.read().expect("the pixels"), pixel)
            })
        })
        .collect();
    for thread in threads {
        let (pixels, pixel) = thread.join().expect("the thread draws");
        assert_eq!(pixels, [pixel; 64]);
    }
}

#[test]
fn a_kernel_fills_a_vertex_buffer_and_another_reads_the_target_of_a_draw() {
    let device = device_with(AdapterSelection::Default);
    let module = load(
        &device,
        r#"
RWStructuredBuffer<float4> corners;
Texture2D<float4> image;
RWStructuredBuffer<float4> picked;

[shader("compute")]
[numthreads(3, 1, 1)]
void fill(uint3 id : SV_DispatchThreadID)
{
    float2 positions[3] = { float2(-1.0, -1.0), float2(3.0, -1.0), float2(-1.0, 3.0) };
    corners[id.x] = float4(positions[id.x], 0.5, 1.0);
}

[shader("compute")]
[numthreads(1, 1, 1)]
void pick()
{
    picked[0] = image.Load(int3(2, 1, 0));
}

[shader("vertex")]
float4 vsMain(float4 position : POSITION) : SV_Position { return position; }

[shader("fragment")]
float4 fsMain() : SV_Target { return float4(0.0, 1.0, 0.0, 1.0); }
"#,
    );
    let corners = Buffer::<[f32; 4]>::zeroed(&device, 3).expect("the corners");
    module
        .kernel("fill")
        .expect("the kernel")
        .call()
        .bind("corners", &corners)
        .dispatch([3, 1, 1])
        .expect("the dispatch");
    let pipeline = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let target = Texture::new(&device, [4, 4], TextureFormat::Rgba8Unorm).expect("a target");
    pipeline
        .draw()
        .vertex("position", &corners)
        .target(&target)
        .clear(BLUE)
        .submit(3, 1)
        .expect("the draw");
    assert_eq!(target.read().expect("the pixels"), [GREEN_PIXEL; 16]);
    let picked = Buffer::<[f32; 4]>::zeroed(&device, 1).expect("the result");
    module
        .kernel("pick")
        .expect("the kernel")
        .call()
        .texture("image", &target)
        .bind("picked", &picked)
        .dispatch([1, 1, 1])
        .expect("the dispatch");
    assert_eq!(picked.read().expect("the result"), [GREEN]);
}
