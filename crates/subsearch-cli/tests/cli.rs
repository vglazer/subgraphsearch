//! End-to-end tests of the `subsearch` command, in particular multi-graph graph6/sparse6 files.

use std::path::Path;
use std::process::{Command, Output};

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_subsearch"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

/// A temporary directory holding `many.g6`: four graphs, the last too small for the searches.
fn multi_graph_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("subsearch-cli-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut text = String::new();
    for (model, n, d, s) in [
        ("exponential", "40", "0.5", "1"),
        ("exponential", "40", "0.5", "2"),
        ("geometric", "50", "0.2", "3"),
        ("power", "6", "0.5", "4"),
    ] {
        let o = run(&dir, &["generate", model, "-n", n, "-d", d, "-s", s]);
        assert!(o.status.success(), "{}", stderr(&o));
        text += &stdout(&o);
    }
    std::fs::write(dir.join("many.g6"), text).unwrap();
    dir
}

#[test]
fn single_graph_commands_reject_multi_graph_files_without_index() {
    let dir = multi_graph_dir("reject");
    let o = run(&dir, &["search", "many.g6", "-k", "5", "-m", "3"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(stderr(&o).contains("holds 4 graphs"), "{}", stderr(&o));
}

#[test]
fn index_selects_one_graph() {
    let dir = multi_graph_dir("index");
    let o = run(&dir, &["info", "many.g6", "--index", "2"]);
    assert!(o.status.success(), "{}", stderr(&o));
    assert!(stdout(&o).starts_with("vertices:   50\n"), "{}", stdout(&o));
    let o = run(&dir, &["info", "many.g6", "--index", "4"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("--index 4 is out of range"),
        "{}",
        stderr(&o)
    );
    let o = run(
        &dir,
        &[
            "search", "many.g6", "--index", "1", "-k", "6", "-m", "7", "-n", "5", "-q",
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
}

#[test]
fn info_lists_every_graph() {
    let dir = multi_graph_dir("info");
    let o = run(&dir, &["info", "many.g6"]);
    let out = stdout(&o);
    assert!(out.starts_with("4 graphs\n"), "{out}");
    assert_eq!(out.lines().count(), 6);
}

#[test]
fn search_all_searches_every_graph_and_skips_small_ones() {
    let dir = multi_graph_dir("all");
    let o = run(
        &dir,
        &[
            "search", "many.g6", "--all", "-k", "8", "-m", "10", "-n", "10", "-q", "--json",
            "r.json",
        ],
    );
    assert!(o.status.success(), "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("graph 3: skipped"), "{out}");
    assert!(out.contains("/4 graphs contain"), "{out}");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("r.json")).unwrap()).unwrap();
    let graphs: Vec<u64> = json
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["graph"].as_u64().unwrap())
        .collect();
    assert_eq!(graphs, vec![0, 1, 2]);
}

#[test]
fn convert_round_trips_multi_graph_files() {
    let dir = multi_graph_dir("convert");
    assert!(
        run(&dir, &["convert", "many.g6", "many.s6"])
            .status
            .success()
    );
    assert!(
        run(&dir, &["convert", "many.s6", "back.g6"])
            .status
            .success()
    );
    assert_eq!(
        std::fs::read(dir.join("many.g6")).unwrap(),
        std::fs::read(dir.join("back.g6")).unwrap()
    );
    let o = run(&dir, &["convert", "many.g6", "x.edges"]);
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("holds a single graph"),
        "{}",
        stderr(&o)
    );
    assert!(
        run(&dir, &["convert", "many.g6", "one.edges", "--index", "0"])
            .status
            .success()
    );
}

#[test]
fn degree_searches_for_regular_subgraphs() {
    let dir = multi_graph_dir("regular");
    for extra in [&[][..], &["--two-phase"][..]] {
        let mut args = vec![
            "search", "many.g6", "--index", "2", "-k", "8", "--degree", "2", "-n", "10", "-q",
        ];
        args.extend_from_slice(extra);
        let o = run(&dir, &args);
        assert!(o.status.success(), "{}", stderr(&o));
        assert!(stdout(&o).contains("regular of degree 2"), "{}", stdout(&o));
    }
    let o = run(
        &dir,
        &[
            "search", "many.g6", "--index", "0", "-k", "5", "-m", "3", "--degree", "2",
        ],
    );
    assert_eq!(
        o.status.code(),
        Some(2),
        "-m and --degree conflict: {}",
        stderr(&o)
    );
    let o = run(
        &dir,
        &[
            "search",
            "many.g6",
            "--index",
            "0",
            "-k",
            "5",
            "--two-phase",
            "-m",
            "3",
        ],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(
        stderr(&o).contains("--two-phase needs --degree"),
        "{}",
        stderr(&o)
    );
}
