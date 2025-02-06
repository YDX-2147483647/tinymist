#![allow(missing_docs)]

use std::sync::Arc;

use reflexo_typst::{Bytes, CompilerFeat, EntryReader};
use tinymist_project::{
    HtmlExport, LspCompilerFeat, PdfExport, PdfFlag, PngExport, SvgExport, TaskWhen,
};
use tinymist_std::error::prelude::*;
use tinymist_std::typst::{TypstDocument, TypstHtmlDocument, TypstPagedDocument};
use tinymist_task::{ExportTimings, HtmlFlag, PngFlag, SvgFlag};
use typlite::Typlite;
use typst::diag::SourceResult;

use crate::project::{ExportMarkdownTask, ExportTextTask, ProjectTask};
use crate::tool::text::FullTextDigest;
use crate::world::base::{
    ConfigTask, DiagnosticsTask, ExportComputation, FlagTask, HtmlCompilationTask,
    OptionDocumentTask, PagedCompilationTask, WorldComputable, WorldComputeGraph,
};

#[derive(Clone, Copy, Default)]
pub struct ProjectCompilation;

impl ProjectCompilation {
    pub fn preconfig_timings<F: CompilerFeat>(graph: &Arc<WorldComputeGraph<F>>) -> Result<bool> {
        // todo: configure run_diagnostics!
        let paged_diag = Some(TaskWhen::OnType);
        let html_diag = Some(TaskWhen::Never);

        let pdf: Option<TaskWhen> = graph
            .get::<ConfigTask<<PdfExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.export.when);
        let svg: Option<TaskWhen> = graph
            .get::<ConfigTask<<SvgExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.export.when);
        let png: Option<TaskWhen> = graph
            .get::<ConfigTask<<PngExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.export.when);
        let html: Option<TaskWhen> = graph
            .get::<ConfigTask<<HtmlExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.export.when);
        let md: Option<TaskWhen> = graph
            .get::<ConfigTask<ExportMarkdownTask>>()
            .transpose()?
            .map(|config| config.export.when);
        let text: Option<TaskWhen> = graph
            .get::<ConfigTask<<TextExport as ExportComputation<LspCompilerFeat, _>>::Config>>()
            .transpose()?
            .map(|config| config.export.when);

        let doc = None::<TypstPagedDocument>.as_ref();
        let check = |timing| ExportTimings::needs_run(&graph.snap, timing, doc).unwrap_or(true);

        let compile_paged = [paged_diag, pdf, svg, png, text, md].into_iter().any(check);
        let compile_html = [html_diag, html].into_iter().any(check);

        let _ = graph.provide::<FlagTask<PagedCompilationTask>>(Ok(FlagTask::flag(compile_paged)));
        let _ = graph.provide::<FlagTask<HtmlCompilationTask>>(Ok(FlagTask::flag(compile_html)));

        Ok(compile_paged || compile_html)
    }
}

impl<F: CompilerFeat> WorldComputable<F> for ProjectCompilation {
    type Output = Self;

    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Self::preconfig_timings(graph)?;
        DiagnosticsTask::compute(graph)?;
        Ok(Self)
    }
}

pub struct ProjectExport;

impl ProjectExport {
    #[must_use = "the result must be checked"]
    pub fn provide(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<()> {
        Ok(())
    }

    // pub fn needs_run<F: CompilerFeat, C: Send + Sync + 'static>(
    //     graph: &Arc<WorldComputeGraph<F>>,
    //     f: impl FnOnce(&Arc<WorldComputeGraph<F>>, Option<&D>, &C) -> bool,
    // ) -> Result<bool> {
    //     let Some(config) = graph.get::<ConfigTask<C>>().transpose()? else {
    //         return Ok(false);
    //     };

    //     let doc = graph.compute::<OptionDocumentTask<D>>()?;
    //     Ok(f(graph, doc.as_deref(), &config))
    // }

    // pub fn run_export<F: CompilerFeat, T: ExportComputation<F, D>>(
    //     graph: &Arc<WorldComputeGraph<F>>,
    // ) -> Result<Option<T::Output>> {
    //     if !OptionDocumentTask::needs_run(graph, T::needs_run)? {
    //         return Ok(None);
    //     }

    //     let config = graph.get::<ConfigTask<T::Config>>().transpose()?;
    //     let result = config.map(|config| T::run_with::<Self>(graph, &config));
    //     result.transpose()
    // }

    fn run_bytes<T: ExportComputation<LspCompilerFeat, TypstPagedDocument>>(
        config: &T::Config,
    ) -> Result<Option<Bytes>> {
        todo!()
    }

    fn run_string<T: ExportComputation<LspCompilerFeat, TypstPagedDocument>>(
        config: &T::Config,
    ) -> Result<Option<Bytes>> {
        todo!()
    }
}

impl WorldComputable<LspCompilerFeat> for ProjectExport {
    type Output = Self;

    fn compute(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<Self> {
        let config = graph.must_get::<ConfigTask<ProjectTask>>()?;
        let output_path = config.as_export().and_then(|e| {
            e.output
                .as_ref()
                .and_then(|o| o.substitute(&graph.snap.world.entry_state()))
        });

        fn from_string(
            s: Arc<Option<SourceResult<String>>>,
        ) -> Result<Option<SourceResult<Bytes>>> {
            Ok(s.as_ref().clone().map(|s| s.map(Bytes::from_string)))
        }

        // ErasedExportTask::<_, PdfFlag>::provide::<LspCompilerFeat,
        // TypstPagedDocument, PdfExport>(     graph,
        // )?;
        // ErasedExportTask::<_, SvgFlag>::provide::<LspCompilerFeat,
        // TypstPagedDocument, SvgExport>(     graph,
        // )?;
        // ErasedExportTask::<_, PngFlag>::provide::<LspCompilerFeat,
        // TypstPagedDocument, PngExport>(     graph,
        // )?;
        // ErasedExportTask::<_, HtmlFlag>::provide::<LspCompilerFeat,
        // TypstHtmlDocument, HtmlExport>(     graph,
        // )?;
        // ErasedExportTask::<_, MarkdownFlag>::provide_raw(graph,
        // TypliteMarkdownExport::run)?; ErasedExportTask::<_,
        // TextFlag>::provide::<LspCompilerFeat, TypstPagedDocument, TextExport>(
        //     graph,
        // )?;

        let output = || -> Result<Option<Bytes>> {
            use ProjectTask::*;
            match config.as_ref() {
                Preview(..) => todo!(),
                ExportPdf(config) => Self::run_bytes::<PdfExport>(config),
                ExportPng(config) => Self::run_bytes::<PngExport>(config),
                ExportSvg(config) => Self::run_string::<SvgExport>(config),
                ExportHtml(config) => Self::run_string::<HtmlExport>(config),
                ExportMd(config) => Self::run_string::<TypliteMdExport>(config),
                ExportText(config) => Self::run_string::<TextExport>(config),
                Query(..) => todo!(),
            }
        };

        if let Some(path) = output_path {
            let output = output()?;
            // todo: don't ignore export source diagnostics
            if let Some(output) = output {
                std::fs::write(path, output).context("failed to write output")?;
            }
        }

        Ok(Self {})
    }
}

pub struct TypliteMdExport(pub Option<SourceResult<String>>);

impl TypliteMdExport {
    fn needs_run(
        graph: &Arc<WorldComputeGraph<LspCompilerFeat>>,
        doc: Option<&TypstPagedDocument>,
        config: &ExportMarkdownTask,
    ) -> bool {
        let timing = config.export.when;
        ExportTimings::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<Option<String>> {
        if !OptionDocumentTask::needs_run(graph, Self::needs_run)? {
            return Ok(None);
        }

        let conv = Typlite::new(Arc::new(graph.snap.world.clone()))
            .convert()
            .map_err(|e| anyhow::anyhow!("failed to convert to markdown: {e}"))?;

        Ok(Some(conv.to_string()))
    }
}

impl WorldComputable<LspCompilerFeat> for TypliteMdExport {
    type Output = Option<String>;

    fn compute(graph: &Arc<WorldComputeGraph<LspCompilerFeat>>) -> Result<Self::Output> {
        Self::run(graph)
    }
}

pub struct TextExport;

impl<F: CompilerFeat> ExportComputation<F, TypstPagedDocument> for TextExport {
    type Output = String;
    type Config = ExportTextTask;

    fn needs_run(
        graph: &Arc<WorldComputeGraph<F>>,
        doc: Option<&TypstPagedDocument>,
        config: &Self::Config,
    ) -> bool {
        let timing = config.export.when;
        ExportTimings::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(
        _g: &Arc<WorldComputeGraph<F>>,
        doc: &Arc<TypstPagedDocument>,
        _config: &ExportTextTask,
    ) -> Result<String> {
        Ok(format!(
            "{}",
            FullTextDigest(TypstDocument::Paged(doc.clone()))
        ))
    }
}

impl<F: CompilerFeat> WorldComputable<F> for TextExport {
    type Output = Option<String>;

    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self::Output> {
        OptionDocumentTask::run_export::<F, Self>(graph)
    }
}
