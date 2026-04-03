//! Enriched function types that join bb-clang's parsed functions
//! with bb-sparse's Windows API metadata and bb-consts cross-references.
//!
//! This module lives in bb-funcs (not bb-clang) so that bb-clang stays
//! reusable without Windows API metadata dependencies. Both CLI and TUI
//! consume these types for rendering.

use std::collections::HashMap;
use std::fmt::Write;

use bb_arch::display::{param_abi_to_json, return_abi_to_json};
use bb_clang::display::{
    format_abi_param, format_arch, format_callconv, format_return_location, format_tags,
};
use bb_clang::{Constant, Function, Param, SourceLocation};
use bb_consts_lib::{ConstFilter, collect_constants, collect_enums};
use bb_sparse::{FuncMetadata, ParamMetadata};
use colored::Colorize;
use comfy_table::{Attribute, Cell, CellAlignment, ContentArrangement, Table, presets};
use serde_json::{Value, json};

/* ────────────────────────────────── Types ───────────────────────────────── */

/// A [`Function`] enriched with optional sparse metadata.
pub struct EnrichedFunction<'a> {
    pub function: &'a Function<'a>,
    pub metadata: Option<&'static FuncMetadata>,
}

impl<'a> EnrichedFunction<'a> {
    #[must_use]
    pub fn new_ref(function: &'a Function<'a>) -> Self {
        let metadata = bb_sparse::lookup(function.get_name());
        Self { function, metadata }
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

/* ─────────────────────── Full enriched detail view ─────────────────────── */

/// Render the full enriched detail view for a function.
#[must_use]
pub fn render_enriched_detail(f: &Function, const_lookup: Option<&ConstantLookup>) -> String {
    let ef = EnrichedFunction::new_ref(f);
    let meta = ef.metadata;
    let mut out = String::new();

    render_prototype(&mut out, f, meta);
    render_header_tags(&mut out, f, meta);
    render_abi_section(&mut out, f);
    if let Some(meta) = meta {
        render_arguments_section(&mut out, f, meta, const_lookup);
        render_info_section(&mut out, meta);
    }

    out
}

/* ───────────────────────── Section renderers ────────────────────────────── */

/// Tags line + variants.
fn render_header_tags(out: &mut String, f: &Function, meta: Option<&FuncMetadata>) {
    let mut tags = format_tags(f);
    if let Some(meta) = meta {
        if let Some(dll) = meta.dll_display() {
            let lib = meta.lib_display().unwrap_or_else(|| "?".into());
            tags.push(format!("{dll} ({lib})"));
        }
    }
    let _ = writeln!(out, "  {}", tags.join("  ·  ").bright_black());

    if let Some(meta) = meta {
        if let Some(ref api) = meta.metadata {
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
    meta: &FuncMetadata,
    const_lookup: Option<&ConstantLookup>,
) {
    let params_with_values: Vec<_> = f
        .get_params()
        .iter()
        .filter_map(|p| {
            let name = p.get_name()?;
            let pm = meta.params.get(name)?;
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

/// Info section: requirements + linkage.
fn render_info_section(out: &mut String, meta: &FuncMetadata) {
    let has_info = meta.min_client_str().is_some()
        || meta.min_server_str().is_some()
        || meta
            .metadata
            .as_ref()
            .is_some_and(|a| a.locations().len() > 1);

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
    if let Some(ref api) = meta.metadata {
        let locations = api.locations();
        if locations.len() > 1 {
            let _ = writeln!(out, "  {} {}", "also in:".dimmed(), locations.join(", "));
        }
    }
}

/* ──────────────────────── C prototype rendering ────────────────────────── */

fn render_prototype(out: &mut String, f: &Function, meta: Option<&FuncMetadata>) {
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
        let pad = bb_cli::terminal_width().saturating_sub(prefix.len() + loc_raw.len());
        let _ = writeln!(out, "  {ret} {name}(void){:>pad$}{}", "", loc_raw.dimmed());
        return;
    }

    let prefix = format!("  {} {}(", f.get_return_type_name(), f.get_name());
    let pad = bb_cli::terminal_width().saturating_sub(prefix.len() + loc_raw.len());
    let _ = writeln!(out, "  {ret} {name}({:>pad$}{}", "", loc_raw.dimmed());

    let sal_width = params
        .iter()
        .map(|p| sal_for_param(p, meta).len())
        .max()
        .unwrap_or(0);
    let type_width = params
        .iter()
        .map(|p| p.get_type_name().len())
        .max()
        .unwrap_or(0);

    for (i, p) in params.iter().enumerate() {
        let is_last = i == params.len() - 1;
        let sal = sal_for_param(p, meta);
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

fn sal_for_param(p: &Param, meta: Option<&FuncMetadata>) -> String {
    let Some(meta) = meta else {
        return String::new();
    };
    let Some(name) = p.get_name() else {
        return String::new();
    };
    let Some(pm) = meta.params.get(name) else {
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
            if let Some(lookup) = const_lookup {
                if let Some(info) = lookup.get(name.as_str()) {
                    let loc_str = info
                        .location
                        .as_ref()
                        .map(std::string::ToString::to_string)
                        .unwrap_or_default();
                    return Some((name.clone(), format!("{:#X}", info.value), loc_str));
                }
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
        if let Some(lookup) = const_lookup {
            if let Some(info) = lookup.get(name.as_str()) {
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

/// Serialize a single function to enriched JSON.
#[must_use]
pub fn function_to_enriched_json(f: &Function, const_lookup: Option<&ConstantLookup>) -> Value {
    let ef = EnrichedFunction::new_ref(f);
    let meta = ef.metadata;

    let params: Vec<Value> = f
        .get_params()
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let pm = meta.and_then(|m| p.get_name().and_then(|n| m.params.get(n)));

            let dirs: Vec<String> = pm
                .map(bb_sparse::ParamMetadata::direction_strings)
                .unwrap_or_default();

            let values = pm.map_or_else(|| json!({}), |m| param_values_to_json(m, const_lookup));

            json!({
                "index": i,
                "name": p.get_name(),
                "type": p.get_type_name(),
                "abi": param_abi_to_json(p.get_abi_location()),
                "directions": dirs,
                "values": values,
            })
        })
        .collect();

    let metadata_json = meta.map(|m| {
        let api = m.metadata.as_ref();
        json!({
            "dll": m.dll_display(),
            "lib": m.lib_display(),
            "min_client": m.min_client_str(),
            "min_server": m.min_server_str(),
            "variants": api.map(bb_sparse::ApiMetadata::names).unwrap_or_default(),
            "locations": api.map(bb_sparse::ApiMetadata::locations).unwrap_or_default(),
        })
    });

    json!({
        "name": f.get_name(),
        "return_type": f.get_return_type_name(),
        "arch": format_arch(f.get_arch()),
        "calling_convention": format_callconv(f.get_calling_convention()),
        // is_dllimport in SDK headers = the function is exported from a DLL.
        "is_exported": f.is_dllimport(),
        "has_body": f.has_body(),
        "location": f.get_location().and_then(|l| serde_json::to_value(l).ok()),
        "params": params,
        "return_abi": return_abi_to_json(f.get_return_location()),
        "metadata": metadata_json,
    })
}

/// Serialize a slice of functions to enriched JSON array.
#[must_use]
pub fn functions_to_enriched_json(
    funcs: &[Function],
    const_lookup: Option<&ConstantLookup>,
) -> Value {
    Value::Array(
        funcs
            .iter()
            .map(|f| function_to_enriched_json(f, const_lookup))
            .collect(),
    )
}
