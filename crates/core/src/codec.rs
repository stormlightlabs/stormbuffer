use std::path::Path;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::{Error, RecordError, Result};
use crate::record::{
    Access, RECORD_FORMAT_VERSION, Record, RecordId, RecordKind, RecordStatus, Scope, Source,
    SourceKind, Timestamp,
};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Frontmatter {
    format_version: u32,
    id: String,
    title: String,
    kind: String,
    scope: String,
    status: String,
    access: String,
    created_at: String,
    updated_at: String,
    tags: Vec<String>,
    aliases: Vec<String>,
    supersedes: Vec<String>,
    sources: Vec<FrontmatterSource>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FrontmatterSource {
    kind: String,
    reference: String,
    actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    observed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_hash: Option<String>,
}

pub fn parse_markdown(path: impl AsRef<Path>, markdown: &str) -> Result<Record> {
    let path = path.as_ref();
    let (frontmatter, body) = split_markdown(markdown)
        .map_err(|message| Error::invalid_record_at(path, RecordError::Markdown { message }))?;
    let raw: Frontmatter = toml::from_str(frontmatter)
        .map_err(|source| Error::invalid_record_at(path, RecordError::TomlParse { source }))?;

    if raw.format_version != RECORD_FORMAT_VERSION {
        return Err(Error::invalid_record_at(
            path,
            RecordError::UnsupportedFormatVersion {
                found: raw.format_version,
                expected: RECORD_FORMAT_VERSION,
            },
        ));
    }

    let record = Record {
        id: parse_field(path, "id", &raw.id, RecordId::parse)?,
        title: raw.title,
        kind: parse_field(path, "kind", &raw.kind, RecordKind::from_str)?,
        scope: parse_field(path, "scope", &raw.scope, Scope::parse)?,
        status: parse_field(path, "status", &raw.status, RecordStatus::from_str)?,
        access: parse_field(path, "access", &raw.access, Access::from_str)?,
        created_at: parse_field(path, "created_at", &raw.created_at, Timestamp::parse)?,
        updated_at: parse_field(path, "updated_at", &raw.updated_at, Timestamp::parse)?,
        tags: raw.tags,
        aliases: raw.aliases,
        supersedes: raw
            .supersedes
            .iter()
            .enumerate()
            .map(|(index, value)| {
                parse_field(
                    path,
                    &format!("supersedes[{index}]"),
                    value,
                    RecordId::parse,
                )
            })
            .collect::<Result<Vec<_>>>()?,
        sources: raw
            .sources
            .into_iter()
            .enumerate()
            .map(|(index, source)| {
                Ok(Source {
                    kind: parse_field(
                        path,
                        &format!("sources[{index}].kind"),
                        &source.kind,
                        SourceKind::from_str,
                    )?,
                    reference: source.reference,
                    actor: source.actor,
                    observed_at: source
                        .observed_at
                        .as_deref()
                        .map(|value| {
                            parse_field(
                                path,
                                &format!("sources[{index}].observed_at"),
                                value,
                                Timestamp::parse,
                            )
                        })
                        .transpose()?,
                    revision: source.revision,
                    content_hash: source.content_hash,
                })
            })
            .collect::<Result<Vec<_>>>()?,
        body: body.to_owned(),
    };

    record.validate().map_err(|error| with_path(path, error))?;
    Ok(record)
}

pub fn render_markdown(record: &Record) -> Result<String> {
    record.validate()?;
    let frontmatter = Frontmatter {
        format_version: RECORD_FORMAT_VERSION,
        id: record.id.to_string(),
        title: record.title.clone(),
        kind: record.kind.to_string(),
        scope: record.scope.to_string(),
        status: record.status.to_string(),
        access: record.access.to_string(),
        created_at: record.created_at.to_string(),
        updated_at: record.updated_at.to_string(),
        tags: record.tags.clone(),
        aliases: record.aliases.clone(),
        supersedes: record.supersedes.iter().map(ToString::to_string).collect(),
        sources: record
            .sources
            .iter()
            .map(|source| FrontmatterSource {
                kind: source.kind.to_string(),
                reference: source.reference.clone(),
                actor: source.actor.clone(),
                observed_at: source.observed_at.map(|value| value.to_string()),
                revision: source.revision.clone(),
                content_hash: source.content_hash.clone(),
            })
            .collect(),
    };
    let serialized = toml::to_string_pretty(&frontmatter).map_err(|source| {
        Error::invalid_record_at(Path::new("record.md"), RecordError::TomlRender { source })
    })?;
    Ok(format!("+++\n{serialized}+++\n\n{}", record.body))
}

fn parse_field<T>(
    path: &Path,
    field: &str,
    value: &str,
    parse: impl FnOnce(&str) -> std::result::Result<T, String>,
) -> Result<T> {
    parse(value).map_err(|message| {
        Error::invalid_record_at(
            path,
            RecordError::Validation {
                field: field.to_owned(),
                message,
            },
        )
    })
}

fn with_path(path: &Path, error: Error) -> Error {
    match error {
        Error::InvalidRecord { source, .. } => Error::invalid_record_at(path, source),
        other => other,
    }
}

fn split_markdown(markdown: &str) -> std::result::Result<(&str, &str), String> {
    let opening_length = if markdown.starts_with("+++\n") {
        4
    } else if markdown.starts_with("+++\r\n") {
        5
    } else {
        return Err("record must begin with a TOML opening delimiter (`+++`)".to_owned());
    };
    let frontmatter_and_body = &markdown[opening_length..];

    let mut search_from = 0;
    while let Some(relative_newline) = frontmatter_and_body[search_from..].find('\n') {
        let newline = search_from + relative_newline;
        let marker_start = newline + 1;
        let after_marker = &frontmatter_and_body[marker_start..];
        if !after_marker.starts_with("+++") {
            search_from = marker_start;
            continue;
        }

        let marker_line_ending = if after_marker.starts_with("+++\r\n") {
            5
        } else if after_marker.starts_with("+++\n") {
            4
        } else {
            search_from = marker_start;
            continue;
        };

        let frontmatter_end = if frontmatter_and_body[..newline].ends_with('\r') {
            newline - 1
        } else {
            newline
        };
        let frontmatter = &frontmatter_and_body[..frontmatter_end];
        let after_closing = &after_marker[marker_line_ending..];
        let blank_line_ending = if after_closing.starts_with("\r\n") {
            2
        } else if after_closing.starts_with('\n') {
            1
        } else {
            return Err("the closing delimiter must be followed by a blank line".to_owned());
        };
        return Ok((frontmatter, &after_closing[blank_line_ending..]));
    }

    Err("record is missing the closing TOML delimiter (`+++`)".to_owned())
}
