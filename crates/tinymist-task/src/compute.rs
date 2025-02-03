#![allow(missing_docs)]

use std::str::FromStr;
use std::sync::Arc;

use tinymist_std::error::prelude::*;
use tinymist_std::typst::{TypstHtmlDocument, TypstPagedDocument};
use typst::diag::SourceResult;
use typst::foundations::{Bytes, Datetime};
use typst::layout::Abs;
use typst::syntax::{ast, SyntaxNode};
use typst::visualize::Color;
use typst_pdf::{PdfOptions, Timestamp};

use crate::model::{ExportHtmlTask, ExportPdfTask, ExportPngTask, ExportSvgTask};
use crate::primitives::TaskWhen;
use crate::{ExportTransform, Pages};
use tinymist_world::{
    args::convert_source_date_epoch, CompileSnapshot, CompilerFeat, ErasedStrExportTask,
    ErasedVecExportTask, ExportComputation, OptionDocumentTask, WorldComputable, WorldComputeGraph,
};

pub struct PdfFlag;
pub struct SvgFlag;
pub struct PngFlag;
pub struct HtmlFlag;

pub struct ExportTimings;

impl ExportTimings {
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
}

pub type ErasedPdfExport = ErasedVecExportTask<PdfFlag>;
pub type ErasedSvgExport = ErasedStrExportTask<SvgFlag>;
pub type ErasedPngExport = ErasedVecExportTask<PngFlag>;
pub type ErasedHtmlExport = ErasedStrExportTask<HtmlFlag>;

pub struct PdfExport(pub Option<SourceResult<Bytes>>);

impl<F: CompilerFeat> ExportComputation<F, TypstPagedDocument> for PdfExport {
    type Output = SourceResult<Bytes>;
    type Config = ExportPdfTask;

    fn needs_run(
        graph: &Arc<WorldComputeGraph<F>>,
        doc: Option<&TypstPagedDocument>,
        config: &Self::Config,
    ) -> bool {
        let timing = config.export.when;
        ExportTimings::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(doc: &Arc<TypstPagedDocument>, config: &ExportPdfTask) -> Result<SourceResult<Bytes>> {
        // todo: timestamp world.now()
        let creation_timestamp = config
            .creation_timestamp
            .map(convert_source_date_epoch)
            .transpose()
            .context_ut("parse pdf creation timestamp")?
            .unwrap_or_else(chrono::Utc::now);

        // todo: Some(pdf_uri.as_str())

        let bytes = typst_pdf::pdf(
            doc,
            &PdfOptions {
                timestamp: convert_datetime(creation_timestamp),
                ..Default::default()
            },
        );

        Ok(bytes.map(Bytes::new))
    }
}

impl<F: CompilerFeat> WorldComputable<F> for PdfExport {
    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Ok(Self(OptionDocumentTask::run_export::<F, Self>(graph)?))
    }
}

pub struct SvgExport(pub Option<SourceResult<String>>);

impl<F: CompilerFeat> ExportComputation<F, TypstPagedDocument> for SvgExport {
    type Output = SourceResult<String>;
    type Config = ExportSvgTask;

    fn needs_run(
        graph: &Arc<WorldComputeGraph<F>>,
        doc: Option<&TypstPagedDocument>,
        config: &Self::Config,
    ) -> bool {
        let timing = config.export.when;
        ExportTimings::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(doc: &Arc<TypstPagedDocument>, config: &ExportSvgTask) -> Result<SourceResult<String>> {
        let (is_first, merged_gap) = get_page_selection(&config.export)?;

        let first_page = doc.pages.first();

        Ok(Ok(if is_first {
            if let Some(first_page) = first_page {
                typst_svg::svg(first_page)
            } else {
                typst_svg::svg_merged(doc, merged_gap)
            }
        } else {
            typst_svg::svg_merged(doc, merged_gap)
        }))
    }
}

impl<F: CompilerFeat> WorldComputable<F> for SvgExport {
    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Ok(Self(OptionDocumentTask::run_export::<F, Self>(graph)?))
    }
}

pub struct PngExport(pub Option<SourceResult<Bytes>>);

impl<F: CompilerFeat> ExportComputation<F, TypstPagedDocument> for PngExport {
    type Output = SourceResult<Bytes>;
    type Config = ExportPngTask;

    fn needs_run(
        graph: &Arc<WorldComputeGraph<F>>,
        doc: Option<&TypstPagedDocument>,
        config: &Self::Config,
    ) -> bool {
        let timing = config.export.when;
        ExportTimings::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(doc: &Arc<TypstPagedDocument>, config: &ExportPngTask) -> Result<SourceResult<Bytes>> {
        let ppi = config.ppi.to_f32();
        if ppi <= 1e-6 {
            tinymist_std::bail!("invalid ppi: {ppi}");
        }

        let fill = if let Some(fill) = &config.fill {
            parse_color(fill.clone()).map_err(|err| anyhow::anyhow!("invalid fill ({err})"))?
        } else {
            Color::WHITE
        };

        let (is_first, merged_gap) = get_page_selection(&config.export)?;

        let ppp = ppi / 72.;
        let pixmap = if is_first {
            if let Some(first_page) = doc.pages.first() {
                typst_render::render(first_page, ppp)
            } else {
                typst_render::render_merged(doc, ppp, merged_gap, Some(fill))
            }
        } else {
            typst_render::render_merged(doc, ppp, merged_gap, Some(fill))
        };

        pixmap
            .encode_png()
            .map(Bytes::new)
            .context_ut("failed to encode PNG")
            .map(Ok)
    }
}

impl<F: CompilerFeat> WorldComputable<F> for PngExport {
    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Ok(Self(OptionDocumentTask::run_export::<F, Self>(graph)?))
    }
}

pub struct HtmlExport(pub Option<SourceResult<String>>);

impl<F: CompilerFeat> ExportComputation<F, TypstHtmlDocument> for HtmlExport {
    type Output = SourceResult<String>;
    type Config = ExportHtmlTask;

    fn needs_run(
        graph: &Arc<WorldComputeGraph<F>>,
        doc: Option<&TypstHtmlDocument>,
        config: &Self::Config,
    ) -> bool {
        let timing = config.export.when;
        ExportTimings::needs_run(&graph.snap, Some(timing), doc).unwrap_or_default()
    }

    fn run(doc: &Arc<TypstHtmlDocument>, _config: &ExportHtmlTask) -> Result<SourceResult<String>> {
        Ok(typst_html::html(doc))
    }
}

impl<F: CompilerFeat> WorldComputable<F> for HtmlExport {
    fn compute(graph: &Arc<WorldComputeGraph<F>>) -> Result<Self> {
        Ok(Self(OptionDocumentTask::run_export::<F, Self>(graph)?))
    }
}

/// Gets legacy page selection
pub fn get_page_selection(task: &crate::ExportTask) -> Result<(bool, Abs)> {
    let is_first = task
        .transform
        .iter()
        .any(|t| matches!(t, ExportTransform::Pages { ranges, .. } if ranges == &[Pages::FIRST]));

    let mut gap_res = Abs::default();
    if !is_first {
        for trans in &task.transform {
            if let ExportTransform::Merge { gap } = trans {
                let gap = gap
                    .as_deref()
                    .map(parse_length)
                    .transpose()
                    .context_ut("failed to parse gap")?;
                gap_res = gap.unwrap_or_default();
            }
        }
    }

    Ok((is_first, gap_res))
}

fn parse_length(gap: &str) -> Result<Abs> {
    let length = typst::syntax::parse_code(gap);
    if length.erroneous() {
        bail!("invalid length: {gap}, errors: {:?}", length.errors());
    }

    let length: Option<ast::Numeric> = descendants(&length).into_iter().find_map(SyntaxNode::cast);

    let Some(length) = length else {
        bail!("not a length: {gap}");
    };

    let (value, unit) = length.get();
    match unit {
        ast::Unit::Pt => Ok(Abs::pt(value)),
        ast::Unit::Mm => Ok(Abs::mm(value)),
        ast::Unit::Cm => Ok(Abs::cm(value)),
        ast::Unit::In => Ok(Abs::inches(value)),
        _ => bail!("invalid unit: {unit:?} in {gap}"),
    }
}

/// Low performance but simple recursive iterator.
fn descendants(node: &SyntaxNode) -> impl IntoIterator<Item = &SyntaxNode> + '_ {
    let mut res = vec![];
    for child in node.children() {
        res.push(child);
        res.extend(descendants(child));
    }

    res
}

fn parse_color(fill: String) -> anyhow::Result<Color> {
    match fill.as_str() {
        "black" => Ok(Color::BLACK),
        "white" => Ok(Color::WHITE),
        "red" => Ok(Color::RED),
        "green" => Ok(Color::GREEN),
        "blue" => Ok(Color::BLUE),
        hex if hex.starts_with('#') => {
            Color::from_str(&hex[1..]).map_err(|e| anyhow::anyhow!("failed to parse color: {e}"))
        }
        _ => anyhow::bail!("invalid color: {fill}"),
    }
}

/// Convert [`chrono::DateTime`] to [`Timestamp`]
fn convert_datetime(date_time: chrono::DateTime<chrono::Utc>) -> Option<Timestamp> {
    use chrono::{Datelike, Timelike};
    let datetime = Datetime::from_ymd_hms(
        date_time.year(),
        date_time.month().try_into().ok()?,
        date_time.day().try_into().ok()?,
        date_time.hour().try_into().ok()?,
        date_time.minute().try_into().ok()?,
        date_time.second().try_into().ok()?,
    );

    Some(Timestamp::new_utc(datetime.unwrap()))
}
