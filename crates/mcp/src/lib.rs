use stormbuffer_core::Error;

pub fn run_stdio() -> Result<(), Box<Error>> {
    Err(Box::new(Error::InvalidInput {
        message: "the MCP stdio adapter is not implemented yet".to_owned(),
    }))
}
