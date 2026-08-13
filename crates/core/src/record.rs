use std::fmt;
use std::str::FromStr;

use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use uuid::Uuid;

use crate::{Error, Result};

pub const RECORD_FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceKind {
    Conversation,
    Document,
    Issue,
    Url,
}

impl fmt::Display for SourceKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Conversation => "conversation",
            Self::Document => "document",
            Self::Issue => "issue",
            Self::Url => "url",
        })
    }
}

impl FromStr for SourceKind {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "conversation" => Ok(Self::Conversation),
            "document" => Ok(Self::Document),
            "issue" => Ok(Self::Issue),
            "url" => Ok(Self::Url),
            _ => Err("must be one of conversation, document, issue, or url".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordKind {
    Fact,
    Decision,
    Procedure,
    Checkpoint,
}

impl fmt::Display for RecordKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Fact => "fact",
            Self::Decision => "decision",
            Self::Procedure => "procedure",
            Self::Checkpoint => "checkpoint",
        })
    }
}

impl FromStr for RecordKind {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "fact" => Ok(Self::Fact),
            "decision" => Ok(Self::Decision),
            "procedure" => Ok(Self::Procedure),
            "checkpoint" => Ok(Self::Checkpoint),
            _ => Err("must be one of fact, decision, procedure, or checkpoint".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordStatus {
    Candidate,
    Active,
    Superseded,
    Archived,
}

impl RecordStatus {
    pub fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Candidate, Self::Active | Self::Archived)
                | (Self::Active, Self::Superseded | Self::Archived)
                | (Self::Archived, Self::Active)
        ) || self == next
    }
}

impl fmt::Display for RecordStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Candidate => "candidate",
            Self::Active => "active",
            Self::Superseded => "superseded",
            Self::Archived => "archived",
        })
    }
}

impl FromStr for RecordStatus {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "candidate" => Ok(Self::Candidate),
            "active" => Ok(Self::Active),
            "superseded" => Ok(Self::Superseded),
            "archived" => Ok(Self::Archived),
            _ => Err("must be one of candidate, active, superseded, or archived".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProposalActor {
    Human,
    Agent,
}

impl fmt::Display for ProposalActor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Human => "human",
            Self::Agent => "agent",
        })
    }
}

impl FromStr for ProposalActor {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            _ => Err("must be one of human or agent".to_owned()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalOutcome {
    Accepted,
    DuplicateOf,
    PossibleOverlap,
    RequiresApproval,
    Invalid,
}

impl fmt::Display for ProposalOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Accepted => "accepted",
            Self::DuplicateOf => "duplicate_of",
            Self::PossibleOverlap => "possible_overlap",
            Self::RequiresApproval => "requires_approval",
            Self::Invalid => "invalid",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Access {
    Human,
    Agent,
}

impl fmt::Display for Access {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Human => "human",
            Self::Agent => "agent",
        })
    }
}

impl FromStr for Access {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        match value {
            "human" => Ok(Self::Human),
            "agent" => Ok(Self::Agent),
            _ => Err("must be one of human or agent".to_owned()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Record {
    pub id: RecordId,
    pub title: String,
    pub kind: RecordKind,
    pub scope: Scope,
    pub status: RecordStatus,
    pub access: Access,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub tags: Vec<String>,
    pub aliases: Vec<String>,
    pub supersedes: Vec<RecordId>,
    pub sources: Vec<Source>,
    pub body: String,
}

impl Record {
    pub fn validate(&self) -> Result<()> {
        validate_text("title", &self.title)?;
        self.scope.validate()?;

        if self.updated_at < self.created_at {
            return Err(Error::invalid_record(
                "updated_at",
                "must not precede created_at",
            ));
        }

        validate_collection("tags", &self.tags)?;
        validate_collection("aliases", &self.aliases)?;

        let mut superseded_ids = std::collections::HashSet::with_capacity(self.supersedes.len());
        for id in &self.supersedes {
            if id == &self.id {
                return Err(Error::invalid_record(
                    "supersedes",
                    "must not contain the record id",
                ));
            }
            if !superseded_ids.insert(id) {
                return Err(Error::invalid_record(
                    "supersedes",
                    "must not contain duplicates",
                ));
            }
        }

        if self.sources.is_empty() {
            return Err(Error::invalid_record(
                "sources",
                "must contain at least one source",
            ));
        }
        for source in &self.sources {
            source.validate()?;
        }

        if self.body.trim().is_empty() {
            return Err(Error::invalid_record(
                "body",
                "must contain non-whitespace text",
            ));
        }
        if self.body.contains('\0') {
            return Err(Error::invalid_record(
                "body",
                "must not contain NUL characters",
            ));
        }

        Ok(())
    }

    pub fn validate_provenance(&self) -> Result<()> {
        if self.sources.is_empty() {
            return Err(Error::invalid_record(
                "sources",
                "must contain at least one attributable source",
            ));
        }
        if self.sources.iter().any(|source| {
            source.actor.eq_ignore_ascii_case("inference")
                || source.reference.starts_with("inference:")
                || source.reference.starts_with("inference://")
        }) {
            return Err(Error::invalid_record(
                "sources",
                "unsupported inference cannot be used as provenance",
            ));
        }
        Ok(())
    }

    pub fn transition_to(&mut self, next: RecordStatus) -> Result<()> {
        if !self.status.can_transition_to(next) {
            return Err(Error::invalid_record(
                "status",
                format!("invalid lifecycle transition: {} -> {}", self.status, next),
            ));
        }
        self.status = next;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RecordId(Uuid);

impl RecordId {
    pub fn new_v7() -> Self {
        Self(Uuid::now_v7())
    }

    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        if value.trim() != value || value.is_empty() {
            return Err("must be a non-empty UUID without surrounding whitespace".to_owned());
        }
        let id =
            Uuid::parse_str(value).map_err(|error| format!("must be a valid UUID: {error}"))?;
        if id.is_nil() {
            return Err("must not be the nil UUID".to_owned());
        }
        Ok(Self(id))
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for RecordId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

impl FromStr for RecordId {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Scope(String);

impl Scope {
    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        if value.trim() != value || value.is_empty() {
            return Err("must be a non-empty scope without surrounding whitespace".to_owned());
        }
        if value == "global" {
            return Ok(Self(value.to_owned()));
        }
        let Some(project) = value.strip_prefix("project:") else {
            return Err("must be `global` or use the `project:<uuid>` form".to_owned());
        };
        if project.is_empty()
            || project.chars().any(|character| {
                character.is_whitespace() || character.is_control() || character == ':'
            })
        {
            return Err(
                "project scope names must be non-empty and contain no whitespace or colons"
                    .to_owned(),
            );
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<()> {
        Self::parse(&self.0)
            .map(|_| ())
            .map_err(|message| Error::invalid_record("scope", message))
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Scope {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Timestamp(OffsetDateTime);

impl Timestamp {
    pub fn now_utc() -> Self {
        Self(OffsetDateTime::now_utc())
    }

    pub fn parse(value: &str) -> std::result::Result<Self, String> {
        OffsetDateTime::parse(value, &Rfc3339)
            .map(Self)
            .map_err(|error| format!("must be an RFC 3339 timestamp: {error}"))
    }

    pub const fn from_offset_datetime(value: OffsetDateTime) -> Self {
        Self(value)
    }

    pub const fn as_offset_datetime(self) -> OffsetDateTime {
        self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.format(&Rfc3339) {
            Ok(value) => formatter.write_str(&value),
            Err(_) => Err(fmt::Error),
        }
    }
}

impl FromStr for Timestamp {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub kind: SourceKind,
    pub reference: String,
    pub actor: String,
}

impl Source {
    fn validate(&self) -> Result<()> {
        validate_text("source reference", &self.reference)?;
        validate_text("source actor", &self.actor)
    }
}

fn validate_text(field: &str, value: &str) -> Result<()> {
    if value.is_empty() || value.trim() != value {
        return Err(Error::invalid_record(
            field,
            "must be non-empty and have no surrounding whitespace",
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(Error::invalid_record(
            field,
            "must not contain control characters",
        ));
    }
    Ok(())
}

fn validate_collection(field: &str, values: &[String]) -> Result<()> {
    for value in values {
        validate_text(field, value)?;
    }
    Ok(())
}
