use slango::{Buffer, Device, DeviceOptions};

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` has a public field for each kernel and keeps its module private behind `Shader::module` (SLM-15)"
)]
mod saxpy {
    slango_macros::shader!("shaders/saxpy.slang");
}

fn inputs(count: u16) -> (Vec<f32>, Vec<f32>) {
    let x = (0..count).map(f32::from).collect();
    let y = (0..count)
        .map(|index| f32::from(index) * 0.5 + 1.0)
        .collect();
    (x, y)
}

fn saxpy_on_the_cpu(a: f32, x: &[f32], y: &[f32]) -> Vec<f32> {
    x.iter().zip(y).map(|(x, y)| a * x + y).collect()
}

#[test]
fn saxpy_through_the_bindings_gives_the_results_computed_on_the_cpu() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let shader = saxpy::Shader::load(&device)?;
    let (x, y) = inputs(1000);
    let x_buffer = Buffer::from_slice(&device, &x)?;
    let y_buffer = Buffer::from_slice(&device, &y)?;
    let result = Buffer::<f32>::zeroed(&device, 1000)?;
    let args = saxpy::SaxpyArgs {
        a: 2.5,
        count: 1000,
        x: &x_buffer,
        y: &y_buffer,
        result: &result,
    };
    shader.saxpy.dispatch(&args, [1000, 1, 1])?;
    assert_eq!(result.read()?, saxpy_on_the_cpu(2.5, &x, &y));
    Ok(())
}

#[test]
fn saxpy_on_thread_groups_stops_at_the_count() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let shader = saxpy::Shader::load(&device)?;
    let (x, y) = inputs(200);
    let x_buffer = Buffer::from_slice(&device, &x)?;
    let y_buffer = Buffer::from_slice(&device, &y)?;
    let result = Buffer::from_slice(&device, &[-1.0_f32; 200])?;
    let args = saxpy::SaxpyArgs {
        a: -3.0,
        count: 130,
        x: &x_buffer,
        y: &y_buffer,
        result: &result,
    };
    shader.saxpy.dispatch_groups(&args, [3, 1, 1])?;
    let mut expected = saxpy_on_the_cpu(-3.0, &x[..130], &y[..130]);
    expected.resize(200, -1.0);
    assert_eq!(result.read()?, expected);
    assert_eq!(shader.saxpy.kernel().name(), "saxpy");
    assert!(shader.module().kernel("saxpy").is_ok());
    Ok(())
}
