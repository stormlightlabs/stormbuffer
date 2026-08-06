use stormbuffer_core::{Error, Result};

/// The HTTP server is deliberately deferred to the local-server milestone.
pub fn run() -> Result<()> {
    Err(Error::InvalidInput(
        "the local server is not implemented yet".to_owned(),
    ))
}
