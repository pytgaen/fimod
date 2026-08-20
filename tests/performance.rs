use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use fimod::convert::{json_into_monty, monty_to_json};
use fimod::format::{csv_to_monty, serialize_csv, CsvOptions, DataFormat};
use fimod::pipeline::{
    build_scripts, execute_chain_to_value, ChainExecCtx, PipelineMetadata, ScriptRef,
};
use fimod::sandbox::SandboxPolicy;
use serde_json::{Map, Value};

const RUNS: usize = 5;

fn median_elapsed(mut run: impl FnMut() -> usize) -> Duration {
    black_box(run());

    let mut timings = Vec::with_capacity(RUNS);
    let mut checksum = 0usize;
    for _ in 0..RUNS {
        let start = Instant::now();
        checksum ^= black_box(run());
        timings.push(start.elapsed());
    }
    black_box(checksum);

    timings.sort_unstable();
    timings[timings.len() / 2]
}

fn perf_budget(release_ms: u64, debug_ms: u64) -> Duration {
    if cfg!(debug_assertions) {
        Duration::from_millis(debug_ms)
    } else {
        Duration::from_millis(release_ms)
    }
}

fn assert_under_budget(name: &str, actual: Duration, budget: Duration) {
    let actual_ms = actual.as_secs_f64() * 1000.0;
    let budget_ms = budget.as_secs_f64() * 1000.0;
    println!("perf {name}: median {actual_ms:.3} ms (budget {budget_ms:.3} ms)");
    assert!(
        actual <= budget,
        "{name}: median {actual_ms:.3} ms exceeded budget {budget_ms:.3} ms"
    );
}

fn tool_path(name: &str) -> Option<PathBuf> {
    if let Ok(output) = Command::new("mise").args(["which", name]).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    if Command::new(name).arg("--version").output().is_ok() {
        return Some(PathBuf::from(name));
    }

    None
}

fn require_tool(name: &str) -> Option<PathBuf> {
    let path = tool_path(name);
    if path.is_none() {
        eprintln!("perf comparison skipped: {name} is not available");
    }
    path
}

fn checked_output(program: &Path, args: &[OsString]) -> Output {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|err| panic!("failed to run {}: {err}", program.display()));
    assert!(
        output.status.success(),
        "{} failed with status {:?}\nstderr:\n{}",
        program.display(),
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn median_command_elapsed(program: &Path, args: &[OsString]) -> (Duration, Vec<u8>) {
    let warmup = checked_output(program, args);

    let mut timings = Vec::with_capacity(RUNS);
    let mut checksum = 0usize;
    for _ in 0..RUNS {
        let start = Instant::now();
        let output = checked_output(program, args);
        checksum ^= output.stdout.len();
        timings.push(start.elapsed());
    }
    black_box(checksum);

    timings.sort_unstable();
    (timings[timings.len() / 2], warmup.stdout)
}

fn report_comparison(name: &str, fimod: Duration, baseline_name: &str, baseline: Duration) {
    let fimod_ms = fimod.as_secs_f64() * 1000.0;
    let baseline_ms = baseline.as_secs_f64() * 1000.0;
    let ratio = fimod_ms / baseline_ms;
    println!(
        "perf {name}: fimod {fimod_ms:.3} ms vs {baseline_name} {baseline_ms:.3} ms ({ratio:.2}x)"
    );
}

fn assert_json_output_eq(left: &[u8], right: &[u8]) {
    let left: Value = serde_json::from_slice(left).unwrap();
    let right: Value = serde_json::from_slice(right).unwrap();
    assert_eq!(left, right);
}

fn large_json_array(rows: usize) -> String {
    let mut out = String::with_capacity(rows * 80);
    out.push('[');
    for i in 0..rows {
        if i > 0 {
            out.push(',');
        }
        write!(
            out,
            r#"{{"id":{i},"name":"user-{i:05}","active":{},"score":{},"team":"team-{}"}}"#,
            i % 2 == 0,
            i % 1_000,
            i % 16
        )
        .unwrap();
    }
    out.push(']');
    out
}

fn large_ndjson(rows: usize) -> String {
    let mut out = String::with_capacity(rows * 80);
    for i in 0..rows {
        writeln!(
            out,
            r#"{{"id":{i},"name":"user-{i:05}","active":{},"score":{},"team":"team-{}"}}"#,
            i % 2 == 0,
            i % 1_000,
            i % 16
        )
        .unwrap();
    }
    out
}

fn large_yaml_array(rows: usize) -> String {
    let mut out = String::with_capacity(rows * 70);
    for i in 0..rows {
        writeln!(out, "- id: {i}").unwrap();
        writeln!(out, "  name: user-{i:05}").unwrap();
        writeln!(out, "  active: {}", i % 2 == 0).unwrap();
        writeln!(out, "  score: {}", i % 1_000).unwrap();
        writeln!(out, "  team: team-{}", i % 16).unwrap();
    }
    out
}

fn large_csv_table(rows: usize) -> String {
    let mut out = String::with_capacity(rows * 40);
    out.push_str("id,name,active,score,team\n");
    for i in 0..rows {
        writeln!(
            out,
            "{i},user-{i:05},{},{},team-{}",
            i % 2 == 0,
            i % 1_000,
            i % 16
        )
        .unwrap();
    }
    out
}

fn large_log_lines(rows: usize) -> String {
    let mut out = String::with_capacity(rows * 40);
    for i in 0..rows {
        let level = if i % 10 == 0 { "ERROR" } else { "INFO" };
        writeln!(out, "{level} event={i:06} service=api latency={}", i % 997).unwrap();
    }
    out
}

fn os_args(args: &[&OsStr]) -> Vec<OsString> {
    args.iter().map(|arg| (*arg).to_os_string()).collect()
}

#[test]
#[ignore = "performance smoke test; run with `task test:performance`"]
fn json_parse_convert_and_serialize_large_array_under_budget() {
    let input = large_json_array(20_000);

    let elapsed = median_elapsed(|| {
        let value = DataFormat::JsonCompact.parse(black_box(&input)).unwrap();
        let monty = json_into_monty(value);
        let roundtrip = monty_to_json(monty).unwrap();
        let output = DataFormat::JsonCompact.serialize(&roundtrip).unwrap();
        output.len()
    });

    assert_under_budget(
        "json parse + Monty round-trip + compact serialize",
        elapsed,
        perf_budget(350, 3_500),
    );
}

#[test]
#[ignore = "external CLI comparison; run with `task test:performance`"]
fn cli_json_filter_compares_with_jq() {
    let Some(jq) = require_tool("jq") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("users.json");
    fs::write(&input, large_json_array(20_000)).unwrap();
    let fimod = PathBuf::from(env!("CARGO_BIN_EXE_fimod"));

    let fimod_args = os_args(&[
        OsStr::new("shape"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("-e"),
        OsStr::new(r#"[row for row in data if row["active"]]"#),
        OsStr::new("--output-format"),
        OsStr::new("json-compact"),
    ]);
    let jq_args = os_args(&[
        OsStr::new("-c"),
        OsStr::new(r#"[.[] | select(.active)]"#),
        input.as_os_str(),
    ]);

    let (fimod_elapsed, fimod_out) = median_command_elapsed(&fimod, &fimod_args);
    let (jq_elapsed, jq_out) = median_command_elapsed(&jq, &jq_args);
    assert_json_output_eq(&fimod_out, &jq_out);
    report_comparison(
        "CLI JSON filter over 20,000 records",
        fimod_elapsed,
        "jq",
        jq_elapsed,
    );
}

#[test]
#[ignore = "performance smoke test; run with `task test:performance`"]
fn cli_identity_json_to_ndjson_under_budget() {
    let rows = 20_000;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("users.json");
    fs::write(&input, large_json_array(rows)).unwrap();
    let fimod = PathBuf::from(env!("CARGO_BIN_EXE_fimod"));
    let args = os_args(&[
        OsStr::new("shape"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("-e"),
        OsStr::new("data"),
        OsStr::new("--output-format"),
        OsStr::new("ndjson"),
    ]);

    let (elapsed, output) = median_command_elapsed(&fimod, &args);
    let output = String::from_utf8(output).unwrap();
    assert_eq!(output.lines().count(), rows);
    assert_under_budget(
        "CLI identity JSON to NDJSON over 20,000 records",
        elapsed,
        perf_budget(250, 2_500),
    );
}

#[test]
#[ignore = "performance smoke test; run with `task test:performance`"]
fn cli_identity_json_to_csv_under_budget() {
    let rows = 20_000;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("users.json");
    fs::write(&input, large_json_array(rows)).unwrap();
    let fimod = PathBuf::from(env!("CARGO_BIN_EXE_fimod"));
    let args = os_args(&[
        OsStr::new("shape"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("-e"),
        OsStr::new("data"),
        OsStr::new("--output-format"),
        OsStr::new("csv"),
    ]);

    let (elapsed, output) = median_command_elapsed(&fimod, &args);
    let output = String::from_utf8(output).unwrap();
    assert_eq!(output.lines().count(), rows + 1);
    assert!(output.starts_with("id,name,active,score,team\n"));
    assert_under_budget(
        "CLI identity JSON to CSV over 20,000 records",
        elapsed,
        perf_budget(300, 3_000),
    );
}

#[test]
#[ignore = "performance smoke test; run with `task test:performance`"]
fn cli_identity_ndjson_to_json_under_budget() {
    let rows = 20_000;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("users.ndjson");
    fs::write(&input, large_ndjson(rows)).unwrap();
    let fimod = PathBuf::from(env!("CARGO_BIN_EXE_fimod"));
    let args = os_args(&[
        OsStr::new("shape"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("-e"),
        OsStr::new("data"),
        OsStr::new("--output-format"),
        OsStr::new("json-compact"),
    ]);

    let (elapsed, output) = median_command_elapsed(&fimod, &args);
    let value: Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value.as_array().map(Vec::len), Some(rows));
    assert_under_budget(
        "CLI identity NDJSON to compact JSON over 20,000 records",
        elapsed,
        perf_budget(250, 2_500),
    );
}

#[test]
#[ignore = "external CLI comparison; run with `task test:performance`"]
fn cli_yaml_to_json_compares_with_yq() {
    let Some(yq) = require_tool("yq") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("users.yaml");
    fs::write(&input, large_yaml_array(5_000)).unwrap();
    let fimod = PathBuf::from(env!("CARGO_BIN_EXE_fimod"));

    let fimod_args = os_args(&[
        OsStr::new("shape"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("-e"),
        OsStr::new("data"),
        OsStr::new("--output-format"),
        OsStr::new("json-compact"),
    ]);
    let yq_args = os_args(&[
        OsStr::new("-o=json"),
        OsStr::new("-I=0"),
        OsStr::new("."),
        input.as_os_str(),
    ]);

    let (fimod_elapsed, fimod_out) = median_command_elapsed(&fimod, &fimod_args);
    let (yq_elapsed, yq_out) = median_command_elapsed(&yq, &yq_args);
    assert_json_output_eq(&fimod_out, &yq_out);
    report_comparison(
        "CLI YAML to compact JSON over 5,000 records",
        fimod_elapsed,
        "yq",
        yq_elapsed,
    );
}

#[test]
#[ignore = "external CLI comparison; run with `task test:performance`"]
fn cli_lines_filter_compares_with_awk() {
    let Some(awk) = require_tool("awk") else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("events.log");
    fs::write(&input, large_log_lines(50_000)).unwrap();
    let fimod = PathBuf::from(env!("CARGO_BIN_EXE_fimod"));

    let fimod_args = os_args(&[
        OsStr::new("shape"),
        OsStr::new("-i"),
        input.as_os_str(),
        OsStr::new("--input-format"),
        OsStr::new("lines"),
        OsStr::new("-e"),
        OsStr::new(r#"[line for line in data if "ERROR" in line]"#),
        OsStr::new("--output-format"),
        OsStr::new("lines"),
    ]);
    let awk_args = os_args(&[OsStr::new("/ERROR/"), input.as_os_str()]);

    let (fimod_elapsed, fimod_out) = median_command_elapsed(&fimod, &fimod_args);
    let (awk_elapsed, awk_out) = median_command_elapsed(&awk, &awk_args);
    assert_eq!(
        String::from_utf8(fimod_out).unwrap(),
        String::from_utf8(awk_out).unwrap()
    );
    report_comparison(
        "CLI line filter over 50,000 records",
        fimod_elapsed,
        "awk",
        awk_elapsed,
    );
}

#[test]
#[ignore = "performance smoke test; run with `task test:performance`"]
fn csv_direct_monty_round_trip_large_table_under_budget() {
    let input = large_csv_table(20_000);
    let opts = CsvOptions::default();

    let elapsed = median_elapsed(|| {
        let (monty, headers) = csv_to_monty(black_box(&input), &opts).unwrap();
        let value = monty_to_json(monty).unwrap();
        let output = serialize_csv(&value, &opts).unwrap();
        output.len() ^ headers.map_or(0, |h| h.len())
    });

    assert_under_budget(
        "CSV direct Monty round-trip + serialize",
        elapsed,
        perf_budget(450, 4_500),
    );
}

#[test]
#[ignore = "performance smoke test; run with `task test:performance`"]
fn chained_molds_keep_large_payload_under_budget() {
    let input = DataFormat::JsonCompact
        .parse(&large_json_array(1_500))
        .unwrap();
    let steps = build_scripts(
        &[
            ScriptRef::Expr("[{**row, 'score': row['score'] + 1} for row in data]".to_string()),
            ScriptRef::Expr("[row for row in data if row['active']]".to_string()),
            ScriptRef::Expr("it_sort_by(data, 'name')".to_string()),
        ],
        false,
    )
    .unwrap();
    let metadata = PipelineMetadata {
        input: Some("perf.json"),
        output: None,
        input_format: Some("json"),
        output_format: Some("json"),
        in_place: false,
        slurp: false,
        no_input: false,
    };
    let env_value = Value::Object(Map::new());
    let headers_value = Value::Object(Map::new());
    let policy = SandboxPolicy::zero_authorization();
    let ctx = ChainExecCtx {
        extra_args: &[],
        env_value: &env_value,
        policy: &policy,
        debug: false,
        msg_level: 0,
    };

    let elapsed = median_elapsed(|| {
        let initial_data = json_into_monty(input.clone());
        let output =
            execute_chain_to_value(&steps, initial_data, &metadata, &headers_value, &ctx).unwrap();
        output.value.as_array().map_or(0, Vec::len)
    });

    assert_under_budget(
        "3-step mold chain over 1,500 records",
        elapsed,
        perf_budget(250, 12_000),
    );
}
