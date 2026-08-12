use std::fs;
use std::io::{self, Read};
use std::path::Path;

use anyhow::{Context, Result as AnyhowResult, bail};
use stormbuffer_core::{self as core, StoreScope};

use crate::command::{ImportArgs, PathArgs};
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
    match core::import_store(&paths, &bundle, &options)
        .context("could not import canonical records")
    {
        Ok(report) => {
            output.field("Imported", report.imported);
            output.field("Skipped", report.skipped);
            output.field("Overwritten", report.overwritten);
            output.field("Remapped", report.remapped);
            0
        }
        Err(error) => report_error(error, output),
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
