use std::path::Path;

use slang_compiler::{
    CompilerOptions, Component, EntryPoint, GlobalSession, MatrixLayout, Session, Target,
    TargetFormat, Toggle,
};

use crate::__private::EmbeddedSource;
use crate::error::{Error, Result};
use crate::layout::{self, CompiledRender, RenderStage, Source, Usage};

/// The usage of each vertex attribute of one vertex entry point, by the
/// name of the entry point and the path of each attribute.
pub(super) type AttributeUsage = (String, Vec<(String, Usage)>);

/// The Slang source of a load: the modules that it loads first, in their
/// order, the search paths, and the module itself.
pub(super) struct LoadSource<'s> {
    pub(super) imports: &'s [EmbeddedSource<'s>],
    pub(super) search_paths: &'s [&'s Path],
    pub(super) source: &'s Source<'s>,
}

/// Replaces the usage of the vertex attributes of each usable vertex record
/// among `records` with the usage that the metadata of SPIR-V reports, for a
/// device whose target is not SPIR-V.
///
/// The metadata of DXIL reports every declared vertex attribute as used,
/// because an attribute that the shader does not read stays a parameter of
/// the entry point there, while SPIR-V removes it. A draw gives no buffer
/// for an unused attribute, so the usage comes from a compile for SPIR-V
/// in a session of its own that defines the macro of the device's session.
///
/// When that compile fails, the records keep the usage of the device's
/// target, with which a draw needs a buffer for every declared attribute.
pub(super) fn use_spirv_usage(
    global: &GlobalSession,
    define: &str,
    load: &LoadSource<'_>,
    records: &mut [CompiledRender],
) {
    let names: Vec<&str> = records
        .iter()
        .filter(|record| {
            record.stage == RenderStage::Vertex
                && record
                    .usable
                    .as_ref()
                    .is_ok_and(|usable| !usable.layout.attributes.is_empty())
        })
        .map(|record| record.name.as_str())
        .collect();
    if names.is_empty() {
        return;
    }
    if let Ok(usage) = spirv_usage(global, define, load, &names) {
        apply(records, &usage);
    }
}

/// Compiles the source of `load` with the vertex entry points named `names`
/// for SPIR-V, in a session that defines `define` as `1` and has the
/// settings of a session of slang-rhi, and returns the usage of the vertex
/// attributes of each of those entry points whose walk succeeds.
pub(super) fn spirv_usage(
    global: &GlobalSession,
    define: &str,
    load: &LoadSource<'_>,
    names: &[&str],
) -> Result<Vec<AttributeUsage>> {
    let mut target_options = CompilerOptions::default();
    target_options.glsl_force_scalar_layout(Toggle::On);
    let mut builder = Session::builder(global)
        .target(Target::new(TargetFormat::Spirv).options(target_options))
        .define(define, "1")
        .matrix_layout(MatrixLayout::RowMajor);
    for path in load.search_paths {
        builder = builder.search_path(path);
    }
    let session = builder.build().map_err(Error::compile)?;
    let mut imports = Vec::with_capacity(load.imports.len());
    for import in load.imports {
        imports.push(
            session
                .load_module_from_source(import.name, import.path, import.text)
                .map_err(Error::compile)?
                .into_value(),
        );
    }
    let source = load.source;
    let module = session
        .load_module_from_source(source.name, source.path, source.text)
        .map_err(Error::compile)?
        .into_value();
    let entry_points = names
        .iter()
        .map(|name| module.entry_point(name).map_err(Error::compile))
        .collect::<Result<Vec<EntryPoint>>>()?;
    let mut components = vec![Component::Module(&module)];
    components.extend(entry_points.iter().map(Component::EntryPoint));
    let program = session
        .compose(&components)
        .map_err(Error::compile)?
        .into_value()
        .link()
        .map_err(Error::compile)?
        .into_value();
    let target = session.target_index(0).map_err(Error::compile)?;
    let (_, records) = layout::walk_program(
        &program,
        target,
        |_, _, _| Ok(()),
        |at, name, stage| layout::render_entry(at, name, stage, None, |_, _| Ok(Vec::new())).0,
    )?;
    Ok(records
        .into_iter()
        .filter_map(|record| {
            let usable = record.usable.ok()?;
            let attributes = usable
                .layout
                .attributes
                .into_iter()
                .map(|attribute| (attribute.path, attribute.usage))
                .collect();
            Some((record.name, attributes))
        })
        .collect())
}

/// Replaces the usage of each attribute of the usable vertex records among
/// `records` that `usage` names by its entry point and its path.
pub(super) fn apply(records: &mut [CompiledRender], usage: &[AttributeUsage]) {
    for record in records {
        if record.stage != RenderStage::Vertex {
            continue;
        }
        let Some((_, attributes)) = usage.iter().find(|(name, _)| *name == record.name) else {
            continue;
        };
        let Ok(usable) = record.usable.as_mut() else {
            continue;
        };
        for attribute in &mut usable.layout.attributes {
            if let Some((_, found)) = attributes.iter().find(|(path, _)| *path == attribute.path) {
                attribute.usage = *found;
            }
        }
    }
}

#[cfg(test)]
mod tests;
