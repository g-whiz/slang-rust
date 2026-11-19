use std::path::Path;
use std::rc::Rc;

use slang_compiler::{Component, EntryPoint};
use slang_rhi::CompileSession;

use super::device::Device;
use super::error::{Call, from_rhi};
use super::kernel::Kernel;
use super::render;
use crate::__private::EmbeddedSource;
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{self, Source};
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
/// session of the load, the modules that it loaded before the module, and
/// the module that it compiled.
#[derive(Clone)]
pub(crate) struct Module {
    _objects: Rc<Objects>,
}

struct Objects {
    _module: slang_compiler::Module,
    _imports: Vec<slang_compiler::Module>,
    _session: CompileSession,
}

impl Module {
    /// Compiles the module of `source` in a session of its own that
    /// reproduces the settings of the device's session, walks it for the
    /// device's target, and creates the compute pipeline of each kernel.
    pub(crate) fn load(device: &Device, source: &Source<'_>) -> Result<Loaded> {
        let search_paths: Vec<&Path> = source.search_path.into_iter().collect();
        let compile = session(device, &search_paths)?;
        Self::load_in(device, compile, Vec::new(), source)
    }

    /// Compiles the embedded sources in a session of their own that
    /// reproduces the settings of the device's session and has no search
    /// path, each under its module name in the given order, and loads the
    /// last as [`Module::load`] loads a source.
    pub(crate) fn load_embedded(device: &Device, embedded: &Embedded<'_>) -> Result<Loaded> {
        let Some((last, imports)) = embedded.sources.split_last() else {
            return Err(Error::new(ErrorKind::InvalidArgument, LOAD)
                .with_detail("the embedded module holds no Slang source".to_owned()));
        };
        let compile = session(device, &[])?;
        let imports = imports
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
        Self::load_in(device, compile, imports, &source)
    }

    /// Compiles the module of `source` in the session `compile`, which holds
    /// the modules `imports` already, walks it for the device's target, and
    /// creates the compute pipeline of each kernel.
    fn load_in(
        device: &Device,
        compile: CompileSession,
        imports: Vec<slang_compiler::Module>,
        source: &Source<'_>,
    ) -> Result<Loaded> {
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
        let (kernels, records) = layout::walk_program(
            &program,
            target,
            |at, snapshot, places| Ok((at.index, snapshot, places)),
            |_, name, stage| render::record(name, stage),
        )?;
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
                    Kernel::create(device, &compile, &module, entry_point, &snapshot, places)?;
                Ok((snapshot, kernel))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Loaded {
            module: Self {
                _objects: Rc::new(Objects {
                    _module: module,
                    _imports: imports,
                    _session: compile,
                }),
            },
            kernels,
            render: records,
        })
    }
}

/// Creates a Slang session for one load on `device` with the search paths.
fn session(device: &Device, search_paths: &[&Path]) -> Result<CompileSession> {
    device
        .rhi_device()
        .create_slang_session(search_paths)
        .map_err(|error| from_rhi(LOAD, Call::Other, error))
}
