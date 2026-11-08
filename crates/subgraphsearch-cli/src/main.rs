//! The `subgraphsearch` command: see [`subgraphsearch::run`].

fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(subgraphsearch::run(std::env::args_os()))
}
