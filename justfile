# convenience recipe for installing the cli & mcp server
install:
    cargo install --path crates/cli --locked && cargo install --path crates/mcp --locked

# deploy the kit doc site & then push
deploy-docs:
    pnpm --dir packages/docs deploy && git push

# reinstall the codex plugin
codex:
    codex plugin remove stormbuffer@stormbuffer-source
    codex plugin add stormbuffer@stormbuffer-source
