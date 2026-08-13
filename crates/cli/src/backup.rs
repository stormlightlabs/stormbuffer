use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;

use anyhow::{Context, Result as AnyhowResult, bail};
use stormbuffer_core::{self as core, StoreScope};

use crate::command::{DestroyStoreArgs, ImportArgs, PathArgs, VerifyExportArgs};
use crate::echo::Echo;
use crate::{report_error, resolve};

pub(super) fn run_export(scope: StoreScope, arguments: PathArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let bundle = match core::export_store(&paths).context("could not export canonical records") {
        Ok(bundle) => bundle,
        Err(error) => return report_error(error, output),
    };
    let encoded = match core::encode_export(&bundle) {
        Ok(encoded) => encoded,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    match arguments.path.as_deref() {
        None | Some("-") => {
            output.raw(encoded.as_bytes());
        }
        Some(path) => match core::write_export_archive(&paths, Path::new(path), encoded.as_bytes())
        {
            Ok(()) => output.line(&format!(
                "{} {} records to {}",
                output.success("Exported"),
                bundle.records.len(),
                output.path(path)
            )),
            Err(error) => {
                return report_error(
                    anyhow::Error::new(error).context("could not write export archive"),
                    output,
                );
            }
        },
    }
    0
}

pub(super) fn run_import(scope: StoreScope, arguments: ImportArgs, output: &Echo) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let contents = if arguments.path == "-" {
        match read_import_archive(io::stdin()) {
            Ok(contents) => contents,
            Err(error) => {
                return report_error(
                    error.context("could not read import archive from stdin"),
                    output,
                );
            }
        }
    } else {
        match fs::File::open(&arguments.path)
            .map_err(anyhow::Error::new)
            .and_then(read_import_archive)
        {
            Ok(contents) => contents,
            Err(error) => {
                return report_error(error.context("could not read import archive"), output);
            }
        }
    };
    let bundle = match core::decode_export(&contents) {
        Ok(bundle) => bundle,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    let options = match import_options(&arguments) {
        Ok(options) => options,
        Err(error) => return report_error(error, output),
    };
    if arguments.dry_run {
        return match core::preview_import(&paths, &bundle, &options)
            .context("could not preview canonical record import")
        {
            Ok(preview) => {
                output.field("Dry run", "yes");
                for record in &preview.records {
                    output.line(&format!(
                        "{} -> {} | {} | {} | {}{}",
                        record.source_id,
                        record.target_id,
                        record.scope,
                        record.destination,
                        record.action,
                        record
                            .equivalent_record_id
                            .as_ref()
                            .map_or_else(String::new, |id| format!(" | equivalent: {id}"))
                    ));
                }
                print_import_report(&preview.report, output);
                0
            }
            Err(error) => report_error(error, output),
        };
    }
    match core::import_store(&paths, &bundle, &options)
        .context("could not import canonical records")
    {
        Ok(report) => {
            print_import_report(&report, output);
            0
        }
        Err(error) => report_error(error, output),
    }
}

pub(super) fn run_verify_export(arguments: VerifyExportArgs, output: &Echo) -> i32 {
    let contents = match read_archive_path(&arguments.path) {
        Ok(contents) => contents,
        Err(error) => return report_error(error.context("could not read export archive"), output),
    };
    let bundle = match core::decode_export(&contents).and_then(|bundle| {
        let report = core::verify_export(&bundle)?;
        Ok(report)
    }) {
        Ok(report) => report,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    output.field("Verified", arguments.path);
    output.field("Format version", bundle.format_version);
    output.field("Source scope", bundle.source_scope);
    output.field("Records", bundle.records);
    0
}

pub(super) fn run_destroy_store(
    scope: StoreScope,
    arguments: DestroyStoreArgs,
    output: &Echo,
) -> i32 {
    let paths = match resolve(scope) {
        Ok(paths) => paths,
        Err(error) => return report_error(error, output),
    };
    let preview = match core::preview_store_destruction(&paths) {
        Ok(preview) => preview,
        Err(error) => return report_error(anyhow::Error::new(error), output),
    };
    output.field("Store ID", &preview.store_id);
    output.field("Scope", &preview.scope);
    output.field("Root", &preview.root);
    output.field("Store root bytes", preview.store_root_bytes);
    output.field("Canonical records", preview.records);
    output.field("Canonical bytes", preview.canonical_bytes);
    output.field("Disposable bytes", preview.disposable_bytes);

    if let Some(expected) = arguments.store_id.as_deref() {
        if expected != preview.store_id {
            output.error("the supplied --store-id does not match the selected store");
            return 1;
        }
    }
    if arguments.yes && arguments.store_id.is_none() {
        output.error("noninteractive destruction requires both --yes and --store-id");
        return 1;
    }
    if !arguments.yes {
        if !io::stdin().is_terminal() || !io::stdout().is_terminal() || !io::stderr().is_terminal()
        {
            output.error("noninteractive destruction requires both --yes and --store-id");
            return 1;
        }
        let mut stderr = io::stderr().lock();
        let _ = write!(
            stderr,
            "Type store ID {} to destroy it (or cancel and use --export first): ",
            preview.store_id
        );
        let _ = stderr.flush();
        let mut answer = String::new();
        if let Err(error) = io::stdin().read_line(&mut answer) {
            return report_error(
                anyhow::Error::new(error).context("could not read confirmation"),
                output,
            );
        }
        if answer.trim() != preview.store_id {
            output.error("store destruction cancelled");
            return 1;
        }
    }
    match core::destroy_store(
        &paths,
        &preview.store_id,
        core::DestructionAcknowledgement::deliberate(),
        arguments.export.as_deref(),
    ) {
        Ok(()) => {
            if let Some(destination) = arguments.export.as_deref() {
                output.field("Exported", destination.display());
            }
            output.line(&format!("Destroyed store {}", preview.store_id));
            0
        }
        Err(error) => report_error(
            anyhow::Error::new(error).context("could not destroy selected store"),
            output,
        ),
    }
}

fn print_import_report(report: &core::ImportReport, output: &Echo) {
    output.field("Imported", report.imported);
    output.field("Skipped", report.skipped);
    output.field("Overwritten", report.overwritten);
    output.field("Remapped", report.remapped);
}

fn read_archive_path(path: &str) -> AnyhowResult<String> {
    if path == "-" {
        read_import_archive(io::stdin())
    } else {
        fs::File::open(path)
            .map_err(anyhow::Error::new)
            .and_then(read_import_archive)
    }
}

fn import_options(arguments: &ImportArgs) -> AnyhowResult<core::ImportOptions> {
    Ok(core::ImportOptions {
        id_collision: arguments
            .on_id
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        scope_collision: arguments
            .on_scope
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
        existing_record: arguments
            .on_existing
            .as_deref()
            .map(str::parse)
            .transpose()
            .map_err(anyhow::Error::msg)?,
    })
}

fn read_import_archive(reader: impl Read) -> AnyhowResult<String> {
    let limit = u64::try_from(core::MAX_EXPORT_ARCHIVE_BYTES).expect("archive limit fits in u64");
    let mut contents = String::new();
    reader
        .take(limit + 1)
        .read_to_string(&mut contents)
        .context("could not read the archive")?;
    if contents.len() > core::MAX_EXPORT_ARCHIVE_BYTES {
        bail!(
            "import archive exceeds the {} byte limit",
            core::MAX_EXPORT_ARCHIVE_BYTES
        );
    }
    Ok(contents)
}
