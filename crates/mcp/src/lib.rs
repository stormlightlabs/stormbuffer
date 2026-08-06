use stormbuffer_core::{Error, Result};

/// The MCP adapter is intentionally an explicit stub until the protocol milestone.
pub fn run_stdio() -> Result<()> {
    Err(Error::InvalidInput(
        "the MCP stdio adapter is not implemented yet".to_owned(),
    ))
}
