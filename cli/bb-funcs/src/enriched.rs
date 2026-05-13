//! Enriched function types that join bb-clang's parsed functions
//! with bb-sparse's Windows API metadata and bb-consts cross-references.
//!
//! This module lives in bb-funcs (not bb-clang) so that bb-clang stays
//! reusable without Windows API metadata dependencies. Both CLI and TUI
//! consume these types for rendering.

use std::collections::HashMap;
use std::fmt::Write;

use bb_arch::ToJson as AbiToJson;
use bb_clang::display::{format_abi_param, format_return_location, format_tags};
use bb_clang::{Constant, Function, Param, SourceLocation, ToJson};
use bb_cli::terminal_width;
use bb_consts_lib::{ConstFilter, collect_constants, collect_enums};
use bb_sdk::SdkMode;
use bb_sparse::{DriverMetadata, Entry, ParamMetadata};
use colored::Colorize;
use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table, presets};
use serde_json::{Value, json};

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A [`Function`] enriched with an optional sparse-metadata entry.
pub struct EnrichedFunction<'a> {
    pub function: &'a Function<'a>,
    pub entry: Option<Entry<'static>>,
}

impl<'a> EnrichedFunction<'a> {
    /// Resolve sparse metadata for `function`, preferring the dataset that
    /// matches the active `SdkMode`. Kernel mode tries `lookup_driver` first
    /// then falls back to `lookup_sdk`; user mode is reversed.
    #[must_use]
    pub fn new_ref(function: &'a Function<'a>, mode: SdkMode) -> Self {
        let name = function.get_name();
        let entry = lookup_for_mode(name, mode);
        Self { function, entry }
    }
}

/// Mode-aware sparse lookup: prefer the dataset that matches the SDK mode,
/// fall back to the other if the function isn't in the preferred one.
fn lookup_for_mode(name: &str, mode: SdkMode) -> Option<Entry<'static>> {
    match mode {
        SdkMode::Kernel => bb_sparse::lookup_driver(name)
            .map(Entry::Driver)
            .or_else(|| bb_sparse::lookup_sdk(name).map(Entry::Sdk)),
        SdkMode::User => bb_sparse::lookup_sdk(name)
            .map(Entry::Sdk)
            .or_else(|| bb_sparse::lookup_driver(name).map(Entry::Driver)),
    }
}

/// Resolved constant info for cross-referencing param values.
pub struct ConstantInfo {
    pub value: u64,
    pub location: Option<SourceLocation>,
}

/// A lookup table of constant name -> resolved info.
pub type ConstantLookup = HashMap<String, ConstantInfo>;

/// Build a [`ConstantLookup`] from collected constants.
#[must_use]
pub fn build_constant_lookup(constants: &[Constant]) -> ConstantLookup {
    let mut map = HashMap::new();
    for c in constants {
        if let Some(v) = c.get_value().as_u64() {
            map.insert(
                c.get_name().to_string(),
                ConstantInfo {
                    value: v,
                    location: c.get_location().cloned(),
                },
            );
        }
    }
    map
}

/// Build a [`ConstantLookup`] from a macro-preprocessed translation unit.
///
/// Collects all constants and enum constants from the TU and resolves
/// their values and source locations for cross-referencing with sparse
/// parameter values.
#[must_use]
pub fn build_constant_lookup_from_tu(tu: &clang::TranslationUnit) -> ConstantLookup {
    let no_filter = ConstFilter {
        header_filter: None,
        enum_pattern: None,
        const_pattern: None,
        case_sensitive: true,
        scoped_to_enum: false,
    };
    let enums = collect_enums(tu, &no_filter);
    let constants = collect_constants(tu, &no_filter);

    let mut lookup = build_constant_lookup(&constants);
    for e in &enums {
        for c in e.get_constants() {
            if let Some(v) = c.get_value().as_u64() {
                lookup.insert(
                    c.get_name().to_string(),
                    ConstantInfo {
                        value: v,
                        location: c.get_location().cloned(),
                    },
                );
            }
        }
    }
    lookup
}

/// Project a [`ConstantLookup`] down to a plain `name -> u64` map for
/// IRQL resolution. The location info isn't needed there.
#[must_use]
pub fn numeric_const_lookup(lookup: &ConstantLookup) -> HashMap<String, u64> {
    lookup.iter().map(|(k, v)| (k.clone(), v.value)).collect()
}

/* ─────────────────────── Full enriched detail view ─────────────────────── */

/// Render the full enriched detail view for a function.
#[must_use]
pub fn render_enriched_detail(
    f: &Function,
    mode: SdkMode,
    const_lookup: Option<&ConstantLookup>,
) -> String {
    let ef = EnrichedFunction::new_ref(f, mode);
    let entry = ef.entry;
    let mut out = String::new();

    render_prototype(&mut out, f, entry);
    render_header_tags(&mut out, f, entry);
    render_abi_section(&mut out, f);
    if let Some(entry) = entry {
        render_arguments_section(&mut out, f, entry, const_lookup);
        render_info_section(&mut out, entry);
        if let Some(drv) = entry.driver() {
            render_driver_section(&mut out, drv);
        }
    }

    out
}

/* ───────────────────────── Section renderers ────────────────────────────── */

/// Tags line + variants.
fn render_header_tags(out: &mut String, f: &Function, entry: Option<Entry<'_>>) {
    let mut tags = format_tags(f);
    if let Some(entry) = entry {
        let meta = entry.as_metadata();
        if let Some(dll) = meta.dll_display() {
            let lib = meta.lib_display().unwrap_or_else(|| "?".into());
            tags.push(format!("{dll} ({lib})"));
        }
    }
    let _ = writeln!(out, "  {}", tags.join("  ·  ").bright_black());

    if let Some(entry) = entry
        && let Some(api) = entry.as_metadata().api_metadata()
    {
        let names = api.names();
        if names.len() > 1 {
            let _ = writeln!(
                out,
                "  {} {}",
                "variants:".dimmed(),
                names.join(", ").bright_black()
            );
        }
    }
}

/// ABI section: stack note, param rows, return location.
fn render_abi_section(out: &mut String, f: &Function) {
    let _ = writeln!(out);
    let _ = writeln!(out, "  {}", "ABI".white().bold().underline());

    let params = f.get_params();
    if params.iter().any(bb_clang::Param::is_stack) {
        let _ = writeln!(
            out,
            "  {}",
            "callee-entry offsets (before prologue)".bright_black()
        );
    }
    let _ = writeln!(out);

    if params.is_empty() {
        let _ = writeln!(out, "  {}", "(no parameters)".dimmed());
    } else {
        for (i, p) in params.iter().enumerate() {
            let is_last = i == params.len() - 1;
            let connector = if is_last { "╰─" } else { "├─" };
            let _ = writeln!(out, "  {} {}", connector.dimmed(), format_abi_param(i, p));
        }
    }

    let ret_type = f.get_return_type_name().cyan();
    let ret_loc = format_return_location(f.get_return_location()).yellow();
    let _ = writeln!(out, "  {} {ret_loc}  {ret_type}", "╰".dimmed());
}

/// Arguments section: per-param constant values in tables.
fn render_arguments_section(
    out: &mut String,
    f: &Function,
    entry: Entry<'_>,
    const_lookup: Option<&ConstantLookup>,
) {
    let params = entry.as_metadata().params();
    let params_with_values: Vec<_> = f
        .get_params()
        .iter()
        .filter_map(|p| {
            let name = p.get_name()?;
            let pm = params.get(name)?;
            if pm.values.is_empty() {
                return None;
            }
            Some((name, pm))
        })
        .collect();

    if params_with_values.is_empty() {
        return;
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  {}", "Arguments".white().bold().underline());

    for (name, pm) in &params_with_values {
        let dirs = pm.direction_strings();
        let dir_str = if dirs.is_empty() {
            String::new()
        } else {
            format!(" {}", format!("[{}]", dirs.join(", ")).bright_black())
        };

        let _ = writeln!(out);
        let _ = writeln!(out, "  {}{dir_str}", name.cyan().bold());
        render_values_table(out, pm, const_lookup);
    }
}

/// Info section: requirements + linkage (shared fields).
fn render_info_section(out: &mut String, entry: Entry<'_>) {
    let meta = entry.as_metadata();
    let api_locations = meta
        .api_metadata()
        .map(bb_sparse::ApiMetadata::locations)
        .unwrap_or_default();

    let has_info = meta.min_client_str().is_some()
        || meta.min_server_str().is_some()
        || api_locations.len() > 1;

    if !has_info {
        return;
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  {}", "Info".white().bold().underline());

    if let Some(c) = meta.min_client_str() {
        let _ = writeln!(out, "  {} {c}", "client:".dimmed());
    }
    if let Some(s) = meta.min_server_str() {
        let _ = writeln!(out, "  {} {s}", "server:".dimmed());
    }
    if api_locations.len() > 1 {
        let _ = writeln!(
            out,
            "  {} {}",
            "also in:".dimmed(),
            api_locations.join(", ")
        );
    }
}

/// Driver section: IRQL + KMDF/UMDF + target-type + tech-root + include.
/// Only invoked when `entry.driver()` is `Some`. Rows with no source data
/// are skipped to avoid dead lines.
fn render_driver_section(out: &mut String, drv: &DriverMetadata) {
    let irql_line = drv.irql.as_ref().map(|c| match c.op.as_deref() {
        Some(op) => format!("{op} {}", c.level),
        None => c.level.clone(),
    });

    let rows: [(&str, Option<String>); 7] = [
        ("irql:", irql_line),
        ("kmdf:", drv.kmdf_ver_str().map(str::to_string)),
        ("umdf:", drv.umdf_ver_str().map(str::to_string)),
        ("target-type:", drv.target_type_str().map(str::to_string)),
        ("tech-root:", drv.tech_root_str().map(str::to_string)),
        ("include:", drv.include_header_str().map(str::to_string)),
        ("kind:", drv.construct_type_str().map(str::to_string)),
    ];

    if rows.iter().all(|(_, v)| v.is_none()) {
        return;
    }

    let _ = writeln!(out);
    let _ = writeln!(out, "  {}", "Driver".white().bold().underline());
    for (label, value) in rows {
        if let Some(v) = value {
            let _ = writeln!(out, "  {} {v}", label.dimmed());
        }
    }
}

/* ──────────────────────── C prototype rendering ────────────────────────── */

fn render_prototype(out: &mut String, f: &Function, entry: Option<Entry<'_>>) {
    let ret = f.get_return_type_name().green();
    let name = f.get_name().cyan().bold();

    let params = f.get_params();

    // Right-align location to terminal edge.
    let loc_raw = f
        .get_location()
        .map(std::string::ToString::to_string)
        .unwrap_or_default();

    if params.is_empty() {
        let prefix = format!("  {} {}(void)", f.get_return_type_name(), f.get_name());
        let pad = terminal_width().saturating_sub(prefix.len() + loc_raw.len());
        let _ = writeln!(out, "  {ret} {name}(void){:>pad$}{}", "", loc_raw.dimmed());
        return;
    }

    let prefix = format!("  {} {}(", f.get_return_type_name(), f.get_name());
    let pad = terminal_width().saturating_sub(prefix.len() + loc_raw.len());
    let _ = writeln!(out, "  {ret} {name}({:>pad$}{}", "", loc_raw.dimmed());

    let sal_width = params
        .iter()
        .map(|p| sal_for_param(p, entry).len())
        .max()
        .unwrap_or(0);
    let type_width = params
        .iter()
        .map(|p| p.get_type_name().len())
        .max()
        .unwrap_or(0);

    for (i, p) in params.iter().enumerate() {
        let is_last = i == params.len() - 1;
        let sal = sal_for_param(p, entry);
        let sal_styled = if sal.is_empty() {
            format!("{:>width$}", "", width = sal_width + 6)
        } else {
            format!("/* {sal:<sal_width$} */").dimmed().to_string()
        };
        let type_name = p.get_type_name().cyan();
        let param_name = p.get_name().map_or_else(
            || "<unnamed>".dimmed().to_string(),
            |n| n.white().bold().to_string(),
        );
        let comma = if is_last { "" } else { "," };
        let _ = writeln!(
            out,
            "    {sal_styled} {type_name:<type_width$} {param_name}{comma}",
        );
    }
    let _ = writeln!(out, "  );");
}

fn sal_for_param(p: &Param, entry: Option<Entry<'_>>) -> String {
    let Some(entry) = entry else {
        return String::new();
    };
    let Some(name) = p.get_name() else {
        return String::new();
    };
    let Some(pm) = entry.as_metadata().params().get(name) else {
        return String::new();
    };
    let dirs = pm.direction_strings();
    if dirs.is_empty() {
        return String::new();
    }
    dirs.join(", ")
}

/* ──────────────────── Values table rendering ───────────────────────────── */

fn render_values_table(
    out: &mut String,
    pm: &ParamMetadata,
    const_lookup: Option<&ConstantLookup>,
) {
    let mut entries: Vec<(String, String, String)> = pm
        .values
        .iter()
        .filter_map(|(name, sparse_val)| {
            if let Some(lookup) = const_lookup
                && let Some(info) = lookup.get(name.as_str())
            {
                let loc_str = info
                    .location
                    .as_ref()
                    .map(std::string::ToString::to_string)
                    .unwrap_or_default();
                return Some((name.clone(), format!("{:#X}", info.value), loc_str));
            }
            let val_str = match sparse_val.as_i64() {
                Some(v) => format!("{v:#X}"),
                None if sparse_val.is_null() => return None,
                None => sparse_val.to_string(),
            };
            Some((name.clone(), val_str, String::new()))
        })
        .collect();

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    if entries.is_empty() {
        return;
    }

    let mut table = Table::new();
    table
        .load_preset(presets::UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic);

    table.set_header(vec![
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Value")
            .add_attribute(Attribute::Bold)
            .set_alignment(CellAlignment::Right),
        Cell::new("Source").add_attribute(Attribute::Bold),
    ]);

    for (name, val, loc) in &entries {
        table.add_row(vec![
            Cell::new(name),
            Cell::new(val).set_alignment(CellAlignment::Right),
            Cell::new(loc),
        ]);
    }

    for line in table.to_string().lines() {
        let _ = writeln!(out, "    {line}");
    }
}

/* ──────────────────────── JSON serialization ────────────────────────────── */

/// Serialize the enriched param values (cross-ref'd with bb-consts) to JSON.
fn param_values_to_json(pm: &ParamMetadata, const_lookup: Option<&ConstantLookup>) -> Value {
    let mut obj = serde_json::Map::new();

    for (name, sparse_val) in &pm.values {
        // Cross-ref with bb-consts first.
        if let Some(lookup) = const_lookup
            && let Some(info) = lookup.get(name.as_str())
        {
            let loc_json = info
                .location
                .as_ref()
                .and_then(|l| serde_json::to_value(l).ok())
                .unwrap_or(Value::Null);
            obj.insert(
                name.clone(),
                json!({ "value": info.value, "source": loc_json }),
            );

            // No need to proceed.
            continue;
        }

        // If present, fall back on sparse default value.
        if !sparse_val.is_null() {
            let val = sparse_val.as_i64().map_or_else(
                || json!({ "value": sparse_val, "source": null }),
                |v| json!({ "value": v, "source": null }),
            );
            obj.insert(name.clone(), val);
        }
    }

    Value::Object(obj)
}

/// JSON shape for a driver entry's extra fields.
fn driver_to_json(drv: &DriverMetadata) -> Value {
    json!({
        "irql": drv.irql,
        "irql_raw": drv.irql_raw_str(),
        "kmdf_ver": drv.kmdf_ver_str(),
        "umdf_ver": drv.umdf_ver_str(),
        "target_type": drv.target_type_str(),
        "tech_root": drv.tech_root_str(),
        "include_header": drv.include_header_str(),
        "construct_type": drv.construct_type_str(),
    })
}

/// Serialize a single function to enriched JSON.
#[must_use]
pub fn function_to_enriched_json(
    f: &Function,
    mode: SdkMode,
    const_lookup: Option<&ConstantLookup>,
) -> Value {
    let ef = EnrichedFunction::new_ref(f, mode);
    let entry = ef.entry;

    // Start from the base serde JSON for each param, then enrich with
    // sparse metadata and reformatted ABI info.
    let params: Vec<Value> = f
        .get_params()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut pj = p.to_json();
            let obj = pj.as_object_mut().unwrap();

            // Replace raw abi_location with the enriched format.
            obj.remove("abi_location");
            obj.insert("index".into(), json!(i));
            obj.insert("abi".into(), p.get_abi_location().to_json());

            // Add sparse metadata (directions, known constant values).
            let pm = entry.and_then(|e| p.get_name().and_then(|n| e.as_metadata().params().get(n)));
            let dirs: Vec<String> = pm
                .map(bb_sparse::ParamMetadata::direction_strings)
                .unwrap_or_default();
            obj.insert("directions".into(), json!(dirs));
            obj.insert(
                "values".into(),
                pm.map_or_else(|| json!({}), |m| param_values_to_json(m, const_lookup)),
            );

            pj
        })
        .collect();

    // Build the function-level JSON from the base serde output, then enrich.
    let mut fj = f.to_json();
    let obj = fj.as_object_mut().unwrap();
    obj.insert("params".into(), json!(params));
    obj.insert("return_abi".into(), f.get_return_location().to_json());

    if let Some(entry) = entry {
        let meta = entry.as_metadata();
        let api = meta.api_metadata();
        obj.insert(
            "metadata".into(),
            json!({
                "source": match entry.source() {
                    bb_sparse::Source::Sdk => "sdk",
                    bb_sparse::Source::Driver => "driver",
                },
                "dll": meta.dll_display(),
                "lib": meta.lib_display(),
                "min_client": meta.min_client_str(),
                "min_server": meta.min_server_str(),
                "variants": api.map(bb_sparse::ApiMetadata::names).unwrap_or_default(),
                "locations": api.map(bb_sparse::ApiMetadata::locations).unwrap_or_default(),
            }),
        );

        if let Some(drv) = entry.driver() {
            obj.insert("driver".into(), driver_to_json(drv));
        }
    }

    fj
}

/// Serialize a slice of functions to enriched JSON array.
#[must_use]
pub fn functions_to_enriched_json(
    funcs: &[Function],
    mode: SdkMode,
    const_lookup: Option<&ConstantLookup>,
) -> Value {
    Value::Array(
        funcs
            .iter()
            .map(|f| function_to_enriched_json(f, mode, const_lookup))
            .collect(),
    )
}
