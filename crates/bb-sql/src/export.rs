//! SQLite export utilities.
//!
//! Exports serde-serialized JSON objects into SQLite tables. Top-level scalar
//! fields become columns with native SQLite types; nested objects and arrays
//! are stored as JSON text. This ensures the SQLite export has the same level
//! of detail as the JSON output.

use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::Connection;
use serde_json::Value;

/* ────────────────────────── Column inference ──────────────────────────── */

/// Infer column definitions from the union of all keys across all rows.
///
/// Scans every row to build a complete column set. This is necessary because
/// serde `skip_serializing_if` can omit keys from individual rows (e.g.
/// `expression: Option<String>` is absent when `None`).
fn infer_columns(rows: &[Value]) -> Vec<(String, &'static str)> {
    let mut columns: Vec<(String, &'static str)> = Vec::new();

    for row in rows {
        let Some(obj) = row.as_object() else {
            continue;
        };
        for (key, val) in obj {
            if columns.iter().any(|(k, _)| k == key) {
                continue;
            }
            let sql_type = match val {
                Value::Bool(_) => "BOOLEAN",
                Value::Number(n) if n.is_i64() || n.is_u64() => "INTEGER",
                Value::Number(_) => "REAL",
                Value::String(_) => "TEXT",
                Value::Null => "TEXT",
                Value::Array(_) | Value::Object(_) => "TEXT",
            };
            columns.push((key.clone(), sql_type));
        }
    }

    columns
}

/// Convert a JSON value to a rusqlite parameter.
fn json_to_param(val: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match val {
        Value::Bool(b) => Box::new(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(u) = n.as_u64() {
                Box::new(u as i64)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(rusqlite::types::Null)
            }
        }
        Value::String(s) => Box::new(s.clone()),
        Value::Null => Box::new(rusqlite::types::Null),
        // Nested structures are stored as JSON text.
        val @ (Value::Array(_) | Value::Object(_)) => Box::new(val.to_string()),
    }
}

/* ─────────────────────────── Public API ───────────────────────────────── */

/// Export JSON objects to a SQLite table.
///
/// Each [`Value`] in `rows` must be a JSON object. The table schema is
/// inferred from the union of all keys across all rows. Top-level scalar
/// fields map to native SQLite types; nested objects/arrays are stored as
/// JSON text.
///
/// When `rows` is empty, an empty table with no columns is still created
/// so the output file always exists.
///
/// This produces the same level of detail as `--json` output.
pub fn export_json_to_sqlite(path: &Path, table: &str, rows: &[Value]) -> Result<()> {
    let columns = infer_columns(rows);

    let mut conn = Connection::open(path)
        .with_context(|| format!("failed to open SQLite database: {}", path.display()))?;

    // Drop + create table.
    conn.execute_batch(&format!("DROP TABLE IF EXISTS [{table}]"))
        .context("failed to drop existing table")?;

    if columns.is_empty() {
        // Create an empty sentinel table so the file always exists.
        conn.execute_batch(&format!("CREATE TABLE [{table}] (_empty INTEGER)"))
            .context("failed to create empty table")?;
        return Ok(());
    }

    let col_defs: Vec<String> = columns
        .iter()
        .map(|(name, sql_type)| format!("[{name}] {sql_type}"))
        .collect();
    let create_sql = format!("CREATE TABLE [{table}] ({})", col_defs.join(", "));
    conn.execute_batch(&create_sql)
        .context("failed to create table")?;

    if rows.is_empty() {
        return Ok(());
    }

    // Insert rows in a transaction.
    let placeholders: Vec<String> = (1..=columns.len()).map(|i| format!("?{i}")).collect();
    let insert_sql = format!("INSERT INTO [{table}] VALUES ({})", placeholders.join(", "));

    let tx = conn.transaction().context("failed to begin transaction")?;
    {
        let mut stmt = tx
            .prepare(&insert_sql)
            .context("failed to prepare insert")?;
        for row in rows {
            let obj = row.as_object().context("row is not a JSON object")?;
            let params: Vec<Box<dyn rusqlite::types::ToSql>> = columns
                .iter()
                .map(|(key, _)| {
                    let val = obj.get(key).unwrap_or(&Value::Null);
                    json_to_param(val)
                })
                .collect();
            let param_refs: Vec<&dyn rusqlite::types::ToSql> =
                params.iter().map(|p| p.as_ref()).collect();
            stmt.execute(param_refs.as_slice())
                .context("failed to insert row")?;
        }
    }
    tx.commit().context("failed to commit transaction")?;

    Ok(())
}

/* ────────────────────────────── Tests ─────────────────────────────────── */

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};

    use serde_json::json;

    use super::*;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_db() -> std::path::PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir();
        dir.join(format!("bb_sql_test_{}_{n}.db", std::process::id()))
    }

    #[test]
    fn export_basic_rows() {
        let path = temp_db();
        let _ = fs::remove_file(&path);

        let rows = vec![
            json!({"name": "foo", "value": 42, "active": true}),
            json!({"name": "bar", "value": 7, "active": false}),
        ];

        export_json_to_sqlite(&path, "test", &rows).unwrap();

        let conn = Connection::open(&path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM test", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 2);

        let name: String = conn
            .query_row("SELECT name FROM test WHERE value = 42", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "foo");

        fs::remove_file(&path).ok();
    }

    #[test]
    fn export_empty_rows_creates_table() {
        let path = temp_db();
        let _ = fs::remove_file(&path);

        export_json_to_sqlite(&path, "empty", &[]).unwrap();

        let conn = Connection::open(&path).unwrap();
        // Table should exist.
        let exists: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='empty'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(exists, 1);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn schema_union_across_rows() {
        let path = temp_db();
        let _ = fs::remove_file(&path);

        // First row has no "extra" field, second row does.
        // Both should be captured.
        let rows = vec![
            json!({"name": "a", "value": 1}),
            json!({"name": "b", "value": 2, "extra": "hello"}),
        ];

        export_json_to_sqlite(&path, "test", &rows).unwrap();

        let conn = Connection::open(&path).unwrap();

        // Check that the "extra" column exists.
        let mut stmt = conn.prepare("PRAGMA table_info(test)").unwrap();
        let col_names: HashSet<String> = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();

        assert!(col_names.contains("name"));
        assert!(col_names.contains("value"));
        assert!(
            col_names.contains("extra"),
            "extra column from row 2 should be in schema"
        );

        // First row should have NULL for extra.
        let extra: Option<String> = conn
            .query_row("SELECT extra FROM test WHERE name = 'a'", [], |r| r.get(0))
            .unwrap();
        assert!(extra.is_none(), "row 1 should have NULL for extra");

        // Second row should have the value.
        let extra: Option<String> = conn
            .query_row("SELECT extra FROM test WHERE name = 'b'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(extra.as_deref(), Some("hello"));

        fs::remove_file(&path).ok();
    }

    #[test]
    fn nested_json_stored_as_text() {
        let path = temp_db();
        let _ = fs::remove_file(&path);

        let rows = vec![json!({"name": "x", "nested": {"a": 1, "b": 2}})];

        export_json_to_sqlite(&path, "test", &rows).unwrap();

        let conn = Connection::open(&path).unwrap();
        let nested: String = conn
            .query_row("SELECT nested FROM test", [], |r| r.get(0))
            .unwrap();
        // Should be valid JSON text.
        let parsed: serde_json::Value = serde_json::from_str(&nested).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 2);

        fs::remove_file(&path).ok();
    }

    #[test]
    fn multiple_tables_same_file() {
        let path = temp_db();
        let _ = fs::remove_file(&path);

        let rows1 = vec![json!({"x": 1})];
        let rows2 = vec![json!({"y": 2})];

        export_json_to_sqlite(&path, "table1", &rows1).unwrap();
        export_json_to_sqlite(&path, "table2", &rows2).unwrap();

        let conn = Connection::open(&path).unwrap();
        let t1: i64 = conn
            .query_row("SELECT x FROM table1", [], |r| r.get(0))
            .unwrap();
        let t2: i64 = conn
            .query_row("SELECT y FROM table2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(t1, 1);
        assert_eq!(t2, 2);

        fs::remove_file(&path).ok();
    }
}
