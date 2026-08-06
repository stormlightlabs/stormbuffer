fn main() {
    if std::env::args().any(|argument| argument == "--help" || argument == "-h") {
        println!("Usage: stormbuffer-server");
        println!();
        println!("Run the local Stormbuffer HTTP server (not implemented yet).");
        return;
    }

    if let Err(error) = stormbuffer_server::run() {
        eprintln!("stormbuffer-server: {error}");
        std::process::exit(1);
    }
}
