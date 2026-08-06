use stormbuffer_core::Error;

pub fn run() -> Result<(), Box<Error>> {
    Err(Box::new(Error::InvalidInput {
        message: "the local server is not implemented yet".to_owned(),
    }))
}
