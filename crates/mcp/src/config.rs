use stormbuffer_core as core;

pub const MAX_TOOL_ENVELOPE_BYTES: usize = 120 * 1024;
pub const SERVER_NAME: &str = "stormbuffer-mcp";
pub const SERVER_VERSION: &str = "0.1.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct McpConfig {
    pub scope: core::StoreScope,
    pub allow_writes: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            scope: core::StoreScope::Global,
            allow_writes: false,
        }
    }
}
