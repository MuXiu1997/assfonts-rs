#![forbid(unsafe_code)]

mod backend;
mod files;
use anyhow::{bail, Context, Result};
use assfonts_ass::AssCodec;
use assfonts_core::{FontResolver, MissingGlyphPolicy, Processor, SubtitleCodec};
use assfonts_fonts::FontCatalog;
use clap::Parser;
use serde::Serialize;
use std::{collections::BTreeSet, fs, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(
    name = "assfonts-rs",
    version,
    about = "Subset fonts and embed them into ASS subtitles",
    long_about = "A modular native ASS font processor. HarfBuzz is statically linked by default. UTF-8 ASS v4+ input; explicit font files/directories; no system font fallback."
)]
struct Args {
    /// ASS file or directory (repeatable; directories are recursive)
    #[arg(short='i',long="input",required_unless_present="list_backends",num_args=1..)]
    inputs: Vec<PathBuf>,
    /// TTF/OTF/TTC/OTC file or directory (repeatable; directories are recursive)
    #[arg(short='f',long="font",required_unless_present="list_backends",num_args=1..)]
    fonts: Vec<PathBuf>,
    /// Output directory; defaults to each input's directory
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,
    /// Write a JSON batch report (incompatible with --check)
    #[arg(long, conflicts_with = "check")]
    report: Option<PathBuf>,
    /// Analyze and resolve fonts without subsetting or writing any files
    #[arg(long, conflicts_with = "overwrite")]
    check: bool,
    /// Replace existing output/report files (never input subtitles)
    #[arg(long)]
    overwrite: bool,
    /// Compiled subsetter backend
    #[arg(long, default_value = "harfbuzz")]
    backend: String,
    /// Missing cmap glyphs: warn requires an identical external renderer/default-font environment
    #[arg(long, default_value = "error", value_parser = ["error", "warn"])]
    missing_glyphs: String,
    /// List compiled backends and exit
    #[arg(long)]
    list_backends: bool,
    /// Print a machine-readable JSON result to stdout
    #[arg(long)]
    json: bool,
    /// Verbosity: 0 quiet, 1 summary, 2 per-file details
    #[arg(short='v',long,default_value_t=1,value_parser=clap::value_parser!(u8).range(0..=2))]
    verbosity: u8,
}

#[derive(Serialize)]
struct FileReport {
    input: PathBuf,
    output: PathBuf,
    #[serde(flatten)]
    report: assfonts_core::Report,
}

fn run(args: Args) -> Result<()> {
    let policy = if args.missing_glyphs == "warn" {
        MissingGlyphPolicy::Warn
    } else {
        MissingGlyphPolicy::Error
    };
    if args.list_backends {
        println!("{}", backend::available().join("\n"));
        return Ok(());
    }
    let inputs = files::discover(&args.inputs, &["ass"], true)?;
    let font_files = files::discover(&args.fonts, &["ttf", "otf", "ttc", "otc"], false)?;
    let mut catalog = FontCatalog::default();
    for file in &font_files {
        let bytes = fs::read(file).with_context(|| format!("read font {}", file.display()))?;
        catalog.add(file.to_string_lossy(), bytes)?;
    }
    if args.verbosity > 0 {
        eprintln!(
            "Indexed {} faces from {} files",
            catalog.face_count(),
            font_files.len()
        );
    }
    if args.check {
        let mut checked = Vec::new();
        for input in &inputs {
            let text = fs::read_to_string(input)
                .with_context(|| format!("read UTF-8 ASS {}", input.display()))?;
            let usage = AssCodec
                .analyze(&text)
                .with_context(|| format!("analyze {}", input.display()))?;
            let plan = catalog
                .plan(&usage, policy)
                .with_context(|| format!("check {}", input.display()))?;
            if args.verbosity > 0 && !plan.warnings.is_empty() {
                eprintln!("{}: {} missing-cmap warning(s); fixed renderer/default-font environment required", input.display(), plan.warnings.len());
            }
            checked.push(serde_json::json!({"input":input,"font_requests":usage.len(),"missing_glyph_policy":policy,"warnings":plan.warnings}));
        }
        if args.json {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &serde_json::json!({"schema_version":1,"mode":"check","files":checked})
                )?
            );
        } else if args.verbosity > 0 {
            eprintln!("Checked {} subtitle(s); no files written", inputs.len());
        }
        return Ok(());
    }
    let backend = backend::create(&args.backend)?;
    let processor = Processor {
        codec: &AssCodec,
        resolver: &catalog,
        subsetter: backend.as_ref(),
    };
    let mut targets = BTreeSet::new();
    let mut outputs = Vec::new();
    for input in &inputs {
        let target = files::output_for(input, args.output.as_deref())?;
        let target = files::prepare_target(&target, args.overwrite)?;
        if inputs.contains(&target)
            || font_files.contains(&target)
            || !targets.insert(target.clone())
        {
            bail!(
                "output collides with input or another output: {}",
                target.display()
            );
        }
        outputs.push(target);
    }
    let report_target = args
        .report
        .as_ref()
        .map(|p| files::prepare_target(p, args.overwrite))
        .transpose()?;
    if let Some(target) = &report_target {
        if inputs.contains(target) || font_files.contains(target) || !targets.insert(target.clone())
        {
            bail!("report collides with an input/output: {}", target.display());
        }
    }
    // Finish every analysis/subset before publishing any output in this batch.
    let mut prepared = Vec::new();
    let mut reports = Vec::new();
    for (input, output) in inputs.iter().zip(&outputs) {
        let text = fs::read_to_string(input)
            .with_context(|| format!("read UTF-8 ASS {}", input.display()))?;
        let result = processor
            .process_with_policy(&text, policy)
            .with_context(|| format!("process {}", input.display()))?;
        if args.verbosity > 1 {
            eprintln!(
                "{}: {} embedded font(s), {} bytes",
                input.display(),
                result.report.fonts.len(),
                result.subtitle.len()
            );
        }
        if args.verbosity > 0 && !result.report.warnings.is_empty() {
            eprintln!(
                "{}: {} missing-cmap warning(s); fixed renderer/default-font environment required",
                input.display(),
                result.report.warnings.len()
            );
        }
        prepared.push(result.subtitle);
        reports.push(FileReport {
            input: input.clone(),
            output: output.clone(),
            report: result.report,
        });
    }
    let json = serde_json::to_string_pretty(
        &serde_json::json!({"schema_version":1,"mode":"embed","files":reports}),
    )?;
    for (written, (output, text)) in outputs.iter().zip(&prepared).enumerate() {
        files::atomic_write(output, text.as_bytes(), args.overwrite).with_context(|| {
            format!(
                "publish {} ({written} earlier outputs already written)",
                output.display()
            )
        })?;
    }
    if let Some(target) = report_target {
        files::atomic_write(&target, json.as_bytes(), args.overwrite).with_context(|| {
            format!(
                "subtitles written, but report publication failed: {}",
                target.display()
            )
        })?;
    }
    if args.json {
        println!("{json}");
    } else if args.verbosity > 0 {
        eprintln!(
            "Wrote {} subtitle(s) using {}",
            outputs.len(),
            backend.name()
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error:#}");
            ExitCode::FAILURE
        }
    }
}
