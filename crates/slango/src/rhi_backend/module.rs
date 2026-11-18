use std::path::Path;
use std::rc::Rc;

use slang_compiler::{Component, EntryPoint};
use slang_rhi::CompileSession;

use super::device::Device;
use super::error::{Call, from_rhi};
use super::kernel::Kernel;
use super::render;
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{self, Compiled, Source};
use crate::module::Loaded;

const LOAD: &str = "load a Slang module";

/// The Slang objects of one load of a [`crate::Module`]: the compile
/// session of the load and the module that it compiled.
#[derive(Clone)]
pub(crate) struct Module {
    _objects: Rc<Objects>,
}

struct Objects {
    _module: slang_compiler::Module,
    _session: CompileSession,
}

impl Module {
    /// Compiles the module of `source` in a session of its own that
    /// reproduces the settings of the device's session, walks it for the
    /// device's target, and creates the compute pipeline of each kernel.
    pub(crate) fn load(device: &Device, source: &Source<'_>) -> Result<Loaded> {
        let search_paths: Vec<&Path> = source.search_path.into_iter().collect();
        let compile = device
            .rhi_device()
            .create_slang_session(&search_paths)
            .map_err(|error| from_rhi(LOAD, Call::Other, error))?;
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
                    _session: compile,
                }),
            },
            kernels,
            render: records,
        })
    }

    /// Fails, because this backend compiles every module from its Slang
    /// source for the device and loads no precompiled SPIR-V.
    pub(crate) fn load_compiled(_device: &Device, _compiled: Compiled) -> Result<Loaded> {
        Err(
            Error::new(ErrorKind::Unsupported, "load a precompiled module").with_detail(
                "slango with the feature `rhi` loads no precompiled module".to_owned(),
            ),
        )
    }
}
