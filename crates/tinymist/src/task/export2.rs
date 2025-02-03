#![allow(missing_docs)]

use std::sync::Arc;

use reflexo_typst::{CompilerFeat, EntryReader};
use tinymist_project::{
    CompileSnapshot, HtmlExport, LspCompilerFeat, PdfExport, PdfFlag, PngExport, SvgExport,
    TaskWhen,
};
use tinymist_std::error::prelude::*;
use tinymist_std::typst::{TypstDocument, TypstHtmlDocument, TypstPagedDocument};
use tinymist_task::{
    ErasedHtmlExport, ErasedPdfExport, ErasedPngExport, ErasedSvgExport, HtmlFlag, PngFlag, SvgFlag,
};
use typlite::Typlite;
use typst::diag::SourceResult;

use crate::project::{ExportMarkdownTask, ExportTextTask, ProjectTask};
use crate::tool::text::FullTextDigest;
use crate::world::base::{
    ConfigTask, DiagnosticsTask, ErasedExportTask, ErasedStrExportTask, ExportComputation,
    FlagTask, HtmlCompilationTask, OptionDocumentTask, PagedCompilationTask, WorldComputable,
    WorldComputeGraph,
};

struct MarkdownFlag;
struct TextFlag;

type ErasedMarkdownExport = ErasedStrExportTask<MarkdownFlag>;
type ErasedTextExport = ErasedStrExportTask<TextFlag>;

pub struct ProjectExport;

impl ProjectExport {
    #[must_use = "the result must be checked"]
    pub fn provide(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<()> {
        ErasedExportTask::<_, PdfFlag>::provide::<LspCompilerFeat, TypstPagedDocument, PdfExport>(
            graph,
        )?;
        ErasedExportTask::<_, SvgFlag>::provide::<LspCompilerFeat, TypstPagedDocument, SvgExport>(
            graph,
        )?;
        ErasedExportTask::<_, PngFlag>::provide::<LspCompilerFeat, TypstPagedDocument, PngExport>(
            graph,
        )?;
        ErasedExportTask::<_, HtmlFlag>::provide::<LspCompilerFeat, TypstHtmlDocument, HtmlExport>(
            graph,
        )?;
        ErasedExportTask::<_, MarkdownFlag>::provide_raw(graph, TypliteMarkdownExport::run)?;
        ErasedExportTask::<_, TextFlag>::provide::<LspCompilerFeat, TypstPagedDocument, TextExport>(
            graph,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Default)]
pub struct ProjectCompilation;

impl ProjectCompilation {
    fn needs_run<F: CompilerFeat, D: typst::Document>(
        snap: &CompileSnapshot<F>,
        timing: Option<TaskWhen>,
        docs: Option<&D>,
    ) -> Option<bool> {
        let s = snap.signal;
        let when = timing.unwrap_or(TaskWhen::Never);
        if !matches!(when, TaskWhen::Never) && s.by_entry_update {
            return Some(true);
        }

        match when {
            TaskWhen::Never => Some(false),
            TaskWhen::OnType => Some(s.by_mem_events),
            TaskWhen::OnSave => Some(s.by_fs_events),
            TaskWhen::OnDocumentHasTitle if s.by_fs_events => {
                docs.map(|doc| doc.info().title.is_some())
            }
            TaskWhen::OnDocumentHasTitle => Some(false),
        }
    }

    pub fn preconfig_timings<F: CompilerFeat>(graph: &Arc<WorldComputeGraph<F>>) -> Result<bool> {
        // todo: configure run_diagnostics!
        let paged_diag = Some(TaskWhen::OnType);
        let html_diag = Some(TaskWhen::Never);

        let pdf: Option<TaskWhen> = graph
            .get::<ConfigTask<<PdfExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.0.export.when);
        let svg: Option<TaskWhen> = graph
            .get::<ConfigTask<<SvgExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.0.export.when);
        let png: Option<TaskWhen> = graph
            .get::<ConfigTask<<PngExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.0.export.when);
        let html: Option<TaskWhen> = graph
            .get::<ConfigTask<<HtmlExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.0.export.when);
        let md: Option<TaskWhen> = graph
            .get::<ConfigTask<ExportMarkdownTask>>()
            .transpose()?
            .map(|config| config.0.export.when);
        let text: Option<TaskWhen> = graph
            .get::<ConfigTask<<TextExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.0.export.when);

        let doc = None::<TypstPagedDocument>.as_ref();
        let check = |timing| Self::needs_run(&graph.snap, timing, doc).unwrap_or(true);

        let compile_paged = [paged_diag, pdf, svg, png, text, md].into_iter().any(check);
        let compile_html = [html_diag, html].into_iter().any(check);

        let _ = graph.provide(Ok(FlagTask::<PagedCompilationTask>::flag(compile_paged)));
        let _ = graph.provide(Ok(FlagTask::<HtmlCompilationTask>::flag(compile_html)));

        Ok(compile_paged || compile_html)
    }
}

impl<F: CompilerFeat> WorldComputable<F> for ProjectCompilation {
    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Self::preconfig_timings(graph)?;
        DiagnosticsTask::compute(graph)?;
        Ok(Self)
    }
}

impl WorldComputable<LspCompilerFeat> for ProjectExport {
    fn compute(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<Self> {
        let config = graph.must_get::<ConfigTask<ProjectTask>>()?;
        let output_path = config.0.as_export().and_then(|e| {
            e.output
                .as_ref()
                .and_then(|o| o.substitute(&graph.snap.world.entry_state()))
        });

        let output = || -> Result<Option<SourceResult<Vec<u8>>>> {
            Ok(match &config.0 {
                ProjectTask::Preview(..) => todo!(),
                ProjectTask::ExportPdf(..) => graph.compute::<ErasedPdfExport>()?.result.clone(),
                ProjectTask::ExportPng(..) => graph.compute::<ErasedPngExport>()?.result.clone(),
                ProjectTask::ExportSvg(..) => {
                    let svg = graph.compute::<ErasedSvgExport>()?.result.clone();
                    svg.map(|s| s.map(|s| s.into_bytes()))
                }
                ProjectTask::ExportHtml(..) => {
                    let html = graph.compute::<ErasedHtmlExport>()?.result.clone();
                    html.map(|s| s.map(|s| s.into_bytes()))
                }
                ProjectTask::ExportMarkdown(..) => {
                    let markdown = graph.compute::<ErasedMarkdownExport>()?.result.clone();
                    markdown.map(|s| s.map(|s| s.into_bytes()))
                }
                ProjectTask::ExportText(..) => {
                    let text = graph.compute::<ErasedTextExport>()?.result.clone();
                    text.map(|s| s.map(|s| s.into_bytes()))
                }
                ProjectTask::Query(..) => todo!(),
            })
        };

        if let Some(path) = output_path {
            let output = output()?;
            // todo: don't ignore export source diagnostics
            if let Some(Ok(output)) = output {
                std::fs::write(path, output).context("failed to write output")?;
            }
        }

        Ok(Self {})
    }
}

pub struct TypliteMarkdownExport(pub Option<SourceResult<String>>);

impl TypliteMarkdownExport {
    fn needs_run(
        graph: &Arc<WorldComputeGraph<LspCompilerFeat>>,
        doc: Option<&TypstPagedDocument>,
        config: &ExportMarkdownTask,
    ) -> bool {
        let timing = config.export.when;
        ProjectCompilation::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(
        graph: &Arc<WorldComputeGraph<LspCompilerFeat>>,
    ) -> Result<Option<SourceResult<String>>> {
        if !OptionDocumentTask::needs_run(graph, Self::needs_run)? {
            return Ok(None);
        }

        let conv = Typlite::new(Arc::new(graph.snap.world.clone()))
            .convert()
            .map_err(|e| anyhow::anyhow!("failed to convert to markdown: {e}"))?;

        Ok(Some(Ok(conv.to_string())))
    }
}

impl WorldComputable<LspCompilerFeat> for TypliteMarkdownExport {
    fn compute(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<Self> {
        Self::run(graph).map(Self)
    }
}

pub struct TextExport(pub Option<SourceResult<String>>);

impl<F: CompilerFeat> ExportComputation<F, TypstPagedDocument> for TextExport {
    type Output = SourceResult<String>;
    type Config = ExportTextTask;

    fn needs_run(
        graph: &Arc<WorldComputeGraph<F>>,
        doc: Option<&TypstPagedDocument>,
        config: &Self::Config,
    ) -> bool {
        let timing = config.export.when;
        ProjectCompilation::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(
        doc: &Arc<TypstPagedDocument>,
        _config: &ExportTextTask,
    ) -> Result<SourceResult<String>> {
        Ok(Ok(format!(
            "{}",
            FullTextDigest(TypstDocument::Paged(doc.clone()))
        )))
    }
}

impl<F: CompilerFeat> WorldComputable<F> for TextExport {
    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Ok(Self(OptionDocumentTask::run_export::<F, Self>(graph)?))
    }
}
