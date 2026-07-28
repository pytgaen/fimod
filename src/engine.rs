use std::borrow::Cow;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::{Context, Result};
use monty::{MontyRun, RunProgress};
use monty_types::{
    CompileOptions, DictPairs, ExcType, ExtFunctionResult, LimitedTracker, MontyDate,
    MontyDateTime, MontyException, MontyObject, NameLookupResult, OsFunctionCall, PrintWriter,
    PrintWriterCallback, ResourceLimits,
};
use serde_json::{Number, Value};

use crate::convert::json_to_monty;

/// What to do with a pending pipeline step (injected by the mold at runtime).
#[derive(Debug)]
pub enum PendingOp {
    InsertNext,
    Append,
}

/// A step to be inserted into the pipeline after the current step completes.
#[derive(Debug)]
pub struct PendingStep {
    pub op: PendingOp,
    /// Normalized spec: `{"mold"|"expr": "...", "input_format"?: ..., ...}`.
    pub spec: Value,
}

/// A mutation requested by a mold on a future pipeline step's options.
#[derive(Debug)]
pub struct PendingMutation {
    pub step_idx: usize,
    pub key: String,
    pub value: Value,
}

/// Result of executing a single mold step.
///
/// `value` is `MontyObject` (not `serde_json::Value`) so that `execute_chain`
/// can thread results between steps without paying the round-trip
/// conversion cost. Convert at the chain boundary via `crate::convert::monty_to_json`.
pub struct MoldExecResult {
    pub value: MontyObject,
    pub exit_code: Option<i32>,
    pub format_override: Option<String>,
    pub output_file: Option<String>,
    pub pending_steps: Vec<PendingStep>,
    pub pending_mutations: Vec<PendingMutation>,
}

/// Return type of mold execution.
pub type MoldResult = Result<MoldExecResult>;
use crate::dotpath;
use crate::env_helpers;
use crate::exit_control;
use crate::format_control;
use crate::gatekeeper;
use crate::hash;
use crate::iter_helpers;
use crate::mold::{MoldArgSpec, MoldArgType};
use crate::msg;
use crate::regex;
use crate::sandbox::SandboxPolicy;
use crate::template;

/// Exit code used when the sandbox aborts execution (time or memory limit exceeded).
/// 128 + 9 (SIGKILL-ish); mirrors OOM-killer convention and stands out from generic failures.
pub const SANDBOX_EXPLODED_EXIT_CODE: i32 = 137;

/// Error returned when a sandbox limit is exceeded. Carries the stderr message the CLI should print.
#[derive(Debug)]
pub struct SandboxLimitExceeded {
    pub message: String,
}

impl std::fmt::Display for SandboxLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SandboxLimitExceeded {}

/// Custom PrintWriterCallback that redirects Monty's print() output to stderr.
/// Used in --debug mode so that Python print statements don't corrupt stdout.
struct StderrPrint;

impl PrintWriterCallback for StderrPrint {
    fn stdout_write(&mut self, output: Cow<'_, str>) -> Result<(), MontyException> {
        eprint!("{output}");
        Ok(())
    }

    fn stdout_push(&mut self, end: char) -> Result<(), MontyException> {
        eprint!("{end}");
        Ok(())
    }
}

/// Static pipeline metadata exposed to molds via the `pipeline` Python API.
///
/// Created once per pipeline run (or per slurp invocation) and shared by every
/// step in the chain via `MoldOptions::metadata`. Lives here next to
/// `MoldOptions` so a future field stays in one place instead of having to be
/// added to both structs.
pub struct PipelineMetadata<'a> {
    pub input: Option<&'a str>,
    pub output: Option<&'a str>,
    pub input_format: Option<&'a str>,
    pub output_format: Option<&'a str>,
    pub in_place: bool,
    pub slurp: bool,
    pub no_input: bool,
}

/// Runtime options for mold execution.
pub struct MoldOptions<'a> {
    pub extra_args: &'a [(String, String)],
    pub arg_specs: &'a [MoldArgSpec],
    pub env_value: &'a Value,
    pub headers_value: &'a Value,
    pub debug: bool,
    pub msg_level: u8,
    pub mold_base_dir: Option<&'a str>,
    pub policy: &'a SandboxPolicy,
    pub metadata: &'a PipelineMetadata<'a>,
    // Pipeline state — used to build the `pipeline` parameter.
    pub current_step_idx: usize,
    pub total_steps: usize,
    /// Serialized specs of steps after the current one (for `pipeline.step(i)`).
    pub remaining_steps: Vec<Value>,
    /// Per-step override of the input format (when a prior step called
    /// `set_input_format`); falls back to `metadata.input_format` otherwise.
    pub input_format: Option<&'a str>,
    /// Per-step override of the output format (`set_output_format` mutation).
    pub output_format: Option<&'a str>,
    /// Set when a prior step's `pipeline.step(j).set('output_file', ...)`
    /// targets this step — seeds `ctx.output_file` before the script runs.
    pub output_file_override: Option<String>,
    /// Set when a prior step's `pipeline.step(j).set('output_format', ...)`
    /// targets this step — seeds `ctx.format_override` before the script runs.
    pub format_override_init: Option<String>,
    /// Per-step args injected via `Step.create(args={...})`. When `Some`, merged
    /// with `extra_args` (CLI) — step values win on key conflict. `None` =
    /// step receives only CLI args (current behaviour).
    pub step_args: Option<serde_json::Value>,
}

/// Internal runtime context — extends MoldOptions with mutable shared state.
struct MoldContext<'a> {
    debug: bool,
    msg_level: u8,
    mold_base_dir: Option<&'a str>,
    exit_code: Arc<Mutex<Option<i32>>>,
    format_override: Arc<Mutex<Option<String>>>,
    output_file: Arc<Mutex<Option<String>>>,
    policy: &'a SandboxPolicy,
    chain_started_at: Option<Instant>,
    pending_steps: Arc<Mutex<Vec<PendingStep>>>,
    pending_mutations: Arc<Mutex<Vec<PendingMutation>>>,
    current_step_idx: usize,
    total_steps: usize,
    remaining_steps: &'a [Value],
    // Step metadata for building Step Dataclasses.
    input_path: Option<&'a str>,
    output_path: Option<&'a str>,
    in_place: bool,
    slurp: bool,
    no_input: bool,
    input_format: Option<&'a str>,
    output_format: Option<&'a str>,
    /// Merged args dict (CLI ∪ Step.create.args, spec wins) — exactly what
    /// `transform()` receives as the `args` parameter. Read by
    /// `current_step().get('args')`.
    args_value: Value,
}

/// Check whether a name is a known external function exposed to molds.
fn is_external_function(name: &str) -> bool {
    regex::EXTERNAL_FUNCTIONS.contains(&name)
        || dotpath::EXTERNAL_FUNCTIONS.contains(&name)
        || iter_helpers::EXTERNAL_FUNCTIONS.contains(&name)
        || hash::EXTERNAL_FUNCTIONS.contains(&name)
        || exit_control::EXTERNAL_FUNCTIONS.contains(&name)
        || format_control::EXTERNAL_FUNCTIONS.contains(&name)
        || msg::EXTERNAL_FUNCTIONS.contains(&name)
        || gatekeeper::EXTERNAL_FUNCTIONS.contains(&name)
        || env_helpers::EXTERNAL_FUNCTIONS.contains(&name)
        || template::EXTERNAL_FUNCTIONS.contains(&name)
}

/// Route an external function call to the correct module.
fn dispatch_external(
    name: &str,
    args: Vec<MontyObject>,
    _kwargs: Vec<(MontyObject, MontyObject)>,
    ctx: &MoldContext<'_>,
) -> Result<MontyObject> {
    if regex::EXTERNAL_FUNCTIONS.contains(&name) {
        regex::dispatch(name, args)
    } else if dotpath::EXTERNAL_FUNCTIONS.contains(&name) {
        dotpath::dispatch(name, args)
    } else if iter_helpers::EXTERNAL_FUNCTIONS.contains(&name) {
        iter_helpers::dispatch(name, args)
    } else if hash::EXTERNAL_FUNCTIONS.contains(&name) {
        hash::dispatch(name, args)
    } else if exit_control::EXTERNAL_FUNCTIONS.contains(&name) {
        exit_control::dispatch(name, args, &ctx.exit_code)
    } else if format_control::EXTERNAL_FUNCTIONS.contains(&name) {
        format_control::dispatch(name, args, &ctx.format_override, &ctx.output_file)
    } else if msg::EXTERNAL_FUNCTIONS.contains(&name) {
        msg::dispatch(name, args, ctx.msg_level)
    } else if gatekeeper::EXTERNAL_FUNCTIONS.contains(&name) {
        gatekeeper::dispatch(name, args, &ctx.exit_code)
    } else if env_helpers::EXTERNAL_FUNCTIONS.contains(&name) {
        env_helpers::dispatch(name, args)
    } else if template::EXTERNAL_FUNCTIONS.contains(&name) {
        template::dispatch(name, args, ctx.mold_base_dir)
    } else {
        anyhow::bail!("Unknown external function: {name}")
    }
}

// ─── type_id constants for Pipeline Dataclasses ────────────────────────────

const PIPELINE_TYPE_ID: u64 = 0x6669_6d6f_6450_6970;
const STEP_CLASS_TYPE_ID: u64 = 0x6669_6d6f_6453_636c; // Step class object (for Step.create)
const STEP_TYPE_ID: u64 = 0x6669_6d6f_6453_7470; // live Step instance
const STEP_SPEC_TYPE_ID: u64 = 0x6669_6d6f_6453_7063; // Step spec (from Step.create)

fn str_opt_to_monty(s: Option<&str>) -> MontyObject {
    match s {
        Some(v) => MontyObject::String(v.to_string()),
        None => MontyObject::None,
    }
}

fn build_args_value(opts: &MoldOptions<'_>) -> Result<Value> {
    let mut merged: serde_json::Map<String, Value> = opts
        .extra_args
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    if let Some(Value::Object(step_args)) = &opts.step_args {
        for (k, v) in step_args {
            merged.insert(k.clone(), v.clone());
        }
    }
    apply_arg_specs(&mut merged, opts.arg_specs)?;
    Ok(Value::Object(merged))
}

fn apply_arg_specs(args: &mut serde_json::Map<String, Value>, specs: &[MoldArgSpec]) -> Result<()> {
    for spec in specs {
        if let MoldArgType::Unknown(raw) = &spec.arg_type {
            anyhow::bail!("arg error: {} unknown type {:?}", spec.name, raw);
        }
        if spec.default.is_some() && !spec.optional {
            anyhow::bail!(
                "arg error: {} default requires optional marker (?)",
                spec.name
            );
        }

        match args.remove(&spec.name) {
            Some(value) => {
                let coerced = coerce_arg_value(spec, value, "got")?;
                args.insert(spec.name.clone(), coerced);
            }
            None if spec.optional => {
                if let Some(default) = &spec.default {
                    let coerced = coerce_arg_default(spec, default)?;
                    args.insert(spec.name.clone(), coerced);
                }
            }
            None => {
                anyhow::bail!(
                    "arg error: {} is required (expected {})",
                    spec.name,
                    spec.arg_type.label()
                );
            }
        }
    }
    Ok(())
}

fn coerce_arg_default(spec: &MoldArgSpec, raw: &str) -> Result<Value> {
    if raw.trim() == "None" {
        return Ok(Value::Null);
    }

    match spec.arg_type {
        MoldArgType::Json => serde_json::from_str(raw.trim())
            .or_else(|_| serde_json::from_str(&unquote_arg_default(raw)))
            .map_err(|_| {
                anyhow::anyhow!(
                    "arg error: {} default expected json, got {:?}",
                    spec.name,
                    raw
                )
            }),
        _ => coerce_arg_value(spec, Value::String(unquote_arg_default(raw)), "default"),
    }
}

fn coerce_arg_value(spec: &MoldArgSpec, value: Value, got_label: &str) -> Result<Value> {
    if value.is_null() {
        if spec.optional || matches!(spec.arg_type, MoldArgType::Json) {
            return Ok(Value::Null);
        }
        anyhow::bail!(
            "arg error: {} expected {}, {} null",
            spec.name,
            spec.arg_type.label(),
            got_label
        );
    }

    match spec.arg_type {
        MoldArgType::Str => match value {
            Value::String(s) => Ok(Value::String(s)),
            other => arg_type_error(spec, got_label, &other),
        },
        MoldArgType::Int => coerce_int_arg(spec, value, got_label),
        MoldArgType::Float => coerce_float_arg(spec, value, got_label),
        MoldArgType::Bool => coerce_bool_arg(spec, value, got_label),
        MoldArgType::Json => coerce_json_arg(spec, value, got_label),
        MoldArgType::Unknown(_) => unreachable!("unknown arg type checked before coercion"),
    }
}

fn coerce_int_arg(spec: &MoldArgSpec, value: Value, got_label: &str) -> Result<Value> {
    let parsed = match &value {
        Value::Number(n) => n
            .as_i64()
            .or_else(|| n.as_u64().and_then(|u| i64::try_from(u).ok())),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    };
    match parsed {
        Some(i) => Ok(Value::Number(i.into())),
        None => arg_type_error(spec, got_label, &value),
    }
}

fn coerce_float_arg(spec: &MoldArgSpec, value: Value, got_label: &str) -> Result<Value> {
    let parsed = match &value {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        _ => None,
    };
    match parsed.and_then(Number::from_f64) {
        Some(n) => Ok(Value::Number(n)),
        None => arg_type_error(spec, got_label, &value),
    }
}

fn coerce_bool_arg(spec: &MoldArgSpec, value: Value, got_label: &str) -> Result<Value> {
    match &value {
        Value::Bool(_) => Ok(value),
        Value::String(s) if s.eq_ignore_ascii_case("true") => Ok(Value::Bool(true)),
        Value::String(s) if s.eq_ignore_ascii_case("false") => Ok(Value::Bool(false)),
        _ => arg_type_error(spec, got_label, &value),
    }
}

fn coerce_json_arg(spec: &MoldArgSpec, value: Value, got_label: &str) -> Result<Value> {
    match value {
        Value::String(s) => serde_json::from_str(&s).map_err(|_| {
            anyhow::anyhow!(
                "arg error: {} expected json, {} {:?}",
                spec.name,
                got_label,
                s
            )
        }),
        other => Ok(other),
    }
}

fn arg_type_error<T>(spec: &MoldArgSpec, got_label: &str, value: &Value) -> Result<T> {
    anyhow::bail!(
        "arg error: {} expected {}, {} {}",
        spec.name,
        spec.arg_type.label(),
        got_label,
        format_arg_value(value)
    )
}

fn format_arg_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("{value:?}"))
}

fn unquote_arg_default(raw: &str) -> String {
    let s = raw.trim();
    if s.len() < 2 {
        return s.to_string();
    }
    let q = s.as_bytes()[0];
    if (q != b'"' && q != b'\'') || s.as_bytes()[s.len() - 1] != q {
        return s.to_string();
    }

    let mut out = String::with_capacity(s.len() - 2);
    let mut chars = s[1..s.len() - 1].chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(next) if next == q as char || next == '\\' => out.push(next),
                Some(next) => {
                    out.push(c);
                    out.push(next);
                }
                None => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Build a Step Dataclass with no readable attrs — all reads must go through
/// `step.get('key')`, all writes through `step.set('key', value)`.
/// Only `_step_idx` is stored internally to identify which step this is.
fn build_step_dc(step_idx: usize) -> MontyObject {
    let attrs: Vec<(MontyObject, MontyObject)> = vec![(
        MontyObject::String("_step_idx".into()),
        MontyObject::Int(step_idx as i64),
    )];
    MontyObject::Dataclass {
        name: "Step".to_string(),
        type_id: STEP_TYPE_ID,
        field_names: vec![],
        attrs: DictPairs::from(attrs),
        frozen: false,
    }
}

fn build_current_step_dc(ctx: &MoldContext<'_>) -> MontyObject {
    build_step_dc(ctx.current_step_idx)
}

fn build_future_step_dc(step_idx: usize) -> MontyObject {
    build_step_dc(step_idx)
}

fn get_dc_attr<'a>(dc: &'a MontyObject, key: &str) -> Option<&'a MontyObject> {
    if let MontyObject::Dataclass { attrs, .. } = dc {
        for (k, v) in attrs {
            if let MontyObject::String(k_str) = k {
                if k_str == key {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn get_step_idx(dc: &MontyObject) -> Result<usize> {
    match get_dc_attr(dc, "_step_idx") {
        Some(MontyObject::Int(i)) => Ok(*i as usize),
        _ => anyhow::bail!("pipeline step: missing _step_idx attribute"),
    }
}

fn extract_int_arg(arg: &MontyObject, method_name: &str) -> Result<i64> {
    match arg {
        MontyObject::Int(i) => Ok(*i),
        _ => anyhow::bail!("{method_name}: argument must be an integer"),
    }
}

/// Dispatch a method call on a Pipeline, Step instance, or Step class Dataclass.
/// `args[0]` is always `self` (the Dataclass instance).
fn dispatch_method(
    name: &str,
    args: &[MontyObject],
    kwargs: &[(MontyObject, MontyObject)],
    ctx: &MoldContext<'_>,
) -> Result<MontyObject> {
    let receiver_type = args.first().and_then(|o| match o {
        MontyObject::Dataclass { type_id, .. } => Some(*type_id),
        _ => None,
    });

    if matches!(
        name,
        "current_step" | "step" | "length" | "insert_next" | "append"
    ) && receiver_type != Some(PIPELINE_TYPE_ID)
    {
        anyhow::bail!("{name}() is not a Step method (only on pipeline)");
    }
    if name == "set" && args.len() >= 3 && receiver_type != Some(STEP_TYPE_ID) {
        anyhow::bail!("set() is not a pipeline method (only on Step)");
    }
    if name == "get" && args.len() >= 2 && receiver_type != Some(STEP_TYPE_ID) {
        anyhow::bail!("get() is not a pipeline method (only on Step)");
    }

    match name {
        "current_step" => Ok(build_current_step_dc(ctx)),
        "step" => {
            let raw_idx = if args.len() > 1 {
                extract_int_arg(&args[1], "pipeline.step()")?
            } else {
                let mut found = None;
                for (k, v) in kwargs {
                    if matches!(k, MontyObject::String(s) if s == "i" || s == "index") {
                        found = Some(extract_int_arg(v, "pipeline.step()")?);
                        break;
                    }
                }
                found
                    .ok_or_else(|| anyhow::anyhow!("pipeline.step(): requires an index argument"))?
            };
            if raw_idx < 0 {
                anyhow::bail!("pipeline.step(): index must be non-negative");
            }
            let idx = raw_idx as usize;
            if idx == ctx.current_step_idx {
                return Ok(build_current_step_dc(ctx));
            }
            if idx > ctx.current_step_idx {
                let remaining_idx = idx - ctx.current_step_idx - 1;
                if ctx.remaining_steps.get(remaining_idx).is_none() {
                    anyhow::bail!(
                        "pipeline.step({idx}): index out of range (total={})",
                        ctx.total_steps
                    );
                }
                return Ok(build_future_step_dc(idx));
            }
            anyhow::bail!("pipeline.step({idx}): cannot access past steps")
        }
        "length" => Ok(MontyObject::Int(ctx.total_steps as i64)),
        "create" if matches!(args.first(), Some(MontyObject::Dataclass { type_id, .. }) if *type_id == STEP_CLASS_TYPE_ID) => {
            dispatch_step_create(kwargs)
        }
        "set" if args.len() >= 3 => {
            let step_idx = get_step_idx(&args[0])?;
            let key = crate::monty_args::expect_string_owned(&args[1], "Step.set() key")?;
            let value = crate::convert::monty_to_json(args[2].clone())
                .context("Step.set(): cannot convert value")?;
            set_step_field(step_idx, &key, value, ctx)
        }
        "get" if args.len() >= 2 => {
            let step_idx = get_step_idx(&args[0])?;
            let key = crate::monty_args::expect_string_owned(&args[1], "Step.get() key")?;
            get_step_field(step_idx, &key, ctx)
        }
        "insert_next" | "append" => {
            let spec = extract_step_spec(name, args, kwargs)?;
            ctx.pending_steps.lock().unwrap().push(PendingStep {
                op: if name == "insert_next" {
                    PendingOp::InsertNext
                } else {
                    PendingOp::Append
                },
                spec,
            });
            Ok(MontyObject::None)
        }
        _ => anyhow::bail!("pipeline: unknown method '{name}'"),
    }
}

/// Extract a normalized step spec from `insert_next`/`append` — only `Step.create(...)` accepted.
fn extract_step_spec(
    method_name: &str,
    args: &[MontyObject],
    _kwargs: &[(MontyObject, MontyObject)],
) -> Result<Value> {
    if let Some(MontyObject::Dataclass { type_id, attrs, .. }) = args.get(1) {
        if *type_id == STEP_SPEC_TYPE_ID {
            return dataclass_attrs_to_json(attrs);
        }
    }
    anyhow::bail!("pipeline.{method_name}(): argument must be a Step.create(...) spec");
}

fn dataclass_attrs_to_json(attrs: &DictPairs) -> Result<Value> {
    let mut map = serde_json::Map::new();
    for (k, v) in attrs {
        if let MontyObject::String(key) = k {
            if !key.starts_with('_') {
                map.insert(
                    key.clone(),
                    crate::convert::monty_to_json(v.clone())
                        .context("Step spec: cannot convert field value")?,
                );
            }
        }
    }
    Ok(Value::Object(map))
}

fn get_step_field(step_idx: usize, key: &str, ctx: &MoldContext<'_>) -> Result<MontyObject> {
    const READABLE: &[&str] = &[
        "index",
        "input_format",
        "output_format",
        "input",
        "output",
        "in_place",
        "slurp",
        "no_input",
        "args",
    ];
    if !READABLE.contains(&key) {
        anyhow::bail!("Step.get('{key}'): unknown field");
    }

    let is_current = step_idx == ctx.current_step_idx;

    Ok(match key {
        "index" => MontyObject::Int(step_idx as i64),
        "in_place" => MontyObject::Bool(ctx.in_place),
        "slurp" => MontyObject::Bool(ctx.slurp),
        "no_input" => MontyObject::Bool(ctx.no_input),
        "input" => {
            if is_current {
                str_opt_to_monty(ctx.input_path)
            } else {
                MontyObject::None
            }
        }
        "output" => {
            if is_current {
                str_opt_to_monty(ctx.output_path)
            } else {
                MontyObject::None
            }
        }
        "input_format" | "output_format" => {
            if is_current {
                let val = if key == "input_format" {
                    ctx.input_format
                } else {
                    ctx.output_format
                };
                str_opt_to_monty(val)
            } else if step_idx > ctx.current_step_idx {
                let remaining_idx = step_idx - ctx.current_step_idx - 1;
                let val = ctx
                    .remaining_steps
                    .get(remaining_idx)
                    .and_then(|spec| spec.get(key))
                    .and_then(Value::as_str);
                str_opt_to_monty(val)
            } else {
                MontyObject::None
            }
        }
        "args" => {
            if is_current {
                json_to_monty(&ctx.args_value)
            } else if step_idx > ctx.current_step_idx {
                let remaining_idx = step_idx - ctx.current_step_idx - 1;
                let spec_args = ctx
                    .remaining_steps
                    .get(remaining_idx)
                    .and_then(|spec| spec.get("args"))
                    .cloned()
                    .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
                json_to_monty(&spec_args)
            } else {
                MontyObject::None
            }
        }
        _ => unreachable!(),
    })
}

fn set_step_field(
    step_idx: usize,
    key: &str,
    value: Value,
    ctx: &MoldContext<'_>,
) -> Result<MontyObject> {
    const WRITABLE: &[&str] = &[
        "exit",
        "output_format",
        "input_format",
        "output_file",
        "args",
    ];
    const READONLY: &[&str] = &["index", "input", "output", "in_place", "slurp", "no_input"];
    /// Keys that only make sense to set on a future step — setting them on the
    /// current step is forbidden because the value has already been consumed
    /// (e.g. `args` was already passed to `transform()`).
    const WRITABLE_FUTURE_ONLY: &[&str] = &["args"];

    if READONLY.contains(&key) {
        anyhow::bail!("Step.set('{key}'): field is read-only");
    }
    if !WRITABLE.contains(&key) {
        anyhow::bail!("Step.set('{key}'): unknown field");
    }

    if step_idx == ctx.current_step_idx && WRITABLE_FUTURE_ONLY.contains(&key) {
        anyhow::bail!("Step.set('{key}'): can only be set on a future step");
    }

    if key == "args" && !value.is_object() {
        anyhow::bail!("Step.set('args'): value must be a dict");
    }

    if step_idx == ctx.current_step_idx && key != "input_format" {
        match key {
            "exit" => {
                let code = value
                    .as_i64()
                    .map(|v| v as i32)
                    .ok_or_else(|| anyhow::anyhow!("Step.set('exit'): value must be an integer"))?;
                *ctx.exit_code.lock().unwrap() = Some(code);
            }
            "output_format" => {
                let fmt = value
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("Step.set('output_format'): value must be a string")
                    })?
                    .to_string();
                *ctx.format_override.lock().unwrap() = Some(fmt);
            }
            "output_file" => {
                let path = value
                    .as_str()
                    .ok_or_else(|| {
                        anyhow::anyhow!("Step.set('output_file'): value must be a string")
                    })?
                    .to_string();
                *ctx.output_file.lock().unwrap() = Some(path);
            }
            _ => unreachable!(),
        }
    } else {
        ctx.pending_mutations.lock().unwrap().push(PendingMutation {
            step_idx,
            key: key.to_string(),
            value,
        });
    }
    Ok(MontyObject::None)
}

/// Build a Step spec Dataclass from `Step.create(...)` kwargs.
fn dispatch_step_create(kwargs: &[(MontyObject, MontyObject)]) -> Result<MontyObject> {
    let mut mold: Option<String> = None;
    let mut expr: Option<String> = None;
    let mut spec_attrs: Vec<(MontyObject, MontyObject)> = vec![(
        MontyObject::String("_type".into()),
        MontyObject::String("step_spec".into()),
    )];

    for (k, v) in kwargs {
        if let MontyObject::String(key) = k {
            match key.as_str() {
                "mold" => {
                    if let MontyObject::String(s) = v {
                        mold = Some(s.clone());
                        spec_attrs.push((MontyObject::String("mold".into()), v.clone()));
                    }
                }
                "expr" => {
                    if let MontyObject::String(s) = v {
                        expr = Some(s.clone());
                        spec_attrs.push((MontyObject::String("expr".into()), v.clone()));
                    }
                }
                "input_format" | "output_format" => {
                    spec_attrs.push((MontyObject::String(key.clone()), v.clone()));
                }
                "args" => {
                    let json_val = crate::convert::monty_to_json(v.clone())
                        .context("Step.create(args=...): cannot convert value")?;
                    if !json_val.is_object() {
                        anyhow::bail!("Step.create(): args must be a dict");
                    }
                    spec_attrs.push((MontyObject::String(key.clone()), v.clone()));
                }
                _ => {}
            }
        }
    }

    if mold.is_none() && expr.is_none() {
        anyhow::bail!("Step.create(): exactly one of `mold=` or `expr=` is required");
    }
    if mold.is_some() && expr.is_some() {
        anyhow::bail!("Step.create(): cannot specify both `mold=` and `expr=`");
    }

    Ok(MontyObject::Dataclass {
        name: "Step".to_string(),
        type_id: STEP_SPEC_TYPE_ID,
        field_names: vec![],
        attrs: DictPairs::from(spec_attrs),
        frozen: true,
    })
}

/// Execute a mold Python script against input data using Monty.
///
/// The script must define a `transform(data, **_)` function.
/// `args`, `env`, `headers`, and `pipeline` are passed as keyword arguments.
/// All parameters are always passed explicitly — no global variable injection.
///
/// Takes `data` as an owned `MontyObject` to avoid the json_to_monty conversion
/// when the caller has already built a MontyObject directly (e.g. csv_to_monty path).
pub fn execute_mold(script: &str, data: MontyObject, opts: &MoldOptions<'_>) -> MoldResult {
    execute_mold_with_chain_start(script, data, opts, None)
}

/// Execute one mold while charging its runtime to a chain-wide duration budget.
pub(crate) fn execute_mold_with_chain_start(
    script: &str,
    data: MontyObject,
    opts: &MoldOptions<'_>,
    chain_started_at: Option<Instant>,
) -> MoldResult {
    let merged_args = build_args_value(opts)?;
    let args_dict = json_to_monty(&merged_args);

    let env_obj = json_to_monty(opts.env_value);
    let headers_obj = json_to_monty(opts.headers_value);

    let pipeline_dc = MontyObject::Dataclass {
        name: "Pipeline".to_string(),
        type_id: PIPELINE_TYPE_ID,
        field_names: vec![],
        attrs: DictPairs::from(vec![]),
        frozen: false,
    };

    let input_names = vec![
        "data".to_string(),
        "args".to_string(),
        "env".to_string(),
        "headers".to_string(),
        "pipeline".to_string(),
    ];
    let inputs = vec![data, args_dict, env_obj, headers_obj, pipeline_dc];

    let full_script = format!(
        "{script}\ntransform(data, args=args, env=env, headers=headers, pipeline=pipeline)"
    );

    if opts.debug {
        eprintln!("[debug] script:");
        eprintln!("---");
        eprintln!("{}", full_script.trim_end());
        eprintln!("---");
    }

    let runner = MontyRun::new(
        full_script,
        "mold.py",
        input_names,
        CompileOptions::default(),
    )
    .context("Failed to compile mold script")?;

    let ctx = MoldContext {
        debug: opts.debug,
        msg_level: opts.msg_level,
        mold_base_dir: opts.mold_base_dir,
        exit_code: Arc::new(Mutex::new(None)),
        format_override: Arc::new(Mutex::new(opts.format_override_init.clone())),
        output_file: Arc::new(Mutex::new(opts.output_file_override.clone())),
        policy: opts.policy,
        chain_started_at,
        pending_steps: Arc::new(Mutex::new(Vec::new())),
        pending_mutations: Arc::new(Mutex::new(Vec::new())),
        current_step_idx: opts.current_step_idx,
        total_steps: opts.total_steps,
        remaining_steps: &opts.remaining_steps,
        input_path: opts.metadata.input,
        output_path: opts.metadata.output,
        in_place: opts.metadata.in_place,
        slurp: opts.metadata.slurp,
        no_input: opts.metadata.no_input,
        input_format: opts.input_format,
        output_format: opts.output_format,
        args_value: merged_args,
    };

    let value = run_loop(runner, inputs, &ctx)?;

    let exit_code = ctx.exit_code.lock().unwrap().take();
    let format_override = ctx.format_override.lock().unwrap().take();
    let output_file = ctx.output_file.lock().unwrap().take();
    let pending_steps = ctx.pending_steps.lock().unwrap().drain(..).collect();
    let pending_mutations = ctx.pending_mutations.lock().unwrap().drain(..).collect();

    Ok(MoldExecResult {
        value,
        exit_code,
        format_override,
        output_file,
        pending_steps,
        pending_mutations,
    })
}

fn run_loop(
    runner: MontyRun,
    inputs: Vec<MontyObject>,
    ctx: &MoldContext<'_>,
) -> Result<MontyObject> {
    let mut sp = StderrPrint;
    let mut limits = sandbox_resource_limits(ctx.policy);
    if let (Some(started_at), Some(max_duration)) = (ctx.chain_started_at, ctx.policy.max_duration)
    {
        let elapsed = started_at.elapsed();
        if elapsed >= max_duration {
            return Err(limit_exceeded(
                "max_duration",
                Some(format_duration(max_duration)),
            ));
        }
        limits = limits.max_duration(max_duration - elapsed);
    }
    let tracker = LimitedTracker::new(limits);
    let mut progress = runner
        .start(
            inputs,
            tracker,
            if ctx.debug {
                PrintWriter::Callback(&mut sp)
            } else {
                PrintWriter::Stdout
            },
        )
        .map_err(|e| translate_monty_error(e, ctx.policy))?;

    loop {
        match progress {
            RunProgress::Complete(result) => return Ok(result),
            RunProgress::FunctionCall(mut call) => {
                let function_name = call.function_name.clone();
                let method_call = call.method_call;
                let result = if method_call {
                    dispatch_method(&function_name, &call.args, &call.kwargs, ctx)
                        .map_err(|e| anyhow::anyhow!("Method call '{function_name}' failed: {e}"))?
                } else {
                    let args = std::mem::take(&mut call.args);
                    let kwargs = std::mem::take(&mut call.kwargs);
                    dispatch_external(&function_name, args, kwargs, ctx).map_err(|e| {
                        anyhow::anyhow!("External function '{function_name}' failed: {e}")
                    })?
                };
                let mut sp2 = StderrPrint;
                progress = call
                    .resume(
                        result,
                        if ctx.debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| translate_monty_error(e, ctx.policy))?;
            }
            RunProgress::OsCall(call) => {
                let mut sp2 = StderrPrint;
                progress = call
                    .resume_with(
                        if ctx.debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                        |function_call| {
                            let function_name = function_call.to_string();
                            let result = dispatch_os_call(function_call, ctx.policy);
                            if ctx.debug {
                                eprintln!(
                                    "[debug] OsCall {} -> {}",
                                    function_name,
                                    describe_os_result(&result)
                                );
                            }
                            result.into()
                        },
                    )
                    .map_err(|e| translate_monty_error(e, ctx.policy))?;
            }
            RunProgress::NameLookup(lookup) => {
                let name = lookup.name.clone();
                let result = if name == "Step" {
                    NameLookupResult::Value(MontyObject::Dataclass {
                        name: "Step".to_string(),
                        type_id: STEP_CLASS_TYPE_ID,
                        field_names: vec![],
                        attrs: DictPairs::from(vec![]),
                        frozen: true,
                    })
                } else if is_external_function(&name) {
                    NameLookupResult::Value(MontyObject::Function {
                        name,
                        docstring: None,
                    })
                } else {
                    NameLookupResult::Undefined
                };
                let mut sp2 = StderrPrint;
                progress = lookup
                    .resume(
                        result,
                        if ctx.debug {
                            PrintWriter::Callback(&mut sp2)
                        } else {
                            PrintWriter::Stdout
                        },
                    )
                    .map_err(|e| translate_monty_error(e, ctx.policy))?;
            }
            RunProgress::ResolveFutures(_) => {
                anyhow::bail!("Async futures are not supported in fimod molds");
            }
        }
    }
}

/// Build `ResourceLimits` from a `SandboxPolicy`.
pub fn sandbox_resource_limits(policy: &SandboxPolicy) -> ResourceLimits {
    let mut limits = ResourceLimits::new();
    if let Some(d) = policy.max_duration {
        limits = limits.max_duration(d);
    }
    if let Some(m) = policy.max_memory {
        limits = limits.max_memory(m);
    }
    limits
}

pub fn sandbox_os_call_result(
    function_call: OsFunctionCall,
    policy: &SandboxPolicy,
) -> ExtFunctionResult {
    dispatch_os_call(function_call, policy).into()
}

/// Resolve an `OsCall` result according to the policy.
///
/// Capability-deny defaults follow Python ergonomics:
/// - Clock (`date.today`, `datetime.now`): explicit `PermissionError` with actionable hint when denied, because returning `None` would crash downstream `.isoformat()` calls.
/// - `os.getenv(key)`: returns `None` silently when `key` is not in `allow_env` — mirrors the standard Python behavior for unset vars.
/// - `os.environ`: returns an empty dict when denied (no raise).
/// - `Path.*`: returns `None` (legacy behavior; proper filesystem gating lands with `[[mount]]`).
/// - `open()`: raises `PermissionError` when filesystem access is not implemented, because returning `None` would hand mold code a non-file object.
fn dispatch_os_call(function_call: OsFunctionCall, policy: &SandboxPolicy) -> OsCallOutcome {
    match function_call {
        OsFunctionCall::DateToday => {
            if policy.allow_clock {
                OsCallOutcome::Value(MontyObject::Date(current_date()))
            } else {
                OsCallOutcome::Error(permission_denied(clock_denied_message("date.today")))
            }
        }
        OsFunctionCall::DateTimeNow(_) => {
            if policy.allow_clock {
                OsCallOutcome::Value(MontyObject::DateTime(current_datetime()))
            } else {
                OsCallOutcome::Error(permission_denied(clock_denied_message("datetime.now")))
            }
        }
        OsFunctionCall::Getenv(args) => {
            OsCallOutcome::Value(lookup_env(&args.key, args.default, policy))
        }
        OsFunctionCall::GetEnviron => OsCallOutcome::Value(empty_environ()),
        call @ OsFunctionCall::Open(_) => OsCallOutcome::Error(call.on_no_handler()),
        OsFunctionCall::Exists(_)
        | OsFunctionCall::IsFile(_)
        | OsFunctionCall::IsDir(_)
        | OsFunctionCall::IsSymlink(_)
        | OsFunctionCall::ReadText(_)
        | OsFunctionCall::ReadBytes(_)
        | OsFunctionCall::Stat(_)
        | OsFunctionCall::Iterdir(_)
        | OsFunctionCall::Resolve(_)
        | OsFunctionCall::Absolute(_)
        | OsFunctionCall::WriteText(_)
        | OsFunctionCall::AppendText(_)
        | OsFunctionCall::WriteBytes(_)
        | OsFunctionCall::AppendBytes(_)
        | OsFunctionCall::Mkdir(_)
        | OsFunctionCall::Unlink(_)
        | OsFunctionCall::Rmdir(_)
        | OsFunctionCall::Rename(_) => OsCallOutcome::Value(MontyObject::None),
    }
}

fn clock_denied_message(function: &str) -> String {
    format!(
        "{function}() denied by sandbox policy — add `allow_clock = true` to your sandbox.toml (or run `fimod setup sandbox defaults`)"
    )
}

fn current_date() -> MontyDate {
    use chrono::{Datelike, Local};
    let now = Local::now().date_naive();
    MontyDate {
        year: now.year(),
        month: now.month() as u8,
        day: now.day() as u8,
    }
}

/// Naive datetime (no tz) matches Python's `datetime.now()` without args.
fn current_datetime() -> MontyDateTime {
    use chrono::{Datelike, Local, Timelike};
    let now = Local::now().naive_local();
    MontyDateTime {
        year: now.year(),
        month: now.month() as u8,
        day: now.day() as u8,
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        second: now.second() as u8,
        microsecond: now.nanosecond() / 1_000,
        offset_seconds: None,
        timezone_name: None,
    }
}

/// Result of dispatching an `OsCall` — either a return value or a Python exception.
enum OsCallOutcome {
    Value(MontyObject),
    Error(MontyException),
}

impl From<OsCallOutcome> for ExtFunctionResult {
    fn from(outcome: OsCallOutcome) -> Self {
        match outcome {
            OsCallOutcome::Value(v) => ExtFunctionResult::Return(v),
            OsCallOutcome::Error(e) => ExtFunctionResult::Error(e),
        }
    }
}

fn describe_os_result(outcome: &OsCallOutcome) -> String {
    match outcome {
        OsCallOutcome::Value(_) => "allowed".to_string(),
        OsCallOutcome::Error(_) => "denied".to_string(),
    }
}

fn permission_denied(msg: String) -> MontyException {
    MontyException::new(ExcType::PermissionError, Some(msg))
}

fn lookup_env(key: &str, default: MontyObject, policy: &SandboxPolicy) -> MontyObject {
    if !policy.env_allowed(key) {
        return MontyObject::None;
    }
    match std::env::var(key) {
        Ok(v) => MontyObject::String(v),
        Err(_) => default,
    }
}

fn empty_environ() -> MontyObject {
    MontyObject::Dict(DictPairs::from(Vec::<(MontyObject, MontyObject)>::new()))
}

/// Upgrades resource-limit exceptions (`TimeoutError`, `MemoryError`) into `SandboxLimitExceeded`
/// so the CLI can exit with 137.
pub fn translate_monty_error(err: MontyException, policy: &SandboxPolicy) -> anyhow::Error {
    match err.exc_type() {
        ExcType::TimeoutError => {
            limit_exceeded("max_duration", policy.max_duration.map(format_duration))
        }
        ExcType::MemoryError => limit_exceeded("max_memory", policy.max_memory.map(format_bytes)),
        _ => anyhow::anyhow!("Python error in mold:\n{err}"),
    }
}

fn limit_exceeded(kind: &str, limit: Option<String>) -> anyhow::Error {
    let limit = limit.unwrap_or_else(|| "n/a".to_string());
    anyhow::Error::new(SandboxLimitExceeded {
        message: format!("sandbox exploded: {kind} exceeded ({limit})"),
    })
}

fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 && secs.is_multiple_of(3600) {
        format!("{}h", secs / 3600)
    } else if secs >= 60 && secs.is_multiple_of(60) {
        format!("{}m", secs / 60)
    } else if secs > 0 {
        format!("{secs}s")
    } else {
        format!("{}ms", d.as_millis())
    }
}

fn format_bytes(b: usize) -> String {
    const KB: usize = 1_000;
    const MB: usize = 1_000_000;
    const GB: usize = 1_000_000_000;
    if b >= GB && b.is_multiple_of(GB) {
        format!("{}GB", b / GB)
    } else if b >= MB && b.is_multiple_of(MB) {
        format!("{}MB", b / MB)
    } else if b >= KB && b.is_multiple_of(KB) {
        format!("{}KB", b / KB)
    } else {
        format!("{b}B")
    }
}
