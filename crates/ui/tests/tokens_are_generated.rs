//! Generates `src/tokens/generated.rs` from `docs/design/tokens.json` and fails
//! when the committed file differs (decision 0022). Set
//! `MASSORCA_UPDATE_TOKENS=1` to rewrite the file.

use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::PathBuf;

#[test]
fn tokens_are_generated() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let json_path = root.join("../../docs/design/tokens.json");
    let out_path = root.join("src/tokens/generated.rs");
    let json: Value = serde_json::from_str(&std::fs::read_to_string(&json_path).unwrap()).unwrap();
    let generated = generate(&json);
    let current = std::fs::read_to_string(&out_path).unwrap_or_default();
    if std::env::var_os("MASSORCA_UPDATE_TOKENS").is_some() {
        std::fs::write(&out_path, &generated).unwrap();
        return;
    }
    assert!(
        current.replace("\r\n", "\n") == generated,
        "src/tokens/generated.rs is out of date with docs/design/tokens.json; \
         run `MASSORCA_UPDATE_TOKENS=1 cargo test -p massorca-ui --test tokens_are_generated`"
    );
}

fn generate(json: &Value) -> String {
    let mut out = String::new();
    out.push_str(
        "// @generated from docs/design/tokens.json by tests/tokens_are_generated.rs.\n\
         // Do not edit by hand (decision 0022).\n\n\
         #![allow(clippy::excessive_precision, clippy::unreadable_literal)]\n\n\
         use super::{Color, Easing, FontFamily, Measure, ShadowToken, TextStyleToken};\n\
         use std::time::Duration;\n\n",
    );
    palette(json, &mut out);
    shadows(json, &mut out);
    type_styles(json, &mut out);
    for (group, kind) in [
        ("spacing", Kind::Length),
        ("radius", Kind::Length),
        ("size", Kind::Length),
        ("layout", Kind::Length),
        ("border", Kind::Length),
        ("duration", Kind::Duration),
        ("easing", Kind::Easing),
        ("zIndex", Kind::Int),
        ("contrast", Kind::Ratio),
    ] {
        scalars(json, group, kind, &mut out);
    }
    out
}

fn ident(s: &str) -> String {
    let s = s.replace(['-', '.'], "_");
    if s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("n{s}")
    } else if matches!(
        s.as_str(),
        "type" | "match" | "default" | "ref" | "box" | "move"
    ) {
        format!("r#{s}")
    } else {
        s
    }
}

fn upper(name: &str) -> String {
    name.replace(['-', '.'], "_").to_uppercase()
}

fn pascal(s: &str) -> String {
    s.split(['-', '_'])
        .map(|w| {
            let mut c = w.chars();
            c.next()
                .map(|f| f.to_ascii_uppercase().to_string() + c.as_str())
                .unwrap_or_default()
        })
        .collect()
}

fn color_literal(raw: &str) -> String {
    let raw = raw.trim();
    if let Some(hex) = raw.strip_prefix('#') {
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).unwrap();
        let a = if hex.len() == 8 {
            f32::from(byte(6)) / 255.0
        } else {
            1.0
        };
        assert!(hex.len() == 6 || hex.len() == 8, "bad colour {raw}");
        return format!(
            "Color::rgba8({}, {}, {}, {:?})",
            byte(0),
            byte(2),
            byte(4),
            round3(a)
        );
    }
    let inner = raw
        .strip_prefix("rgba(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or_else(|| panic!("bad colour {raw}"));
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    let a: f32 = parts[3].parse().unwrap();
    format!(
        "Color::rgba8({}, {}, {}, {:?})",
        parts[0],
        parts[1],
        parts[2],
        round3(a)
    )
}

fn round3(v: f32) -> f32 {
    (v * 1000.0).round() / 1000.0
}

/// Colour tokens: one struct per first name segment, the rest of the name as a
/// snake-case field (`status.success.fg` -> `palette.status.success_fg`).
fn palette(json: &Value, out: &mut String) {
    let tokens = json["color"]["tokens"].as_array().unwrap();
    let raw: BTreeMap<String, &Value> = tokens
        .iter()
        .map(|t| (t["name"].as_str().unwrap().to_owned(), &t["value"]))
        .collect();
    let resolve = |name: &str, theme: &str| -> String {
        let mut value = raw[name];
        for _ in 0..8 {
            let s = match value {
                Value::String(s) => s.as_str(),
                Value::Object(m) => m[theme].as_str().unwrap(),
                _ => panic!("bad value for {name}"),
            };
            if let Some(alias) = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
                value = raw
                    .get(alias)
                    .unwrap_or_else(|| panic!("{name}: unknown alias {alias}"));
            } else {
                return color_literal(s);
            }
        }
        panic!("{name}: alias loop");
    };

    let mut groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for t in tokens {
        let name = t["name"].as_str().unwrap();
        let (group, field) = match name.split_once('.') {
            Some((g, f)) => (g.to_owned(), ident(f)),
            None => ("root".to_owned(), ident(name)),
        };
        match groups.iter_mut().find(|(g, _)| *g == group) {
            Some((_, fields)) => fields.push((field, name.to_owned())),
            None => groups.push((group, vec![(field, name.to_owned())])),
        }
    }

    for (group, fields) in &groups {
        if group == "root" {
            continue;
        }
        let _ = writeln!(out, "/// Colour tokens named `{group}.*`.");
        let _ = writeln!(
            out,
            "#[derive(Clone, Copy, Debug, PartialEq)]\npub struct {}Colors {{",
            pascal(group)
        );
        for (field, name) in fields {
            let _ = writeln!(out, "    /// `{name}`\n    pub {field}: Color,");
        }
        out.push_str("}\n\n");
    }
    out.push_str("/// Every colour token of one theme.\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct Palette {\n");
    for (group, fields) in &groups {
        if group == "root" {
            for (field, name) in fields {
                let _ = writeln!(out, "    /// `{name}`\n    pub {field}: Color,");
            }
        } else {
            let _ = writeln!(out, "    pub {}: {}Colors,", ident(group), pascal(group));
        }
    }
    out.push_str("}\n\nimpl Palette {\n");
    for (konst, theme) in [("DARK", "dark"), ("LIGHT", "light")] {
        let _ = writeln!(out, "    pub const {konst}: Palette = Palette {{");
        for (group, fields) in &groups {
            if group == "root" {
                for (field, name) in fields {
                    let _ = writeln!(out, "        {field}: {},", resolve(name, theme));
                }
            } else {
                let _ = writeln!(out, "        {}: {}Colors {{", ident(group), pascal(group));
                for (field, name) in fields {
                    let _ = writeln!(out, "            {field}: {},", resolve(name, theme));
                }
                out.push_str("        },\n");
            }
        }
        out.push_str("    };\n");
    }
    out.push_str("\n    /// Looks a colour up by its token name, as theme files write it.\n");
    out.push_str(
        "    pub fn get(&self, name: &str) -> Option<Color> {\n        Some(match name {\n",
    );
    for (group, fields) in &groups {
        for (field, name) in fields {
            let path = if group == "root" {
                field.clone()
            } else {
                format!("{}.{field}", ident(group))
            };
            let _ = writeln!(out, "            {name:?} => self.{path},");
        }
    }
    out.push_str("            _ => return None,\n        })\n    }\n}\n\n");
    out.push_str("/// Every colour token name, in token-file order.\npub const PALETTE_NAMES: &[&str] = &[\n");
    for t in tokens {
        let _ = writeln!(out, "    {:?},", t["name"].as_str().unwrap());
    }
    out.push_str("];\n\n");
}

fn shadows(json: &Value, out: &mut String) {
    let tokens = json["shadow"]["tokens"].as_array().unwrap();
    out.push_str(
        "/// Shadow tokens of one theme (decision 0022; spike S3 maps blur one to one).\n",
    );
    out.push_str("#[derive(Clone, Copy, Debug, PartialEq)]\npub struct Shadows {\n");
    for t in tokens {
        let name = t["name"].as_str().unwrap();
        let _ = writeln!(
            out,
            "    /// `{name}`\n    pub {}: &'static [ShadowToken],",
            ident(name.trim_start_matches("shadow."))
        );
    }
    out.push_str("}\n\nimpl Shadows {\n");
    for (konst, theme) in [("DARK", "dark"), ("LIGHT", "light")] {
        let _ = writeln!(out, "    pub const {konst}: Shadows = Shadows {{");
        for t in tokens {
            let name = t["name"].as_str().unwrap();
            let value = match &t["value"] {
                Value::String(s) => s.clone(),
                v => v[theme].as_str().unwrap().to_owned(),
            };
            let _ = write!(
                out,
                "        {}: &[",
                ident(name.trim_start_matches("shadow."))
            );
            if value != "none" {
                for layer in split_layers(&value) {
                    out.push_str(&shadow_layer(&layer));
                    out.push_str(", ");
                }
            }
            out.push_str("],\n");
        }
        out.push_str("    };\n");
    }
    out.push_str("}\n\n");
}

fn split_layers(value: &str) -> Vec<String> {
    let mut layers = Vec::new();
    let (mut depth, mut current) = (0, String::new());
    for c in value.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                layers.push(current.trim().to_owned());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(c);
    }
    layers.push(current.trim().to_owned());
    layers
}

fn shadow_layer(layer: &str) -> String {
    let colour_start = layer.find(['#', 'r']).unwrap();
    let numbers: Vec<f32> = layer[..colour_start]
        .split_whitespace()
        .map(|n| n.trim_end_matches("px").parse().unwrap())
        .collect();
    assert_eq!(numbers.len(), 4, "shadow layer {layer}");
    format!(
        "ShadowToken {{ x: {:?}, y: {:?}, blur: {:?}, spread: {:?}, color: {} }}",
        numbers[0],
        numbers[1],
        numbers[2],
        numbers[3],
        color_literal(&layer[colour_start..])
    )
}

fn type_styles(json: &Value, out: &mut String) {
    for group in json["type"]["groups"].as_array().unwrap() {
        let family = group["family"].as_str().unwrap();
        let _ = writeln!(out, "// {}", group["name"].as_str().unwrap());
        for s in group["styles"].as_array().unwrap() {
            let name = s["name"].as_str().unwrap();
            let fam = s
                .get("family")
                .and_then(Value::as_str)
                .unwrap_or(if name.contains("mono") {
                    "mono"
                } else {
                    family
                });
            let px = |k: &str| {
                s[k].as_str()
                    .unwrap()
                    .trim_end_matches("px")
                    .parse::<f32>()
                    .unwrap()
            };
            let spacing = s
                .get("letterSpacing")
                .and_then(Value::as_str)
                .map_or(0.0, |v| v.trim_end_matches("em").parse::<f32>().unwrap());
            let _ = writeln!(
                out,
                "/// Type style `{name}`.\npub const TYPE_{}: TextStyleToken = TextStyleToken {{ family: FontFamily::{}, size: {:?}, line_height: {:?}, weight: {}, letter_spacing_em: {:?} }};",
                upper(name),
                if fam == "mono" { "Mono" } else { "Sans" },
                px("fontSize"),
                px("lineHeight"),
                s["fontWeight"].as_u64().unwrap(),
                spacing
            );
        }
    }
    out.push('\n');
}

#[derive(Clone, Copy)]
enum Kind {
    Length,
    Duration,
    Easing,
    Int,
    Ratio,
}

fn scalars(json: &Value, group: &str, kind: Kind, out: &mut String) {
    let _ = writeln!(out, "// {group}");
    for t in json[group]["tokens"].as_array().unwrap() {
        let name = t["name"].as_str().unwrap();
        let value = t["value"].as_str().unwrap();
        let konst = upper(name);
        let line = match kind {
            Kind::Length => {
                if let Some(px) = value.strip_suffix("px") {
                    format!("pub const {konst}: f32 = {:?};", px.parse::<f32>().unwrap())
                } else if let Some(p) = value.strip_suffix('%') {
                    format!(
                        "pub const {konst}: Measure = Measure::Percent({:?});",
                        p.parse::<f32>().unwrap()
                    )
                } else if let Some(ch) = value.strip_suffix("ch") {
                    format!(
                        "pub const {konst}: Measure = Measure::Ch({:?});",
                        ch.parse::<f32>().unwrap()
                    )
                } else {
                    panic!("{name}: unknown length {value}")
                }
            }
            Kind::Duration => {
                format!(
                    "pub const {konst}: Duration = Duration::from_millis({});",
                    value.trim_end_matches("ms")
                )
            }
            Kind::Easing => {
                if value == "linear" {
                    format!("pub const {konst}: Easing = Easing::Linear;")
                } else {
                    let inner = value
                        .trim_start_matches("cubic-bezier(")
                        .trim_end_matches(')');
                    let n: Vec<f32> = inner
                        .split(',')
                        .map(|v| v.trim().parse().unwrap())
                        .collect();
                    format!(
                        "pub const {konst}: Easing = Easing::CubicBezier({:?}, {:?}, {:?}, {:?});",
                        n[0], n[1], n[2], n[3]
                    )
                }
            }
            Kind::Int => format!("pub const {konst}: u16 = {value};"),
            Kind::Ratio => format!(
                "pub const {konst}: f32 = {:?};",
                value.parse::<f32>().unwrap()
            ),
        };
        let _ = writeln!(out, "/// `{name}`\n{line}");
    }
    out.push('\n');
}
