use std::convert::Infallible;

use super::device::Device;
use crate::call::Subject;
use crate::draw::Recording;
use crate::error::{ErrorKind, Result};
use crate::layout::{CompiledRender, Rejection, RenderStage, UsableRender};
use crate::render::{Names, PipelineSnapshot, stage_word};

/// The render pipeline of a [`crate::RenderPipeline`], which this backend
/// never creates, so no value of the type exists.
#[derive(Clone)]
pub(crate) struct RenderPipeline {
    never: Infallible,
}

impl RenderPipeline {
    pub(crate) fn create(
        _device: &Device,
        _vertex_record: &UsableRender,
        _fragment_record: &UsableRender,
        names: Names<'_>,
        _snapshot: &PipelineSnapshot,
    ) -> Result<Self> {
        Err(names.error(ErrorKind::Unsupported, NO_PIPELINES))
    }

    pub(crate) fn draw(
        &self,
        _device: &Device,
        _subject: Subject<'_>,
        recording: &Recording<'_, '_>,
    ) -> Result<()> {
        let Recording {
            label: _label,
            bindings: _bindings,
            plan: _plan,
            arguments: _arguments,
            vertices: _vertices,
            target: _target,
            clear: _clear,
            depth: _depth,
            clear_depth: _clear_depth,
            indices: _indices,
            count: _count,
            instances: _instances,
        } = recording;
        match self.never {}
    }
}

const NO_PIPELINES: &str = "slango with the feature `rhi` creates no render pipelines";

/// Returns the record of the vertex or fragment entry point `name` of a
/// loaded module, which a render pipeline cannot use on this backend.
pub(super) fn record(name: String, stage: RenderStage) -> CompiledRender {
    let detail = format!(
        "the {} entry point `{name}` cannot be used, because {NO_PIPELINES}",
        stage_word(stage)
    );
    CompiledRender {
        name,
        stage,
        usable: Err(Rejection {
            kind: ErrorKind::Unsupported,
            detail,
            source: None,
        }),
    }
}
