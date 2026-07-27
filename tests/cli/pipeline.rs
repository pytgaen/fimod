use super::helpers::{setup_input, setup_mold};
use predicates::prelude::*;
use std::time::Duration;

// ─── pipeline.current_step() ─────────────────────────────────────────────────

#[test]
fn test_pipeline_current_step_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);
    let mold = setup_mold(
        &dir,
        "step_idx.py",
        r#"
def transform(data, pipeline, **_):
    return pipeline.current_step().get('index')
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout("0\n");
}

#[test]
fn test_pipeline_length_single_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "pipeline.length()"])
        .assert()
        .success()
        .stdout("1\n");
}

#[test]
fn test_pipeline_length_chain() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    // Chain of 3 steps: the last one reads pipeline.length()
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "data",
            "-e",
            "data",
            "-e",
            "pipeline.length()",
        ])
        .assert()
        .success()
        .stdout("3\n");
}

#[test]
fn test_pipeline_step_index_in_chain() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    // Middle step of a 3-step chain: index should be 1
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "data",
            "-e",
            "pipeline.current_step().get('index')",
            "-e",
            "data",
        ])
        .assert()
        .success()
        .stdout("1\n");
}

// ─── pipeline.step(i) ────────────────────────────────────────────────────────

#[test]
fn test_pipeline_step_by_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    // Access step 0 — step(0) == current_step()
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "pipeline.step(0).get('index')"])
        .assert()
        .success()
        .stdout("0\n");
}

// ─── step.set(key, value) ────────────────────────────────────────────────────

#[test]
fn test_pipeline_set_exit_via_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "set_exit.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('exit', 42)
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .code(42);
}

#[test]
fn test_pipeline_set_output_format_via_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"key": "value"}"#);
    let mold = setup_mold(
        &dir,
        "set_fmt.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('output_format', 'json')
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout(predicate::str::contains("key"));
}

#[test]
fn test_pipeline_set_exit_wrong_type_error() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "bad_exit.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('exit', 'not_an_int')
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("integer"));
}

// ─── pipeline.insert_next() / append() ───────────────────────────────────────

#[test]
fn test_pipeline_insert_next_inline() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"10"#);
    let mold = setup_mold(
        &dir,
        "inject.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.insert_next(Step.create(expr="data * 2"))
    return data
"#,
    );
    // Step 1: returns 10, inserts "data * 2" as next step
    // Step 2 (injected): returns 10 * 2 = 20
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout("20\n");
}

#[test]
fn test_pipeline_append_inline() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"5"#);
    let mold = setup_mold(
        &dir,
        "append_step.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(expr="data + 100"))
    return data
"#,
    );
    // Step 1: returns 5, appends "data + 100"
    // Step 2 (appended): returns 105
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout("105\n");
}

#[test]
fn test_pipeline_max_duration_is_shared_by_the_whole_chain() {
    let dir = assert_fs::TempDir::new().unwrap();
    let sandbox = setup_input(
        &dir,
        "sandbox.toml",
        "[sandbox]\nmax_duration = \"20ms\"\nmax_memory = \"unlimited\"\n",
    );
    let mut cmd = assert_cmd::cargo_bin_cmd!("fimod");
    cmd.arg("shape")
        .args(["--no-input", "--sandbox-file", &sandbox]);
    for _ in 0..2_000 {
        cmd.args(["-e", "data"]);
    }
    cmd.timeout(Duration::from_secs(5))
        .assert()
        .code(137)
        .stderr(predicate::str::contains("max_duration"));
}

#[test]
fn test_pipeline_rejects_an_exhausted_duration_before_starting_a_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let sandbox = setup_input(
        &dir,
        "sandbox.toml",
        "[sandbox]\nmax_duration = \"0ms\"\nmax_memory = \"unlimited\"\n",
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "--sandbox-file", &sandbox, "-e", "1"])
        .assert()
        .code(137)
        .stderr(predicate::str::contains("max_duration"));
}

#[test]
fn test_pipeline_self_injection_hits_global_step_limit() {
    let dir = assert_fs::TempDir::new().unwrap();
    let sandbox = setup_input(
        &dir,
        "sandbox.toml",
        "[sandbox]\nmax_duration = \"unlimited\"\nmax_memory = \"unlimited\"\n",
    );
    let recursive = r#"[pipeline.append(Step.create(expr=args["loop"], args={"loop": args["loop"]})) for _ in range(100)] and data"#;

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "--sandbox-file", &sandbox])
        .args(["--arg", &format!("loop={recursive}")])
        .args(["-e", recursive])
        .timeout(Duration::from_secs(5))
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "dynamic pipeline step limit exceeded",
        ));
}

#[test]
fn test_pipeline_insert_next_updates_length() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"0"#);
    let mold = setup_mold(
        &dir,
        "inject_and_count.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.insert_next(Step.create(expr="pipeline.length()"))
    return data
"#,
    );
    // Step 1: inserts a step → chain now has 2 steps; returns 0
    // Step 2 (injected): returns pipeline.length() = 2
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout("2\n");
}

// ─── Step.create() constructor ───────────────────────────────────────────────────────

#[test]
fn test_pipe_missing_mold_or_expr_error() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "bad_pipe.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.insert_next(Step.create())
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mold=").or(predicate::str::contains("expr=")));
}

// ─── Step.get(key) — read API for Step instance ──────────────────────────────

#[test]
fn test_step_get_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "get_index.py",
        r#"
def transform(data, pipeline, **_):
    return pipeline.current_step().get('index')
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .success()
        .stdout("0\n");
}

#[test]
fn test_step_get_unknown_field_error() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "get_unknown.py",
        r#"
def transform(data, pipeline, **_):
    return pipeline.current_step().get('foo')
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Step.get('foo')")
                .and(predicate::str::contains("unknown field")),
        );
}

#[test]
fn test_step_direct_attribute_access_fails() {
    // After (b): Step Dataclass has no readable attrs. Direct attribute access
    // (`step.index`) must fail — only `step.get('index')` is valid.
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "attr_direct.py",
        r#"
def transform(data, pipeline, **_):
    return pipeline.current_step().index
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure();
}

#[test]
fn test_get_method_on_pipeline_error() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "pipeline_get.py",
        r#"
def transform(data, pipeline, **_):
    return pipeline.get('x')
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("get() is not a pipeline method"));
}

// ─── B1: stale error messages from old API ───────────────────────────────────
// Messages must reference Step.set(...) wording, not the old PipelineStep / step['key'].

#[test]
fn test_b1_set_readonly_field_message() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "ro.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('index', 0)
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Step.set('index')")
                .and(predicate::str::contains("read-only")),
        );
}

#[test]
fn test_b1_set_unknown_field_message() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "unknown.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('foo', 1)
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Step.set('foo')")
                .and(predicate::str::contains("unknown field")),
        );
}

#[test]
fn test_b1_set_exit_wrong_type_message() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "wrong_type.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('exit', 'abc')
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("Step.set('exit')").and(predicate::str::contains("integer")),
        );
}

// ─── B2: output_file mutation on future step is propagated (last write wins) ─

#[test]
fn test_b2_output_file_on_future_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"5"#);
    let out_path = dir.path().join("out.json");
    let mold = setup_mold(
        &dir,
        "set_outfile.py",
        &format!(
            r#"
def transform(data, pipeline, **_):
    pipeline.step(1).set('output_file', "{}")
    return data
"#,
            out_path.display()
        ),
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "-e", "data + 100"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());

    let content = std::fs::read_to_string(&out_path).expect("output file should be created");
    assert_eq!(content.trim(), "105");
}

// ─── B4: pipeline.step(-1) rejects negative index with explicit message ──────

#[test]
fn test_b4_step_negative_index() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-e", "pipeline.step(-1).index"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("non-negative"));
}

// ─── B5: pipeline-wide flags propagated to future Step Dataclass ─────────────
// in_place / slurp / no_input must reflect MoldContext, not be hardcoded false.

#[test]
fn test_b5_in_place_propagated_to_future_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "check_in_place.py",
        r#"
def transform(data, pipeline, **_):
    s1 = pipeline.step(1)
    pipeline.current_step().set('exit', 0 if s1.get('in_place') else 1)
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--in-place", "-m", &mold, "-e", "data"])
        .assert()
        .code(0);
}

#[test]
fn test_b5_slurp_propagated_to_future_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "check_slurp.py",
        r#"
def transform(data, pipeline, **_):
    s1 = pipeline.step(1)
    pipeline.current_step().set('exit', 0 if s1.get('slurp') else 1)
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "--slurp", "-m", &mold, "-e", "data"])
        .assert()
        .code(0);
}

#[test]
fn test_b5_no_input_propagated_to_future_step() {
    let dir = assert_fs::TempDir::new().unwrap();
    let mold = setup_mold(
        &dir,
        "check_no_input.py",
        r#"
def transform(data, pipeline, **_):
    s1 = pipeline.step(1)
    pipeline.current_step().set('exit', 0 if s1.get('no_input') else 1)
    return [1]
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["--no-input", "-m", &mold, "-e", "data"])
        .assert()
        .code(0);
}

// ─── P1: Step.create(args={...}) propagates merged args (step wins on conflict) ─

#[test]
fn test_p1_step_create_args_merged_step_wins() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "inject_args.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.insert_next(Step.create(
        expr="args.get('strict','def') + '|' + args.get('cli_only','def')",
        args={"strict": "STEP"}
    ))
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--arg",
            "cli_only=CLI",
            "--arg",
            "strict=CLI_strict",
            "-m",
            &mold,
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout("STEP|CLI");
}

#[test]
fn test_p1_step_create_args_are_cast_by_target_mold_directives() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let target = setup_mold(
        &dir,
        "target_typed.py",
        r#"# fimod: arg=limit:int
def transform(data, args, **_):
    return args["limit"] + data
"#,
    );
    let injector = setup_mold(
        &dir,
        "inject_typed.py",
        &format!(
            r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(mold={target:?}, args={{"limit": "41"}}))
    return data
"#
        ),
    );

    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &injector, "--output-format", "json"])
        .assert()
        .success()
        .stdout("42\n");
}

// ─── C3: bare-kwargs path on insert_next/append is removed (single contract) ─

#[test]
fn test_c3_kwargs_path_rejected() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "kwargs_path.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.insert_next(expr='data * 2')
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Step.create"));
}

// ─── C2: output_format mutation on future step propagates to serialization ───
// Mutation must reach ctx.format_override (effective serialization), not just
// the readable step['output_format'] attribute.

#[test]
fn test_c2_output_format_future_serialization() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"{"x": 1}"#);
    let mold = setup_mold(
        &dir,
        "set_future_fmt.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.step(1).set('output_format', 'yaml')
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold, "-e", "data"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("x:"));
}

// ─── C1: type guards on dispatch_method ──────────────────────────────────────
// Pipeline-only methods must reject Step receivers; Step-only methods must reject
// Pipeline receivers.

#[test]
fn test_c1_pipeline_method_on_step_length() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "step_length.py",
        r#"
def transform(data, pipeline, **_):
    return pipeline.current_step().length()
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("length() is not a Step method"));
}

#[test]
fn test_c1_pipeline_method_on_step_insert_next() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "step_insert.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().insert_next(Step.create(expr="data"))
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "insert_next() is not a Step method",
        ));
}

#[test]
fn test_c1_set_method_on_pipeline() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "pipeline_set.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.set('exit', 1)
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains("set() is not a pipeline method"));
}

// ─── P2: step.get('args') — Sens 1 ───────────────────────────────────────────
// Current step → merged dict (identical to the `args` param the mold receives).
// Future step → spec args (the dict given to Step.create), or {} if none.

#[test]
fn test_p2_current_args_empty() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-e",
            "pipeline.current_step().get('args')",
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout("{}\n");
}

#[test]
fn test_p2_current_args_cli_only() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--arg",
            "foo=bar",
            "-e",
            "pipeline.current_step().get('args')",
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout("{\"foo\":\"bar\"}\n");
}

#[test]
fn test_p2_current_args_matches_args_param_after_inject() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let injector = setup_mold(
        &dir,
        "injector.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(
        expr="pipeline.current_step().get('args') == args",
        args={"k1": "spec", "k3": "spec_only"}
    ))
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "--arg",
            "k1=cli",
            "--arg",
            "k2=cli_only",
            "-m",
            &injector,
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout("true\n");
}

// `pipeline.append` from step 0 is visible only from step 1 onwards (C4 snapshot
// semantics). So the future-step tests use a 3-step chain: step 0 = injector,
// step 1 = inline reader of step(2), step 2 = the injected step.

#[test]
fn test_p2_future_args_with_spec() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let injector = setup_mold(
        &dir,
        "inject_with_args.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(expr="data", args={"foo": "bar"}))
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &injector,
            "-e",
            "pipeline.step(2).get('args')",
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout("{\"foo\":\"bar\"}\n");
}

#[test]
fn test_p2_future_args_without_spec() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let injector = setup_mold(
        &dir,
        "inject_no_args.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(expr="data"))
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &injector,
            "-e",
            "pipeline.step(2).get('args')",
            "--output-format",
            "json-compact",
        ])
        .assert()
        .success()
        .stdout("{}\n");
}

// ─── P3: step.set('args', {...}) — future-only mutation ──────────────────────
// `set('args', dict)` on a future step replaces the entire args block.
// Forbidden on the current step (args was already passed to transform).

#[test]
fn test_p3_set_args_on_current_rejected() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let mold = setup_mold(
        &dir,
        "set_args_current.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.current_step().set('args', {'k': 'v'})
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args(["-i", &input, "-m", &mold])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Step.set('args'): can only be set on a future step",
        ));
}

// `pipeline.step(j)` from step 0 is bound by snapshot semantics (C4): a step
// injected by step 0 is only visible from step 1 onwards. So replace/visibility
// tests use a 3-step chain: step 0 = injector, step 1 = setter, step 2 = target.

#[test]
fn test_p3_set_args_replaces_full_block() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let injector = setup_mold(
        &dir,
        "inject_for_replace.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(
        expr="args.get('old','none') + '|' + args.get('new','none')",
        args={"old": "A"}
    ))
    return data
"#,
    );
    // Step 1 sets step(2).args to a new block; step 2 (the appended target)
    // reads its merged args. If REPLACE semantics hold, 'old' is gone → "none|B".
    // If it were a merge, output would be "A|B".
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &injector,
            "-e",
            "pipeline.step(2).set('args', {'new':'B'}) or data",
            "--output-format",
            "txt",
        ])
        .assert()
        .success()
        .stdout("none|B");
}

#[test]
fn test_p3_set_args_non_dict_rejected() {
    let dir = assert_fs::TempDir::new().unwrap();
    let input = setup_input(&dir, "data.json", r#"1"#);
    let injector = setup_mold(
        &dir,
        "inject_for_str.py",
        r#"
def transform(data, pipeline, **_):
    pipeline.append(Step.create(expr="data"))
    return data
"#,
    );
    assert_cmd::cargo_bin_cmd!("fimod")
        .arg("shape")
        .args([
            "-i",
            &input,
            "-m",
            &injector,
            "-e",
            "pipeline.step(2).set('args', 'not a dict') or data",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Step.set('args'): value must be a dict",
        ));
}
