fn main() {
    if std::env::args().any(|argument| argument == "--help" || argument == "-h") {
        println!("Usage: stormbuffer-mcp --stdio");
        println!();
        println!("Run the Stormbuffer MCP adapter over stdio (not implemented yet).");
        return;
    }

    if !std::env::args().any(|argument| argument == "--stdio") {
        eprintln!("stormbuffer-mcp: --stdio is required; the adapter is not implemented yet");
        std::process::exit(1);
    }

    if let Err(error) = stormbuffer_mcp::run_stdio() {
        eprintln!("stormbuffer-mcp: {error}");
        std::process::exit(1);
    }
}
