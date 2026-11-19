use std::path::Path;
use std::rc::Rc;

use slang_compiler::{Component, EntryPoint};
use slang_rhi::{CompileSession, DeviceType};

use super::device::Device;
use super::error::{Call, from_rhi};
use super::kernel::Kernel;
use super::render::no_semantic_problem;
use super::varying::{self, LoadSource};
use crate::__private::EmbeddedSource;
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{self, Place, RenderStage, Semantic, Source};
use crate::module::Loaded;

const LOAD: &str = "load a Slang module";

/// A module that generated code embeds for the backend rhi: the Slang source
/// of the module and of every module that it imports, each after the modules
/// that it imports, with the module itself last.
#[expect(
    clippy::exhaustive_structs,
    reason = "the hidden module has no stability promise, and generated code builds it with a literal"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Embedded<'a> {
    /// The sources of the modules, in the order in which a load compiles
    /// them.
    pub sources: &'a [EmbeddedSource<'a>],
}

/// The Slang objects of one load of a [`crate::Module`]: the compile
/// session of the load, the modules that it loaded before the module, the
/// module that it compiled, and its vertex and fragment entry points, from
/// which a render pipeline is created after the load.
#[derive(Clone)]
pub(crate) struct Module {
    objects: Rc<Objects>,
}

struct Objects {
    module: slang_compiler::Module,
    _imports: Vec<slang_compiler::Module>,
    render: Vec<RenderEntryPoint>,
    session: CompileSession,
}

/// A vertex or fragment entry point of a loaded module whose record is
/// usable, with the facts that a render pipeline needs beside the record.
pub(super) struct RenderEntryPoint {
    pub(super) name: String,
    pub(super) stage: RenderStage,
    pub(super) entry_point: EntryPoint,
    /// The place of each binding of the record, in the order of its
    /// bindings.
    pub(super) places: Vec<Place>,
    /// The semantic of each vertex attribute of the record, in the order of
    /// its attributes.
    pub(super) semantics: Vec<Option<Semantic>>,
}

impl Module {
    /// Compiles the module of `source` in a session of its own that
    /// reproduces the settings of the device's session, walks it for the
    /// device's target, and creates the compute pipeline of each kernel.
    pub(crate) fn load(device: &Device, source: &Source<'_>) -> Result<Loaded> {
        let search_paths: Vec<&Path> = source.search_path.into_iter().collect();
        let compile = session(device, &search_paths)?;
        let load = LoadSource {
            imports: &[],
            search_paths: &search_paths,
            source,
        };
        Self::load_in(device, compile, Vec::new(), &load)
    }

    /// Compiles the embedded sources in a session of their own that
    /// reproduces the settings of the device's session and has no search
    /// path, each under its module name in the given order, and loads the
    /// last as [`Module::load`] loads a source.
    pub(crate) fn load_embedded(device: &Device, embedded: &Embedded<'_>) -> Result<Loaded> {
        let Some((last, embedded_imports)) = embedded.sources.split_last() else {
            return Err(Error::new(ErrorKind::InvalidArgument, LOAD)
                .with_detail("the embedded module holds no Slang source".to_owned()));
        };
        let compile = session(device, &[])?;
        let imports = embedded_imports
            .iter()
            .map(|import| {
                compile
                    .session()
                    .load_module_from_source(import.name, import.path, import.text)
                    .map_err(Error::compile)
                    .map(slang_compiler::Compiled::into_value)
            })
            .collect::<Result<Vec<_>>>()?;
        let source = Source {
            name: last.name,
            path: last.path,
            text: last.text,
            search_path: None,
        };
        let load = LoadSource {
            imports: embedded_imports,
            search_paths: &[],
            source: &source,
        };
        Self::load_in(device, compile, imports, &load)
    }

    /// Compiles the module of the source of `load` in the session
    /// `compile`, which holds the modules `imports` of its imports already,
    /// walks it for the device's target, and creates the compute pipeline of
    /// each kernel.
    ///
    /// On a device of Direct3D 12, the usage of each vertex attribute is the
    /// one that a compile of the source for SPIR-V reports, because the
    /// metadata of DXIL reports every declared vertex attribute as used.
    fn load_in(
        device: &Device,
        compile: CompileSession,
        imports: Vec<slang_compiler::Module>,
        load: &LoadSource<'_>,
    ) -> Result<Loaded> {
        let source = load.source;
        let session = compile.session();
        let target = session.target_index(0).map_err(Error::compile)?;
        let module = session
            .load_module_from_source(source.name, source.path, source.text)
            .map_err(Error::compile)?
            .into_value();
        let entry_points = module
            .entry_points()
            .map_err(Error::compile)?
            .collect::<slang_compiler::Result<Vec<EntryPoint>>>()
            .map_err(Error::compile)?;
        let mut components = vec![Component::Module(&module)];
        components.extend(entry_points.iter().map(Component::EntryPoint));
        let program = session
            .compose(&components)
            .map_err(Error::compile)?
            .into_value()
            .link()
            .map_err(Error::compile)?
            .into_value();
        let mut facts = Vec::new();
        let (kernels, mut records) = layout::walk_program(
            &program,
            target,
            |at, snapshot, places| Ok((at.index, snapshot, places)),
            |at, name, stage| {
                let (record, places, semantics) =
                    layout::render_entry(at, name, stage, Some(no_semantic_problem), |_, _| {
                        Ok(Vec::new())
                    });
                if record.usable.is_ok() {
                    facts.push((at.index, record.name.clone(), stage, places, semantics));
                }
                record
            },
        )?;
        if device.info().device_type() == DeviceType::D3d12 {
            varying::use_spirv_usage(device.global_session(), "__D3D12__", load, &mut records);
        }
        let render = facts
            .into_iter()
            .map(|(index, name, stage, places, semantics)| {
                let entry_point = entry_points.get(index).cloned().ok_or_else(|| {
                    Error::new(ErrorKind::Compile, LOAD).with_detail(format!(
                        "Slang reports the entry point `{name}` at a position past the entry points of the module"
                    ))
                })?;
                Ok(RenderEntryPoint {
                    name,
                    stage,
                    entry_point,
                    places,
                    semantics,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let kernels = kernels
            .into_iter()
            .map(|(index, snapshot, places)| {
                let entry_point = entry_points.get(index).ok_or_else(|| {
                    Error::new(ErrorKind::Compile, LOAD).with_detail(format!(
                        "Slang reports the kernel `{}` at a position past the entry points of the module",
                        snapshot.name
                    ))
                })?;
                let kernel =
                    Kernel::create(device, &compile, &module, entry_point, &snapshot, &places)?;
                Ok((snapshot, kernel))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Loaded {
            module: Self {
                objects: Rc::new(Objects {
                    module,
                    _imports: imports,
                    render,
                    session: compile,
                }),
            },
            kernels,
            render: records,
        })
    }
}

impl Module {
    /// Returns the usable vertex or fragment entry point of the stage
    /// `stage` named `name`.
    pub(super) fn render_entry_point(
        &self,
        name: &str,
        stage: RenderStage,
    ) -> Option<&RenderEntryPoint> {
        self.objects
            .render
            .iter()
            .find(|entry| entry.stage == stage && entry.name == name)
    }

    pub(super) fn slang_module(&self) -> &slang_compiler::Module {
        &self.objects.module
    }

    pub(super) fn compile_session(&self) -> &CompileSession {
        &self.objects.session
    }
}

/// Creates a Slang session for one load on `device` with the search paths.
fn session(device: &Device, search_paths: &[&Path]) -> Result<CompileSession> {
    device
        .rhi_device()
        .create_slang_session(search_paths)
        .map_err(|error| from_rhi(LOAD, Call::Other, error))
}
