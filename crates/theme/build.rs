//! Generates a tunable reader for every `f32` metric in `src/metrics.rs`.
//!
//! Each `pub const NAME: f32` inside `impl Theme` gets `Theme::name()`, which
//! returns the constant unless the component catalogue is tuning it (see
//! `src/tune.rs`). A metric written in terms of others keeps its formula, with
//! each `Self::OTHER` read through `Self::other()`, so tuning a base size moves
//! everything derived from it.

use std::{env, fs, path::Path};

struct Metric {
    name: String,
    expr: String,
    doc: String,
    group: String,
}

fn main() {
    println!("cargo:rerun-if-changed=src/metrics.rs");
    let source = fs::read_to_string("src/metrics.rs").expect("read src/metrics.rs");
    let metrics = parse(&source);
    let names: Vec<&str> = metrics.iter().map(|metric| metric.name.as_str()).collect();

    let mut readers = String::from("impl Theme {\n");
    let mut table = String::from("pub(crate) static METRICS: &[Metric] = &[\n");
    for metric in &metrics {
        let reader = metric.name.to_lowercase();
        let derived = metric.expr.contains("Self::");
        let formula = if derived {
            rewrite(&metric.expr, &names)
        } else {
            format!("Self::{}", metric.name)
        };
        readers.push_str(&format!(
            "    /// [`Theme::{name}`], through the catalogue's tuning table.\n    \
             #[inline]\n    pub fn {reader}() -> f32 {{\n        \
             crate::tune::read(\"{name}\", Self::{name}, || {formula})\n    }}\n",
            name = metric.name,
        ));
        table.push_str(&format!(
            "    Metric {{ name: \"{name}\", group: {group:?}, doc: {doc:?}, default: Theme::{name}, \
             derived: {derived}, read: Theme::{reader} }},\n",
            name = metric.name,
            group = metric.group,
            doc = metric.doc,
        ));
    }
    readers.push_str("}\n");
    table.push_str("];\n");

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("tuned.rs");
    fs::write(out, readers + &table).expect("write tuned.rs");
}

/// The `f32` constants of the `impl Theme` block, with the doc comment and
/// section heading above each.
fn parse(source: &str) -> Vec<Metric> {
    let mut metrics = Vec::new();
    let mut doc = Vec::<String>::new();
    let mut group = String::new();
    let mut pending: Option<(String, String)> = None;
    for line in source.lines() {
        let trimmed = line.trim();
        if line == "}" && pending.is_none() {
            break; // the end of `impl Theme`
        }
        if let Some((name, mut expr)) = pending.take() {
            expr.push(' ');
            expr.push_str(trimmed);
            if expr.ends_with(';') {
                metrics.push(finish(name, expr, &mut doc, &group));
            } else {
                pending = Some((name, expr));
            }
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("// ----") {
            group = heading
                .trim_matches(|c: char| c == '-' || c.is_whitespace())
                .to_string();
            doc.clear();
        } else if let Some(text) = trimmed.strip_prefix("///") {
            doc.push(text.trim().to_string());
        } else if let Some(rest) = trimmed.strip_prefix("pub const ") {
            let Some((name, rest)) = rest.split_once(": f32 =") else {
                doc.clear();
                continue;
            };
            let expr = rest.trim().to_string();
            if expr.ends_with(';') {
                metrics.push(finish(name.to_string(), expr, &mut doc, &group));
            } else {
                pending = Some((name.to_string(), expr));
            }
        } else if trimmed.is_empty() || !trimmed.starts_with("//") {
            doc.clear();
        }
    }
    metrics
}

fn finish(name: String, expr: String, doc: &mut Vec<String>, group: &str) -> Metric {
    let joined = doc.join(" ");
    let summary = match joined.find(". ") {
        Some(end) => &joined[..=end],
        None => joined.as_str(),
    };
    let metric = Metric {
        name,
        expr: expr.trim_end_matches(';').trim().to_string(),
        doc: summary.trim().to_string(),
        group: group.to_string(),
    };
    doc.clear();
    metric
}

/// `Self::NAME` becomes `Self::name()` for every tunable `NAME`.
fn rewrite(expr: &str, names: &[&str]) -> String {
    let mut out = String::new();
    let mut rest = expr;
    while let Some(at) = rest.find("Self::") {
        out.push_str(&rest[..at]);
        let after = &rest[at + "Self::".len()..];
        let end = after
            .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
            .unwrap_or(after.len());
        let name = &after[..end];
        if names.contains(&name) {
            out.push_str(&format!("Self::{}()", name.to_lowercase()));
        } else {
            out.push_str(&format!("Self::{name}"));
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}
