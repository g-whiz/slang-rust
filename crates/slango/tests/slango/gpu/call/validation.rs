use std::error::Error as _;

use slango::{AdapterSelection, Buffer, Call, Device, ErrorKind, Kernel};

use super::OFFSET;
use crate::common::{device_with, kernel};

const SENTINEL: [f32; 4] = [7.0; 4];

struct Fixture {
    device: Device,
    kernel: Kernel,
    input: Buffer<f32>,
    output: Buffer<f32>,
}

fn fixture() -> slango::Result<Fixture> {
    let device = device_with(AdapterSelection::Default);
    let kernel = kernel(&device, OFFSET, "offset");
    let input = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    let output = Buffer::from_slice(&device, &SENTINEL)?;
    Ok(Fixture {
        device,
        kernel,
        input,
        output,
    })
}

impl Fixture {
    /// Returns a call that gives every parameter of the kernel `offset`
    /// except the one at `skip`.
    fn call_without(&self, skip: &str) -> Call<'_> {
        let mut call = self.kernel.call();
        if skip != "bias" {
            call.set("bias", 1.0_f32);
        }
        if skip != "count" {
            call.set("count", 4_u32);
        }
        if skip != "input" {
            call.bind("input", &self.input);
        }
        if skip != "output" {
            call.bind("output", &self.output);
        }
        call
    }

    /// Dispatches `call`, asserts that it fails with `kind` and `text`, and
    /// that the output buffer is unchanged.
    fn assert_rejected(&self, call: &Call<'_>, kind: ErrorKind, text: &str) -> slango::Result<()> {
        let outcome = call
            .dispatch([4, 1, 1])
            .map_err(|error| (error.kind(), error.to_string()));
        assert_eq!(outcome, Err((kind, text.to_owned())));
        assert_eq!(self.output.read()?, SENTINEL);
        Ok(())
    }
}

#[test]
fn the_complete_call_of_the_fixture_runs() -> slango::Result<()> {
    let fixture = fixture()?;
    fixture.call_without("").dispatch([4, 1, 1])?;
    assert_eq!(fixture.output.read()?, [2.0, 3.0, 4.0, 5.0]);
    Ok(())
}

#[test]
fn an_unknown_parameter_path_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("");
    call.set("biass", 1.0_f32);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the kernel `offset` has no buffer or plain value at the path `biass`",
    )
}

#[test]
fn a_plain_value_that_is_not_set_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    fixture.assert_rejected(
        &fixture.call_without("count"),
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the parameter `count` of the kernel `offset` is not set",
    )
}

#[test]
fn a_buffer_that_is_not_bound_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    fixture.assert_rejected(
        &fixture.call_without("input"),
        ErrorKind::InvalidArgument,
        "cannot call a kernel: no buffer is bound to the parameter `input` of the kernel `offset`",
    )
}

#[test]
fn a_value_of_another_rust_type_is_rejected_with_both_types_in_its_source() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("");
    call.set("count", 4_i32);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the kernel `offset` cannot take the value of the parameter `count`",
    )?;
    let error = call.dispatch([4, 1, 1]).expect_err("the call is rejected");
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some(
            "cannot set a plain value: the parameter `count` has the Slang type `uint`, and the value has the Rust type `i32`"
        )
    );
    Ok(())
}

#[test]
fn a_buffer_of_another_element_size_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let wide = Buffer::from_slice(&fixture.device, &[[1.0_f32; 4]; 4])?;
    let mut call = fixture.call_without("input");
    call.bind("input", &wide);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the parameter `input` of the kernel `offset` has elements of 4 bytes, and the bound buffer has elements of 16 bytes",
    )
}

#[test]
fn a_buffer_bound_to_a_plain_value_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("bias");
    call.bind("bias", &fixture.input);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the parameter `bias` of the kernel `offset` is a plain value, and the call binds a buffer to it",
    )
}

#[test]
fn a_value_given_for_a_buffer_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("input");
    call.set("input", 1.0_f32);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the parameter `input` of the kernel `offset` is a buffer, and the call sets a value for it",
    )
}

#[test]
fn a_buffer_of_another_device_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let other = device_with(AdapterSelection::Default);
    let foreign = Buffer::from_slice(&other, &[1.0_f32; 4])?;
    let mut call = fixture.call_without("input");
    call.bind("input", &foreign);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the buffer bound to the parameter `input` of the kernel `offset` belongs to another device",
    )
}

#[test]
fn an_empty_buffer_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let empty = Buffer::<f32>::from_slice(&fixture.device, &[])?;
    let mut call = fixture.call_without("input");
    call.bind("input", &empty);
    fixture.assert_rejected(
        &call,
        ErrorKind::InvalidArgument,
        "cannot call a kernel: the buffer bound to the parameter `input` of the kernel `offset` is empty",
    )
}

#[test]
fn workgroup_counts_over_the_limit_of_the_device_are_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let limit = slango::wgpu::device(&fixture.device)
        .limits()
        .max_compute_workgroups_per_dimension;
    let call = fixture.call_without("");
    let error = call
        .dispatch_groups([1, limit + 1, 1])
        .expect_err("over the limit");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        format!(
            "cannot call a kernel: the dispatch of the kernel `offset` needs {} thread groups along the y axis, and the device allows {limit}",
            limit + 1
        )
    );
    let groups = u32::MAX / 64 + 1;
    let error = call.dispatch([u32::MAX, 1, 1]).expect_err("over the limit");
    assert_eq!(
        error.to_string(),
        format!(
            "cannot call a kernel: the dispatch of the kernel `offset` needs {groups} thread groups along the x axis, and the device allows {limit}"
        )
    );
    assert_eq!(fixture.output.read()?, SENTINEL);
    Ok(())
}

#[test]
fn one_buffer_bound_as_read_only_and_as_read_write_is_a_backend_error() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("input");
    call.bind("input", &fixture.output);
    let error = call
        .dispatch([4, 1, 1])
        .expect_err("wgpu rejects the aliasing");
    assert_eq!(error.kind(), ErrorKind::Backend, "{error}");
    assert_eq!(
        error.to_string(),
        "cannot call a kernel: wgpu rejected the dispatch of the kernel `offset`"
    );
    println!(
        "wgpu reports: {:?}",
        error.source().map(ToString::to_string)
    );
    assert_eq!(fixture.output.read()?, SENTINEL);
    Ok(())
}
