use stormbuffer_core as core;

pub const MAX_TOOL_ENVELOPE_BYTES: usize = 120 * 1024;
pub const SERVER_NAME: &str = "stormbuffer-mcp";
pub const SERVER_VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpConfig {
    pub scope: core::StoreScope,
    pub write_policy: McpWritePolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum McpWritePolicy {
    #[default]
    ReadOnly,
    CandidateOnly,
    All,
}

impl McpWritePolicy {
    pub(crate) fn allows(self, operation: &str) -> bool {
        match self {
            Self::ReadOnly => false,
            Self::CandidateOnly => matches!(operation, "remember" | "update"),
            Self::All => matches!(operation, "remember" | "update" | "archive"),
        }
    }
}

impl Default for McpConfig {
    fn default() -> Self {
        Self { scope: core::StoreScope::Global, write_policy: McpWritePolicy::ReadOnly }
    }
}
