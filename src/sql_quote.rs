//! SQL identifier fragments for DataFusion-compatible emission.

/// OSI `Dataset::source` values may contain `.` (for example `api.people_daily`).
///
/// Unqualified `a.b` in SQL is parsed as schema `a` and table `b`. When parquet tables are
/// registered with [`datafusion_common::TableReference::bare`], the whole string is one table
/// name in the default schema, so dotted names must be emitted as a single quoted identifier.
pub(crate) fn dotted_osi_name_as_sql_table_expr(name: &str) -> String {
    if name.contains('.') {
        format!("\"{}\"", name.replace('"', "\"\""))
    } else {
        name.to_string()
    }
}
