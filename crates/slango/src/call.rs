use std::collections::HashSet;
use std::fmt;

use ::wgpu::util::DeviceExt as _;

use crate::buffer::Buffer;
use crate::device::scoped;
use crate::error::{Error, ErrorKind, Result};
use crate::kernel::Kernel;
use crate::layout::{KernelLayout, PlainValue, Resource, UniformBlock, Usage};
use crate::marshal::{self, Element, Marshalled, Value};
use crate::sampler::Sampler;
use crate::texture::{Texture, TextureFormat};

const CALL: &str = "call a kernel";

/// One call of a [`Kernel`], which collects the arguments of the kernel and
/// then runs it on the GPU.
///
/// Get one from [`Kernel::call`]. Give each parameter of the kernel by its
/// parameter path, the name of the parameter in the Slang source, with a full
/// stop between the name of a constant buffer or parameter block and the
/// name of a field, such as `params.scale`:
///
/// - [`Call::set`] sets a plain value, a scalar, vector, matrix, or array
///   that the kernel declares as `uniform`, or as a field of a
///   `ConstantBuffer`, `cbuffer`, or `ParameterBlock`.
/// - [`Call::bind`] binds a [`Buffer`] to a `StructuredBuffer` or
///   `RWStructuredBuffer`.
/// - [`Call::texture`] binds a [`Texture`] to a `Texture2D`.
/// - [`Call::sampler`] binds a [`Sampler`] to a `SamplerState`.
///
/// Every setter always succeeds, and a later setter for the same path
/// replaces the earlier one. [`Call::dispatch`] or [`Call::dispatch_groups`]
/// then checks every argument against the parameters of the kernel before
/// any work reaches the GPU, and runs the kernel. A call keeps its arguments,
/// so it can run the kernel again with the same arguments.
///
/// A call borrows its kernel, the paths, the buffers, the textures, and the
/// samplers, and is [`Send`]
/// and [`Sync`]. Each thread that runs a kernel at the same time as another
/// uses its own call.
///
/// # Examples
///
/// See the example of [`Kernel::call`].
pub struct Call<'a> {
    kernel: &'a Kernel,
    arguments: Vec<(&'a str, CallArgument<'a>)>,
}

/// An argument of a [`Call`], with the wgpu objects that it binds.
type CallArgument<'a> = Argument<&'a ::wgpu::Buffer, &'a ::wgpu::TextureView, &'a ::wgpu::Sampler>;

/// One argument of a call, with the handle `B` of a bound buffer, `V` of a
/// bound texture, and `S` of a bound sampler.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Argument<B, V, S> {
    Value(Marshalled),
    Buffer(Bound<B>),
    Texture(BoundTexture<V>),
    Sampler(BoundSampler<S>),
}

/// A buffer bound to a parameter, with the facts that the checks of a call
/// need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Bound<B> {
    pub(crate) handle: B,
    pub(crate) stride: usize,
    pub(crate) len: usize,
    pub(crate) on_device: bool,
}

/// A texture bound to a parameter, with the facts that the checks of a call
/// need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundTexture<V> {
    pub(crate) handle: V,
    pub(crate) format: TextureFormat,
    pub(crate) sampleable: bool,
    pub(crate) on_device: bool,
}

/// A sampler bound to a parameter, with the fact that the checks of a call
/// need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundSampler<S> {
    pub(crate) handle: S,
    pub(crate) on_device: bool,
}

/// What a dispatch binds: the entries of the bind group of each set, from
/// set 0 to the last set with a used binding, and the bytes of the
/// immediates, which are empty when the kernel has none.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Plan {
    pub(crate) groups: Vec<Vec<Entry>>,
    pub(crate) immediates: Vec<u8>,
}

/// One binding of a bind group of a [`Plan`].
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) index: u32,
    pub(crate) source: Source,
}

/// What a binding of a [`Plan`] holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Source {
    /// A uniform buffer that the dispatch creates with these bytes.
    Uniform(Vec<u8>),
    /// The buffer, texture, or sampler of the argument at this index.
    Argument(usize),
}

impl Kernel {
    /// Starts a call of the kernel, to which the returned [`Call`] adds the
    /// arguments before it runs the kernel.
    ///
    /// # Examples
    ///
    /// This example loads a kernel that scales the elements of a buffer by a
    /// factor, runs it over the four elements of a buffer, and reads the
    /// results back. It needs a GPU driver or a software driver at run time,
    /// so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "scale",
    ///     r#"
    ///         RWStructuredBuffer<float> data;
    ///         uniform float factor;
    ///         uniform uint count;
    ///
    ///         [shader("compute")]
    ///         [numthreads(64, 1, 1)]
    ///         void scale(uint3 id : SV_DispatchThreadID)
    ///         {
    ///             if (id.x < count) { data[id.x] *= factor; }
    ///         }
    ///     "#,
    /// )?;
    /// let scale = module.kernel("scale")?;
    /// let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    /// scale
    ///     .call()
    ///     .set("factor", 2.0_f32)
    ///     .set("count", 4_u32)
    ///     .bind("data", &data)
    ///     .dispatch([4, 1, 1])?;
    /// assert_eq!(data.read()?, [2.0, 4.0, 6.0, 8.0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn call(&self) -> Call<'_> {
        Call {
            kernel: self,
            arguments: Vec::new(),
        }
    }
}

impl<'a> Call<'a> {
    /// Sets the plain value at a parameter path to `value`.
    ///
    /// [`Value`] lists the Rust types that stand for the plain values of
    /// Slang. The dispatch checks that the value matches the Slang type of
    /// the parameter, and writes it where the kernel reads it.
    ///
    /// # Examples
    ///
    /// See the example of [`Kernel::call`].
    pub fn set<V: Value>(&mut self, path: &'a str, value: V) -> &mut Self {
        self.insert(path, Argument::Value(marshal::marshal(&value)));
        self
    }

    /// Binds a buffer to the buffer parameter at a parameter path.
    ///
    /// The dispatch checks that the buffer belongs to the device of the
    /// kernel, is not empty, and has elements of the size of the elements of
    /// the parameter.
    ///
    /// # Examples
    ///
    /// See the example of [`Kernel::call`].
    pub fn bind<T: Element>(&mut self, path: &'a str, buffer: &'a Buffer<T>) -> &mut Self {
        let bound = Bound {
            handle: buffer.wgpu_buffer(),
            stride: T::STRIDE,
            len: buffer.len(),
            on_device: buffer.is_on(&self.kernel.device),
        };
        self.insert(path, Argument::Buffer(bound));
        self
    }

    /// Binds a texture to the `Texture2D` parameter at a parameter path.
    ///
    /// The dispatch checks that the texture belongs to the device of the
    /// kernel and that a kernel can sample it, which a texture of the depth
    /// format [`TextureFormat::Depth32Float`] cannot.
    ///
    /// # Examples
    ///
    /// This example samples the pixel of a texture of two by one pixels at
    /// the coordinates (0.75, 0.5), the centre of its right pixel, and writes
    /// it to a buffer. It needs a GPU driver or a software driver at run
    /// time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions, Sampler, SamplerOptions, Texture};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "pick",
    ///     r#"
    ///         Texture2D<float4> image;
    ///         SamplerState nearest;
    ///         RWStructuredBuffer<float4> output;
    ///
    ///         [shader("compute")]
    ///         [numthreads(1, 1, 1)]
    ///         void pick() { output[0] = image.SampleLevel(nearest, float2(0.75, 0.5), 0); }
    ///     "#,
    /// )?;
    /// let image = Texture::from_pixels(&device, [2, 1], &[[0, 0, 0, 255], [255, 0, 0, 255]])?;
    /// let nearest = Sampler::new(&device, &SamplerOptions::default())?;
    /// let output = Buffer::<[f32; 4]>::zeroed(&device, 1)?;
    /// module
    ///     .kernel("pick")?
    ///     .call()
    ///     .texture("image", &image)
    ///     .sampler("nearest", &nearest)
    ///     .bind("output", &output)
    ///     .dispatch([1, 1, 1])?;
    /// assert_eq!(output.read()?, [[1.0, 0.0, 0.0, 1.0]]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn texture(&mut self, path: &'a str, texture: &'a Texture) -> &mut Self {
        let bound = BoundTexture {
            handle: texture.wgpu_view(),
            format: texture.format(),
            sampleable: texture.is_sampleable(),
            on_device: texture.is_on(&self.kernel.device),
        };
        self.insert(path, Argument::Texture(bound));
        self
    }

    /// Binds a sampler to the `SamplerState` parameter at a parameter path.
    ///
    /// The dispatch checks that the sampler belongs to the device of the
    /// kernel.
    ///
    /// # Examples
    ///
    /// See the example of [`Call::texture`].
    pub fn sampler(&mut self, path: &'a str, sampler: &'a Sampler) -> &mut Self {
        let bound = BoundSampler {
            handle: sampler.wgpu_sampler(),
            on_device: sampler.is_on(&self.kernel.device),
        };
        self.insert(path, Argument::Sampler(bound));
        self
    }

    /// Runs the kernel on at least as many threads as `elements` gives
    /// along each of the three axes.
    ///
    /// The kernel runs in whole thread groups, so the dispatch rounds the
    /// number of thread groups along each axis up, and the kernel must
    /// ignore the threads past the end itself, such as with
    /// `if (id.x < count)`. A count of zero along any axis runs no thread,
    /// after the arguments have been checked.
    ///
    /// The dispatch submits the work to the GPU and returns without waiting
    /// for it. [`Buffer::read`] waits for it and returns the results.
    ///
    /// # Examples
    ///
    /// See the example of [`Kernel::call`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of these kinds, before any work reaches the GPU
    /// unless the kind is [`ErrorKind::Backend`]:
    ///
    /// - [`ErrorKind::InvalidArgument`] when a path names no parameter of
    ///   the kernel, a parameter that the kernel uses is not given, a value
    ///   does not match the Slang type of its parameter, an argument of one
    ///   kind is given for a parameter of another, such as a value for a
    ///   buffer parameter or a sampler for a texture, a buffer, texture, or
    ///   sampler belongs to another device, a buffer is empty or has elements
    ///   of another size than the parameter, a texture has the depth format
    ///   or cannot be sampled, or the number of thread groups along an axis
    ///   exceeds the limit of the device.
    /// - [`ErrorKind::UnsupportedParameter`] when the reflected layout of a
    ///   plain value does not hold the value.
    /// - [`ErrorKind::Backend`] when wgpu reports an error while it prepares
    ///   or submits the work, such as when two parameters that the kernel
    ///   writes or reads share one buffer in a way that wgpu forbids.
    pub fn dispatch(&self, elements: [u32; 3]) -> Result<()> {
        let groups =
            workgroup_counts(elements, self.kernel.layout.thread_group_size).ok_or_else(|| {
                Error::new(ErrorKind::InvalidArgument, CALL).with_detail(format!(
                    "the kernel `{}` has a thread group size of zero along an axis",
                    self.kernel.layout.name
                ))
            })?;
        self.dispatch_groups(groups)
    }

    /// Runs the kernel in `groups` thread groups along each of the three
    /// axes, each of the size that [`Kernel::thread_group_size`] returns.
    ///
    /// It works as [`Call::dispatch`] does, without the rounding.
    ///
    /// # Examples
    ///
    /// This example runs a kernel of thread groups of 64 threads in two
    /// thread groups, over 128 elements. It needs a GPU driver or a software
    /// driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "fill",
    ///     r#"
    ///         RWStructuredBuffer<uint> data;
    ///
    ///         [shader("compute")]
    ///         [numthreads(64, 1, 1)]
    ///         void fill(uint3 id : SV_DispatchThreadID) { data[id.x] = id.x; }
    ///     "#,
    /// )?;
    /// let data = Buffer::<u32>::zeroed(&device, 128)?;
    /// module.kernel("fill")?.call().bind("data", &data).dispatch_groups([2, 1, 1])?;
    /// assert_eq!(data.read()?, (0..128).collect::<Vec<u32>>());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of the kinds that [`Call::dispatch`] returns.
    pub fn dispatch_groups(&self, groups: [u32; 3]) -> Result<()> {
        let kernel = self.kernel;
        let plan = plan(&kernel.layout, &self.arguments)?;
        let device = kernel.device.wgpu_device();
        check_workgroups(
            &kernel.layout.name,
            groups,
            device.limits().max_compute_workgroups_per_dimension,
        )?;
        let ((), error) = scoped(device, || self.submit(&plan, groups));
        match error {
            Some(error) => Err(Error::new(ErrorKind::Backend, CALL)
                .with_detail(format!(
                    "wgpu rejected the dispatch of the kernel `{}`",
                    kernel.layout.name
                ))
                .with_source(error)),
            None => Ok(()),
        }
    }

    fn insert(&mut self, path: &'a str, argument: CallArgument<'a>) {
        match self.arguments.iter_mut().find(|(given, _)| *given == path) {
            Some((_, slot)) => *slot = argument,
            None => self.arguments.push((path, argument)),
        }
    }

    fn submit(&self, plan: &Plan, groups: [u32; 3]) {
        let kernel = self.kernel;
        let device = kernel.device.wgpu_device();
        let name = kernel.layout.name.as_str();
        let uniforms: Vec<Vec<Option<::wgpu::Buffer>>> = plan
            .groups
            .iter()
            .map(|entries| {
                entries
                    .iter()
                    .map(|entry| match &entry.source {
                        Source::Uniform(bytes) => Some(device.create_buffer_init(
                            &::wgpu::util::BufferInitDescriptor {
                                label: Some(name),
                                contents: bytes,
                                usage: ::wgpu::BufferUsages::UNIFORM,
                            },
                        )),
                        Source::Argument(_) => None,
                    })
                    .collect()
            })
            .collect();
        let bind_groups: Vec<::wgpu::BindGroup> = plan
            .groups
            .iter()
            .zip(&uniforms)
            .zip(kernel.bind_group_layouts.iter())
            .map(|((entries, uniforms), layout)| {
                let entries: Vec<::wgpu::BindGroupEntry<'_>> = entries
                    .iter()
                    .zip(uniforms)
                    .filter_map(|(entry, uniform)| {
                        let resource = match (&entry.source, uniform) {
                            (Source::Uniform(_), Some(buffer)) => buffer.as_entire_binding(),
                            (Source::Argument(index), _) => self.resource_of(*index)?,
                            (Source::Uniform(_), None) => return None,
                        };
                        Some(::wgpu::BindGroupEntry {
                            binding: entry.index,
                            resource,
                        })
                    })
                    .collect();
                device.create_bind_group(&::wgpu::BindGroupDescriptor {
                    label: Some(name),
                    layout,
                    entries: &entries,
                })
            })
            .collect();
        let mut encoder =
            device.create_command_encoder(&::wgpu::CommandEncoderDescriptor { label: Some(name) });
        {
            let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
                label: Some(name),
                timestamp_writes: None,
            });
            pass.set_pipeline(&kernel.pipeline);
            for (set, group) in (0_u32..).zip(&bind_groups) {
                pass.set_bind_group(set, group, &[]);
            }
            if !plan.immediates.is_empty() {
                pass.set_immediates(0, &plan.immediates);
            }
            pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
        }
        kernel.device.wgpu_queue().submit([encoder.finish()]);
    }

    fn resource_of(&self, index: usize) -> Option<::wgpu::BindingResource<'a>> {
        match self.arguments.get(index) {
            Some((_, Argument::Buffer(bound))) => Some(bound.handle.as_entire_binding()),
            Some((_, Argument::Texture(bound))) => {
                Some(::wgpu::BindingResource::TextureView(bound.handle))
            }
            Some((_, Argument::Sampler(bound))) => {
                Some(::wgpu::BindingResource::Sampler(bound.handle))
            }
            Some((_, Argument::Value(_))) | None => None,
        }
    }
}

impl fmt::Debug for Call<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paths: Vec<&str> = self.arguments.iter().map(|(path, _)| *path).collect();
        f.debug_struct("Call")
            .field("kernel", &self.kernel.layout.name)
            .field("arguments", &paths)
            .finish_non_exhaustive()
    }
}

/// Returns the number of thread groups along each axis that cover
/// `elements` threads in groups of `group_size`, rounded up, or `None` when
/// a group size is zero.
///
/// The rounding cannot overflow, because the result never exceeds
/// `elements`.
pub(crate) fn workgroup_counts(elements: [u32; 3], group_size: [u32; 3]) -> Option<[u32; 3]> {
    let mut groups = [0; 3];
    for ((count, &elements), &size) in groups.iter_mut().zip(&elements).zip(&group_size) {
        let whole = elements.checked_div(size)?;
        *count = if elements.checked_rem(size)? == 0 {
            whole
        } else {
            whole.checked_add(1)?
        };
    }
    Some(groups)
}

/// Fails with [`ErrorKind::InvalidArgument`] when the number of thread
/// groups along an axis exceeds the device's `limit`.
pub(crate) fn check_workgroups(kernel: &str, groups: [u32; 3], limit: u32) -> Result<()> {
    for (axis, count) in ["x", "y", "z"].into_iter().zip(groups) {
        if count > limit {
            return Err(
                Error::new(ErrorKind::InvalidArgument, CALL).with_detail(format!(
                    "the dispatch of the kernel `{kernel}` needs {count} thread groups along the {axis} axis, and the device allows {limit}"
                )),
            );
        }
    }
    Ok(())
}

/// Where a parameter path leads in the layout snapshot of a kernel.
#[derive(Clone, Copy)]
enum Target<'l> {
    /// The storage buffer at this index of the bindings.
    Buffer(usize),
    /// The texture at this index of the bindings.
    Texture(usize),
    /// The sampler at this index of the bindings.
    Sampler(usize),
    /// A plain value of the uniform buffer at this index of the bindings.
    Uniform(usize, &'l PlainValue),
    /// A plain value of the immediates.
    Immediate(&'l PlainValue),
}

/// Checks the arguments of a call against the layout snapshot of its kernel
/// and returns what the dispatch binds.
///
/// It fails with [`ErrorKind::InvalidArgument`] on the first argument, in
/// the order of the arguments, whose path names no parameter, whose kind
/// differs from that of its parameter, whose value does not match, whose
/// buffer, texture, or sampler belongs to another device, whose buffer is
/// empty or has another stride, or whose texture cannot be sampled. It then
/// fails on the first parameter, in the order of the bindings and then the
/// immediates, that the kernel uses and that no argument gives. An argument
/// for a parameter that the kernel does not use is checked and then
/// ignored.
pub(crate) fn plan<B, V, S>(
    layout: &KernelLayout,
    arguments: &[(&str, Argument<B, V, S>)],
) -> Result<Plan> {
    let mut blocks: Vec<Option<Vec<u8>>> = layout
        .bindings
        .iter()
        .map(|binding| match &binding.resource {
            Resource::UniformBuffer(block) => Some(zeroed(block)),
            Resource::StorageBuffer { .. } | Resource::SampledTexture | Resource::Sampler => None,
        })
        .collect();
    let mut immediates = layout.immediates.as_ref().map(zeroed).unwrap_or_default();
    let mut given: Vec<Option<usize>> = vec![None; layout.bindings.len()];
    let mut set: HashSet<&str> = HashSet::new();
    let name = layout.name.as_str();
    for (index, (path, argument)) in arguments.iter().enumerate() {
        let path = *path;
        let target = find(layout, path).ok_or_else(|| {
            invalid(format!(
                "the kernel `{name}` has no parameter at the path `{path}`"
            ))
        })?;
        let binding = match target {
            Target::Buffer(binding) => {
                let Argument::Buffer(bound) = argument else {
                    return Err(mismatch(name, path, "a buffer", argument));
                };
                check_buffer(layout, binding, path, bound)?;
                binding
            }
            Target::Texture(binding) => {
                let Argument::Texture(bound) = argument else {
                    return Err(mismatch(name, path, "a texture", argument));
                };
                check_texture(name, path, bound)?;
                binding
            }
            Target::Sampler(binding) => {
                let Argument::Sampler(bound) = argument else {
                    return Err(mismatch(name, path, "a sampler", argument));
                };
                check_on_device(name, path, "sampler", bound.on_device)?;
                binding
            }
            Target::Uniform(binding, plain) => {
                let Argument::Value(value) = argument else {
                    return Err(mismatch(name, path, "a plain value", argument));
                };
                let block = blocks
                    .get_mut(binding)
                    .and_then(Option::as_mut)
                    .ok_or_else(|| internal(name, path))?;
                write(name, path, value, plain, block)?;
                set.insert(path);
                continue;
            }
            Target::Immediate(plain) => {
                let Argument::Value(value) = argument else {
                    return Err(mismatch(name, path, "a plain value", argument));
                };
                write(name, path, value, plain, &mut immediates)?;
                set.insert(path);
                continue;
            }
        };
        if let Some(slot) = given.get_mut(binding) {
            *slot = Some(index);
        }
    }
    require_arguments(layout, &given, &set)?;
    let groups = group_sets(layout)
        .into_iter()
        .map(|indexes| {
            indexes
                .into_iter()
                .map(|binding| {
                    let index = layout
                        .bindings
                        .get(binding)
                        .map(|binding| binding.index)
                        .ok_or_else(|| internal(name, ""))?;
                    let source = match given.get(binding).copied().flatten() {
                        Some(argument) => Source::Argument(argument),
                        None => Source::Uniform(
                            blocks
                                .get_mut(binding)
                                .and_then(Option::take)
                                .ok_or_else(|| internal(name, ""))?,
                        ),
                    };
                    Ok(Entry { index, source })
                })
                .collect::<Result<Vec<Entry>>>()
        })
        .collect::<Result<Vec<Vec<Entry>>>>()?;
    Ok(Plan { groups, immediates })
}

/// Fails with [`ErrorKind::InvalidArgument`] on the first parameter, in the
/// order of the bindings and then the immediates, that the kernel uses and
/// that no argument gives, where `given` holds the argument of each binding
/// and `set` the paths of the plain values that the arguments set.
fn require_arguments(
    layout: &KernelLayout,
    given: &[Option<usize>],
    set: &HashSet<&str>,
) -> Result<()> {
    let name = layout.name.as_str();
    for (binding, given) in layout.bindings.iter().zip(given) {
        if binding.usage == Usage::Unused {
            continue;
        }
        let what = match &binding.resource {
            Resource::StorageBuffer { .. } => "buffer",
            Resource::SampledTexture => "texture",
            Resource::Sampler => "sampler",
            Resource::UniformBuffer(block) => {
                require_values(name, block, set)?;
                continue;
            }
        };
        if given.is_none() {
            return Err(invalid(format!(
                "no {what} is bound to the parameter `{}` of the kernel `{name}`",
                binding.path
            )));
        }
    }
    if let Some(block) = &layout.immediates {
        require_values(name, block, set)?;
    }
    Ok(())
}

/// Returns the indexes of the used bindings of each set, from set 0 to the
/// last set with a used binding, which are the bind group layouts that the
/// load created.
fn group_sets(layout: &KernelLayout) -> Vec<Vec<usize>> {
    let used = || {
        layout
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| binding.usage == Usage::Used)
    };
    let Some(last_set) = used().map(|(_, binding)| binding.set).max() else {
        return Vec::new();
    };
    (0..=last_set)
        .map(|set| {
            used()
                .filter(|(_, binding)| binding.set == set)
                .map(|(index, _)| index)
                .collect()
        })
        .collect()
}

fn find<'l>(layout: &'l KernelLayout, path: &str) -> Option<Target<'l>> {
    for (index, binding) in layout.bindings.iter().enumerate() {
        match &binding.resource {
            Resource::StorageBuffer { .. } if binding.path == path => {
                return Some(Target::Buffer(index));
            }
            Resource::SampledTexture if binding.path == path => {
                return Some(Target::Texture(index));
            }
            Resource::Sampler if binding.path == path => {
                return Some(Target::Sampler(index));
            }
            Resource::StorageBuffer { .. } | Resource::SampledTexture | Resource::Sampler => {}
            Resource::UniformBuffer(block) => {
                if let Some(plain) = block.values.iter().find(|plain| plain.path == path) {
                    return Some(Target::Uniform(index, plain));
                }
            }
        }
    }
    layout
        .immediates
        .as_ref()?
        .values
        .iter()
        .find(|plain| plain.path == path)
        .map(Target::Immediate)
}

fn check_buffer<B>(
    layout: &KernelLayout,
    binding: usize,
    path: &str,
    bound: &Bound<B>,
) -> Result<()> {
    let name = layout.name.as_str();
    let Some(Resource::StorageBuffer { stride, .. }) = layout
        .bindings
        .get(binding)
        .map(|binding| &binding.resource)
    else {
        return Err(internal(name, path));
    };
    check_on_device(name, path, "buffer", bound.on_device)?;
    if bound.len == 0 {
        return Err(invalid(format!(
            "the buffer bound to the parameter `{path}` of the kernel `{name}` is empty"
        )));
    }
    if !u64::try_from(bound.stride).is_ok_and(|given| given == *stride) {
        return Err(invalid(format!(
            "the parameter `{path}` of the kernel `{name}` has elements of {stride} bytes, and the bound buffer has elements of {} bytes",
            bound.stride
        )));
    }
    Ok(())
}

fn check_texture<V>(name: &str, path: &str, bound: &BoundTexture<V>) -> Result<()> {
    check_on_device(name, path, "texture", bound.on_device)?;
    if bound.format.is_depth() {
        return Err(invalid(format!(
            "the texture bound to the parameter `{path}` of the kernel `{name}` has the depth format {:?}, which a kernel cannot sample",
            bound.format
        )));
    }
    if !bound.sampleable {
        return Err(invalid(format!(
            "the texture bound to the parameter `{path}` of the kernel `{name}` lacks the usage TEXTURE_BINDING, which a kernel needs to sample it"
        )));
    }
    Ok(())
}

fn check_on_device(name: &str, path: &str, what: &str, on_device: bool) -> Result<()> {
    if on_device {
        return Ok(());
    }
    Err(invalid(format!(
        "the {what} bound to the parameter `{path}` of the kernel `{name}` belongs to another device"
    )))
}

/// Returns the error of an argument whose kind differs from that of its
/// parameter, which is `parameter`, such as "a buffer".
fn mismatch<B, V, S>(
    name: &str,
    path: &str,
    parameter: &str,
    argument: &Argument<B, V, S>,
) -> Error {
    let given = match argument {
        Argument::Value(_) => "sets a value for it",
        Argument::Buffer(_) => "binds a buffer to it",
        Argument::Texture(_) => "binds a texture to it",
        Argument::Sampler(_) => "binds a sampler to it",
    };
    invalid(format!(
        "the parameter `{path}` of the kernel `{name}` is {parameter}, and the call {given}"
    ))
}

fn require_values(name: &str, block: &UniformBlock, set: &HashSet<&str>) -> Result<()> {
    match block
        .values
        .iter()
        .find(|plain| !set.contains(plain.path.as_str()))
    {
        Some(plain) => Err(invalid(format!(
            "the parameter `{}` of the kernel `{name}` is not set",
            plain.path
        ))),
        None => Ok(()),
    }
}

fn write(
    name: &str,
    path: &str,
    value: &Marshalled,
    plain: &PlainValue,
    block: &mut [u8],
) -> Result<()> {
    marshal::write(path, value, plain, block).map_err(|error| {
        Error::new(error.kind(), CALL)
            .with_detail(format!(
                "the kernel `{name}` cannot take the value of the parameter `{path}`"
            ))
            .with_source(error)
    })
}

/// Returns a zeroed block of the reflected size of `block`, or an empty one
/// when the size does not fit in memory, which no reflected layout reaches.
fn zeroed(block: &UniformBlock) -> Vec<u8> {
    vec![0; usize::try_from(block.size).unwrap_or(0)]
}

fn invalid(detail: String) -> Error {
    Error::new(ErrorKind::InvalidArgument, CALL).with_detail(detail)
}

fn internal(name: &str, path: &str) -> Error {
    Error::new(ErrorKind::UnsupportedParameter, CALL).with_detail(format!(
        "internal error: the layout of the kernel `{name}` lost the parameter `{path}`"
    ))
}

#[cfg(test)]
mod tests;
