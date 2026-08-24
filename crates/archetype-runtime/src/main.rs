fn main() {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    if let Err(error) = archetype_runtime::serve_authenticated(stdin.lock(), stdout.lock()) {
        eprintln!("archetype runtime stopped: {error}");
        std::process::exit(1);
    }
}
