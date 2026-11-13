//! A deliberately bounded SPICE dialect. The first physical line is the title.
mod expression;
use spice_core::*;
use std::{
    collections::{BTreeMap, HashMap},
    fmt,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub line: usize,
}
#[derive(Debug)]
pub struct ParseError {
    pub span: SourceSpan,
    pub message: String,
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: {}",
            self.span.file.display(),
            self.span.line,
            self.message
        )
    }
}
impl std::error::Error for ParseError {}
type PResult<T> = std::result::Result<T, ParseError>;
#[derive(Clone, Debug)]
struct Line {
    tokens: Vec<String>,
    span: SourceSpan,
}
impl Line {
    fn error(&self, message: impl ToString) -> ParseError {
        ParseError {
            span: self.span.clone(),
            message: message.to_string(),
        }
    }
}
#[derive(Clone, Debug)]
pub enum Analysis {
    OperatingPoint,
    Dc {
        source: String,
        start: f64,
        stop: f64,
        step: f64,
    },
    Ac(FrequencyGrid),
    Transient {
        options: TransientOptions,
        start: f64,
    },
}
impl Analysis {
    pub fn name(&self) -> &'static str {
        match self {
            Self::OperatingPoint => "op",
            Self::Dc { .. } => "dc",
            Self::Ac(_) => "ac",
            Self::Transient { .. } => "tran",
        }
    }
}
#[derive(Debug)]
pub struct ParsedNetlist {
    pub circuit: Circuit,
    pub analyses: Vec<Analysis>,
    pub solver_options: SolverOptions,
    pub source_map: BTreeMap<String, SourceSpan>,
}
#[derive(Clone)]
struct Subcircuit {
    pins: Vec<String>,
    defaults: HashMap<String, String>,
    lines: Vec<Line>,
}
fn tokenize(text: &str) -> std::result::Result<Vec<String>, String> {
    let mut out = Vec::new();
    let chars: Vec<_> = text.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        if chars[k].is_whitespace() || chars[k] == ',' {
            k += 1;
            continue;
        }
        if chars[k] == '$' || chars[k] == ';' {
            break;
        }
        let start = k;
        if chars[k] == '{' {
            let mut depth = 1;
            k += 1;
            while k < chars.len() && depth > 0 {
                if chars[k] == '{' {
                    depth += 1;
                }
                if chars[k] == '}' {
                    depth -= 1;
                }
                k += 1;
            }
            if depth != 0 {
                return Err("unclosed parameter expression".into());
            }
        } else if chars[k] == '\'' || chars[k] == '"' {
            let quote = chars[k];
            k += 1;
            let begin = k;
            while k < chars.len() && chars[k] != quote {
                k += 1;
            }
            if k == chars.len() {
                return Err("unclosed quoted expression".into());
            }
            out.push(format!(
                "{{{}}}",
                chars[begin..k].iter().collect::<String>()
            ));
            k += 1;
            continue;
        } else if "()=".contains(chars[k]) {
            k += 1;
        } else {
            while k < chars.len() && !chars[k].is_whitespace() && !"()=,;$".contains(chars[k]) {
                k += 1;
            }
        }
        out.push(
            chars[start..k]
                .iter()
                .collect::<String>()
                .to_ascii_lowercase(),
        );
    }
    Ok(out)
}
fn logical_lines(text: &str, path: &Path, title: bool) -> PResult<Vec<Line>> {
    let mut lines: Vec<Line> = Vec::new();
    for (n, text) in text.lines().enumerate() {
        if title && n == 0 {
            continue;
        }
        let text = text.trim();
        if text.is_empty() || text.starts_with('*') {
            continue;
        }
        let span = SourceSpan {
            file: path.into(),
            line: n + 1,
        };
        let continuation = text.starts_with('+');
        let tokens = tokenize(if continuation { &text[1..] } else { text }).map_err(|message| {
            ParseError {
                span: span.clone(),
                message,
            }
        })?;
        if continuation {
            let previous = lines.last_mut().ok_or_else(|| ParseError {
                span: span.clone(),
                message: "continuation without preceding statement".into(),
            })?;
            previous.tokens.extend(tokens);
        } else if !tokens.is_empty() {
            lines.push(Line { tokens, span });
        }
    }
    Ok(lines)
}
fn load_file(path: &Path, title: bool, stack: &mut Vec<PathBuf>) -> PResult<Vec<Line>> {
    let error = |message: String| ParseError {
        span: SourceSpan {
            file: path.into(),
            line: 1,
        },
        message,
    };
    let path = path.canonicalize().map_err(|e| error(e.to_string()))?;
    if stack.contains(&path) {
        return Err(error("include cycle".into()));
    }
    if stack.len() >= 64 {
        return Err(error("include nesting limit".into()));
    }
    stack.push(path.clone());
    let text = std::fs::read_to_string(&path).map_err(|e| error(e.to_string()))?;
    let lines = logical_lines(&text, &path, title)?;
    let mut expanded = Vec::new();
    for line in lines {
        if line.tokens[0] == ".include" || line.tokens[0] == ".inc" {
            if line.tokens.len() != 2 {
                return Err(line.error(".include needs one path"));
            }
            // Preserve path case: tokenizer lowercases normal SPICE identifiers.
            let original = text.lines().nth(line.span.line - 1).unwrap_or("");
            let path_text = original
                .trim()
                .split_once(char::is_whitespace)
                .map(|(_, s)| s.trim())
                .unwrap_or("")
                .trim_matches(['\'', '"']);
            expanded.extend(load_file(
                &path.parent().unwrap_or(Path::new(".")).join(path_text),
                false,
                stack,
            )?);
        } else {
            expanded.push(line);
        }
    }
    stack.pop();
    Ok(expanded)
}
pub fn parse_file(path: impl AsRef<Path>) -> PResult<ParsedNetlist> {
    elaborate(load_file(path.as_ref(), true, &mut vec![])?)
}
pub fn parse_str(text: &str) -> PResult<ParsedNetlist> {
    elaborate(logical_lines(text, Path::new("<input>"), true)?)
}
fn assignments(line: &Line, t: &[String]) -> PResult<HashMap<String, String>> {
    let t = if t.first().is_some_and(|s| s == "(") {
        if t.last().is_none_or(|s| s != ")") {
            return Err(line.error("unclosed parameter list"));
        }
        &t[1..t.len() - 1]
    } else {
        t
    };
    if t.iter().any(|s| s == "(" || s == ")") {
        return Err(line.error("put function expressions in braces"));
    }
    let mut out = HashMap::new();
    let mut k = 0;
    while k < t.len() {
        if t[k] == "(" || t[k] == ")" || t[k] == "params:" {
            k += 1;
            continue;
        }
        if k + 2 >= t.len() || t[k + 1] != "=" {
            return Err(line.error(format!("expected name=value near {}", t[k])));
        }
        let name = t[k].clone();
        k += 2;
        let start = k;
        while k < t.len() && t[k] != ")" {
            if k > start && t.get(k + 1).is_some_and(|s| s == "=") {
                break;
            }
            k += 1;
        }
        if k == start {
            return Err(line.error("missing parameter value"));
        }
        if out.insert(name.clone(), t[start..k].join(" ")).is_some() {
            return Err(line.error(format!("duplicate parameter {name}")));
        }
    }
    Ok(out)
}
fn collect_params(
    lines: &[Line],
    mut defaults: HashMap<String, String>,
) -> PResult<HashMap<String, String>> {
    for line in lines.iter().filter(|l| l.tokens[0] == ".param") {
        for (name, value) in assignments(line, &line.tokens[1..])? {
            if defaults.insert(name.clone(), value).is_some() {
                return Err(line.error(format!("duplicate parameter {name}")));
            }
        }
    }
    Ok(defaults)
}
fn value(line: &Line, text: &str, env: &HashMap<String, f64>) -> PResult<f64> {
    expression::evaluate(text, env).map_err(|e| line.error(e))
}
fn model(line: &Line, env: &HashMap<String, f64>) -> PResult<(String, Model)> {
    let t = &line.tokens;
    if t.len() < 3 {
        return Err(line.error(".model requires name and type"));
    }
    let kind = match t[2].as_str() {
        "d" => ModelKind::Diode,
        "npn" => ModelKind::Npn,
        "pnp" => ModelKind::Pnp,
        "nmos" => ModelKind::Nmos,
        "pmos" => ModelKind::Pmos,
        s => return Err(line.error(format!("unsupported model type {s}"))),
    };
    let params = assignments(line, &t[3..])?;
    let mut model = Model::new(kind);
    for (name, expr) in params {
        let v = value(line, &expr, env)?;
        if name == "level" {
            if v != 1.0 {
                return Err(line.error("only model LEVEL=1 is supported"));
            }
        } else {
            model.set(&name, v).map_err(|e| line.error(e))?;
        }
    }
    Ok((t[1].clone(), model))
}
fn elaborate(lines: Vec<Line>) -> PResult<ParsedNetlist> {
    let fallback = Line {
        tokens: vec![],
        span: SourceSpan {
            file: "<input>".into(),
            line: 1,
        },
    };
    let mut root = Vec::new();
    let mut subs = HashMap::new();
    let mut k = 0;
    let mut ended = false;
    while k < lines.len() {
        let line = &lines[k];
        let t = &line.tokens;
        match t[0].as_str() {
            ".end" => {
                ended = true;
                k += 1;
                if k < lines.len() {
                    return Err(lines[k].error("content after .end"));
                }
                break;
            }
            ".subckt" => {
                if t.len() < 2 {
                    return Err(line.error("missing subcircuit name"));
                }
                let mut endpins = 2;
                while endpins < t.len()
                    && t[endpins] != "params:"
                    && t.get(endpins + 1).is_none_or(|s| s != "=")
                {
                    endpins += 1;
                }
                let pins = t[2..endpins].to_vec();
                if pins.iter().collect::<std::collections::HashSet<_>>().len() != pins.len() {
                    return Err(line.error("duplicate subcircuit pin"));
                }
                let defaults = assignments(line, &t[endpins..])?;
                let begin = k + 1;
                k += 1;
                while k < lines.len() && lines[k].tokens[0] != ".ends" {
                    if lines[k].tokens[0] == ".subckt" {
                        return Err(lines[k].error("nested subcircuit definitions are unsupported"));
                    }
                    k += 1;
                }
                if k == lines.len() {
                    return Err(line.error("missing .ends"));
                }
                if lines[k].tokens.len() > 2
                    || (lines[k].tokens.len() == 2 && lines[k].tokens[1] != t[1])
                {
                    return Err(lines[k].error("mismatched .ends"));
                }
                if subs
                    .insert(
                        t[1].clone(),
                        Subcircuit {
                            pins,
                            defaults,
                            lines: lines[begin..k].to_vec(),
                        },
                    )
                    .is_some()
                {
                    return Err(line.error("duplicate subcircuit"));
                }
            }
            ".ends" => return Err(line.error("unexpected .ends")),
            _ => root.push(line.clone()),
        }
        k += 1;
    }
    if !ended {
        return Err(root.last().unwrap_or(&fallback).error("missing .end"));
    }
    let params = collect_params(&root, HashMap::new())?;
    let env = expression::resolve(&HashMap::new(), &params, &HashMap::new())
        .map_err(|e| root.first().unwrap_or(&fallback).error(e))?;
    let mut builder = CircuitBuilder::new();
    let mut source_map = BTreeMap::new();
    let mut solver_options = SolverOptions::default();
    let mut analyses = Vec::new();
    let mut method = IntegrationMethod::Trapezoidal;
    for line in &root {
        let t = &line.tokens;
        match t[0].as_str() {
            ".temp" => {
                if t.len() != 2 {
                    return Err(line.error(".temp supports one static temperature"));
                }
                builder
                    .temperature(value(line, &t[1], &env)?)
                    .map_err(|e| line.error(e))?;
            }
            ".options" | ".option" => {
                for (name, expr) in assignments(line, &t[1..])? {
                    if name == "method" {
                        method = match expr.as_str() {
                            "trap" => IntegrationMethod::Trapezoidal,
                            "gear" => IntegrationMethod::Bdf2,
                            "be" => IntegrationMethod::BackwardEuler,
                            _ => return Err(line.error("method must be trap, gear or be")),
                        };
                        continue;
                    }
                    let v = value(line, &expr, &env)?;
                    match name.as_str() {
                        "reltol" => solver_options.reltol = v,
                        "vntol" => solver_options.vntol = v,
                        "abstol" => solver_options.abstol = v,
                        "itl1" => {
                            if v < 1.0 || v.fract() != 0.0 {
                                return Err(line.error("itl1 must be positive integer"));
                            }
                            solver_options.max_iterations = v as usize;
                        }
                        "maxord" => {
                            if v != 2.0 {
                                return Err(line.error("only maxord=2 is supported"));
                            }
                        }
                        _ => return Err(line.error(format!("unsupported option {name}"))),
                    }
                }
            }
            _ => {}
        }
    }
    for line in &root {
        let t = &line.tokens;
        match t[0].as_str() {
            ".op" => {
                if t.len() != 1 {
                    return Err(line.error("unexpected .op arguments"));
                }
                analyses.push(Analysis::OperatingPoint);
            }
            ".dc" => {
                if t.len() != 5 {
                    return Err(line.error(".dc source start stop step"));
                }
                analyses.push(Analysis::Dc {
                    source: t[1].clone(),
                    start: value(line, &t[2], &env)?,
                    stop: value(line, &t[3], &env)?,
                    step: value(line, &t[4], &env)?,
                });
            }
            ".ac" => {
                if t.len() != 5 {
                    return Err(line.error(".ac lin|dec|oct count start stop"));
                }
                let count = value(line, &t[2], &env)?;
                if count < 1.0 || count.fract() != 0.0 {
                    return Err(line.error("AC count must be positive integer"));
                }
                let n = count as usize;
                let start = value(line, &t[3], &env)?;
                let stop = value(line, &t[4], &env)?;
                let grid = match t[1].as_str() {
                    "lin" => FrequencyGrid::Linear {
                        points: n,
                        start,
                        stop,
                    },
                    "dec" => FrequencyGrid::Decade {
                        points_per_decade: n,
                        start,
                        stop,
                    },
                    "oct" => FrequencyGrid::Octave {
                        points_per_octave: n,
                        start,
                        stop,
                    },
                    _ => return Err(line.error("unknown AC grid")),
                };
                grid.values().map_err(|e| line.error(e))?;
                analyses.push(Analysis::Ac(grid));
            }
            ".tran" => {
                let uic = t.last().is_some_and(|s| s == "uic");
                let end = t.len() - usize::from(uic);
                if !(3..=5).contains(&end) {
                    return Err(line.error(".tran step stop [start [maxstep]] [uic]"));
                }
                let step = value(line, &t[1], &env)?;
                let stop = value(line, &t[2], &env)?;
                let mut options = TransientOptions::new(stop, step);
                let start = if end >= 4 {
                    value(line, &t[3], &env)?
                } else {
                    0.0
                };
                if start < 0.0 || start > stop {
                    return Err(line.error("invalid transient output start"));
                }
                if end == 5 {
                    options.max_step = value(line, &t[4], &env)?;
                }
                options.uic = uic;
                options.method = method;
                analyses.push(Analysis::Transient { options, start });
            }
            _ => {}
        }
    }
    let mut context = Context {
        builder: &mut builder,
        subs: &subs,
        source_map: &mut source_map,
        device_count: 0,
    };
    context.expand(
        &root,
        "",
        &HashMap::new(),
        &env,
        &HashMap::new(),
        &mut vec![],
    )?;
    Ok(ParsedNetlist {
        circuit: builder.build(),
        analyses,
        solver_options,
        source_map,
    })
}
struct Context<'a> {
    builder: &'a mut CircuitBuilder,
    subs: &'a HashMap<String, Subcircuit>,
    source_map: &'a mut BTreeMap<String, SourceSpan>,
    device_count: usize,
}
impl Context<'_> {
    fn expand(
        &mut self,
        lines: &[Line],
        prefix: &str,
        pins: &HashMap<String, String>,
        env: &HashMap<String, f64>,
        inherited: &HashMap<String, Model>,
        stack: &mut Vec<String>,
    ) -> PResult<()> {
        let mut models = inherited.clone();
        let mut own = std::collections::HashSet::new();
        for line in lines.iter().filter(|l| l.tokens[0] == ".model") {
            let (name, m) = model(line, env)?;
            if !own.insert(name.clone()) {
                return Err(line.error("duplicate model"));
            }
            models.insert(name, m);
        }
        let node_name = |name: &str| {
            if name == "0" {
                "0".to_string()
            } else if let Some(pin) = pins.get(name) {
                pin.clone()
            } else {
                format!("{prefix}{name}")
            }
        };
        for line in lines {
            let t = &line.tokens;
            let name = &t[0];
            if name.starts_with('.') {
                match name.as_str() {
                    ".model" | ".param" => {}
                    ".ic" | ".nodeset" => {
                        let mut k = 1;
                        while k < t.len() {
                            if k + 5 >= t.len()
                                || t[k] != "v"
                                || t[k + 1] != "("
                                || t[k + 3] != ")"
                                || t[k + 4] != "="
                            {
                                return Err(line.error("expected v(node)=value"));
                            }
                            let node = self.builder.node(&node_name(&t[k + 2]));
                            let v = value(line, &t[k + 5], env)?;
                            if name == ".ic" {
                                self.builder.initial_voltage(node, v)
                            } else {
                                self.builder.nodeset(node, v)
                            }
                            .map_err(|e| line.error(e))?;
                            k += 6;
                        }
                    }
                    ".temp" | ".options" | ".option" | ".op" | ".dc" | ".ac" | ".tran"
                        if prefix.is_empty() => {}
                    _ => return Err(line.error(format!("unsupported directive {name}"))),
                }
                continue;
            }
            if name.starts_with('x') {
                let pos = t
                    .iter()
                    .position(|s| s == "params:")
                    .or_else(|| (1..t.len()).find(|&k| t.get(k + 1).is_some_and(|s| s == "=")))
                    .unwrap_or(t.len());
                if pos < 2 {
                    return Err(line.error("invalid subcircuit instance"));
                }
                let subname = &t[pos - 1];
                let sub = self
                    .subs
                    .get(subname)
                    .ok_or_else(|| line.error(format!("unknown subcircuit {subname}")))?;
                if stack.contains(subname) || stack.len() >= 64 {
                    return Err(line.error("recursive/deep subcircuit expansion"));
                }
                if pos - 2 != sub.pins.len() {
                    return Err(line.error(format!(
                        "subcircuit {subname} expects {} pins",
                        sub.pins.len()
                    )));
                }
                let overrides = assignments(line, &t[pos..])?;
                let mut evaluated = HashMap::new();
                for (k, v) in overrides {
                    if !sub.defaults.contains_key(&k) {
                        return Err(line.error(format!("unknown subcircuit parameter {k}")));
                    }
                    evaluated.insert(k, value(line, &v, env)?);
                }
                let defs = collect_params(&sub.lines, sub.defaults.clone())?;
                let local =
                    expression::resolve(env, &defs, &evaluated).map_err(|e| line.error(e))?;
                let mapped = sub
                    .pins
                    .iter()
                    .cloned()
                    .zip(t[1..pos - 1].iter().map(|s| node_name(s)))
                    .collect();
                stack.push(subname.clone());
                self.expand(
                    &sub.lines,
                    &format!("{prefix}{name}:"),
                    &mapped,
                    &local,
                    &models,
                    stack,
                )?;
                stack.pop();
                continue;
            }
            self.device_count += 1;
            if self.device_count > 1_000_000 {
                return Err(line.error("device expansion limit"));
            }
            let full_name = format!("{prefix}{name}");
            let mut node_count = 2;
            let element = match name.as_bytes().first().copied() {
                Some(b'r') | Some(b'c') | Some(b'l') => {
                    if t.len() < 4 {
                        return Err(line.error("passive device needs two nodes and a value"));
                    }
                    let v = value(line, &t[3], env)?;
                    let extra = assignments(line, &t[4..])?;
                    let ic = extra.get("ic").map(|s| value(line, s, env)).transpose()?;
                    if extra.keys().any(|k| k != "ic") || name.starts_with('r') && !extra.is_empty()
                    {
                        return Err(line.error("unsupported passive instance parameter"));
                    }
                    if name.starts_with('r') {
                        Element::Resistor(v)
                    } else if name.starts_with('c') {
                        Element::Capacitor { capacitance: v, ic }
                    } else {
                        Element::Inductor { inductance: v, ic }
                    }
                }
                Some(b'v') | Some(b'i') => {
                    if t.len() < 4 {
                        return Err(line.error("source needs two nodes and a specification"));
                    }
                    let s = source(line, &t[3..], env)?;
                    if name.starts_with('v') {
                        Element::Voltage(s)
                    } else {
                        Element::Current(s)
                    }
                }
                Some(b'e') | Some(b'g') => {
                    if t.len() != 6 {
                        return Err(line.error("E/G needs four nodes and linear gain"));
                    }
                    node_count = 4;
                    let gain = value(line, &t[5], env)?;
                    if name.starts_with('e') {
                        Element::Vcvs(gain)
                    } else {
                        Element::Vccs(gain)
                    }
                }
                Some(b'f') | Some(b'h') => {
                    if t.len() != 5 {
                        return Err(line.error("F/H needs two nodes, branch device and gain"));
                    }
                    let control = format!("{prefix}{}", t[3]);
                    let gain = value(line, &t[4], env)?;
                    if name.starts_with('f') {
                        Element::Cccs { control, gain }
                    } else {
                        Element::Ccvs { control, gain }
                    }
                }
                Some(b'd') | Some(b'q') | Some(b'm') => {
                    node_count = if name.starts_with('d') {
                        2
                    } else if name.starts_with('m') {
                        4
                    } else if t.get(4).is_some_and(|s| models.contains_key(s)) {
                        3
                    } else {
                        4
                    };
                    let model_name = t
                        .get(node_count + 1)
                        .ok_or_else(|| line.error("missing device model"))?;
                    let model = models
                        .get(model_name)
                        .ok_or_else(|| line.error(format!("unknown model {model_name}")))?
                        .clone();
                    let valid = if name.starts_with('d') {
                        model.kind == ModelKind::Diode
                    } else if name.starts_with('q') {
                        matches!(model.kind, ModelKind::Npn | ModelKind::Pnp)
                    } else {
                        matches!(model.kind, ModelKind::Nmos | ModelKind::Pmos)
                    };
                    if !valid {
                        return Err(line.error("device/model type mismatch"));
                    }
                    let mut g = Geometry::default();
                    let rest = &t[node_count + 2..];
                    g.off = rest.iter().any(|s| s == "off");
                    let filtered: Vec<_> = rest
                        .iter()
                        .filter(|s| s.as_str() != "off")
                        .cloned()
                        .collect();
                    let mut rest = filtered.as_slice();
                    if !name.starts_with('m')
                        && rest
                            .first()
                            .is_some_and(|_| rest.get(1).is_none_or(|s| s != "="))
                    {
                        g.area = value(line, &rest[0], env)?;
                        rest = &rest[1..];
                    }
                    for (key, expr) in assignments(line, rest)? {
                        if key == "ic" {
                            let tokens = tokenize(&expr).map_err(|e| line.error(e))?;
                            let values = tokens
                                .iter()
                                .map(|s| value(line, s, env))
                                .collect::<PResult<Vec<_>>>()?;
                            let limit = if name.starts_with('d') {
                                1
                            } else if name.starts_with('q') {
                                2
                            } else {
                                3
                            };
                            if values.is_empty() || values.len() > limit {
                                return Err(line.error("invalid device IC voltage count"));
                            }
                            for (k, v) in values.into_iter().enumerate() {
                                g.initial_voltages[k] = Some(v);
                            }
                            continue;
                        }
                        let v = value(line, &expr, env)?;
                        match key.as_str() {
                            "area" if !name.starts_with('m') => g.area = v,
                            "m" => g.multiplier = v,
                            "temp" => g.temperature = Some(v),
                            "dtemp" => g.delta_temperature = v,
                            "w" if name.starts_with('m') => g.width = v,
                            "l" if name.starts_with('m') => g.length = v,
                            "ad" => g.drain_area = v,
                            "as" => g.source_area = v,
                            "pd" => g.drain_perimeter = v,
                            "ps" => g.source_perimeter = v,
                            "nrd" if name.starts_with('m') => g.drain_squares = v,
                            "nrs" if name.starts_with('m') => g.source_squares = v,
                            _ => {
                                return Err(
                                    line.error(format!("unsupported instance parameter {key}"))
                                );
                            }
                        }
                    }
                    Element::Semiconductor { model, geometry: g }
                }
                _ => return Err(line.error(format!("unsupported device {name}"))),
            };
            if t.len() < node_count + 1 {
                return Err(line.error("missing device terminals"));
            }
            let mut nodes: Vec<_> = t[1..node_count + 1]
                .iter()
                .map(|s| self.builder.node(&node_name(s)))
                .collect();
            if name.starts_with('q') && node_count == 3 {
                nodes.push(NodeId::GROUND);
            }
            self.builder
                .add(&full_name, &nodes, element)
                .map_err(|e| line.error(e))?;
            self.source_map.insert(full_name, line.span.clone());
        }
        Ok(())
    }
}
fn source(line: &Line, t: &[String], env: &HashMap<String, f64>) -> PResult<Source> {
    let mut s = Source::dc(0.0);
    let mut k = 0;
    let mut dc = false;
    let mut ac = false;
    while k < t.len() {
        match t[k].as_str() {
            "dc" => {
                if dc || k + 1 >= t.len() {
                    return Err(line.error("invalid/duplicate DC specification"));
                }
                s.dc = value(line, &t[k + 1], env)?;
                dc = true;
                k += 2;
            }
            "ac" => {
                if ac || k + 1 >= t.len() {
                    return Err(line.error("invalid/duplicate AC specification"));
                }
                let mag = value(line, &t[k + 1], env)?;
                k += 2;
                let phase =
                    if k < t.len() && !matches!(t[k].as_str(), "dc" | "pulse" | "sin" | "pwl") {
                        let v = value(line, &t[k], env)?;
                        k += 1;
                        v
                    } else {
                        0.0
                    };
                s.ac = Complex64::from_polar(mag, phase.to_radians());
                ac = true;
            }
            "pulse" | "sin" | "pwl" => {
                if s.waveform.is_some() || t.get(k + 1).is_none_or(|x| x != "(") {
                    return Err(line.error("waveform requires parentheses"));
                }
                let kind = t[k].as_str();
                k += 2;
                let mut args = Vec::new();
                while k < t.len() && t[k] != ")" {
                    args.push(value(line, &t[k], env)?);
                    k += 1;
                }
                if k == t.len() {
                    return Err(line.error("unclosed waveform"));
                }
                k += 1;
                s.waveform = Some(match kind {
                    "pulse" => {
                        if args.len() != 7 {
                            return Err(
                                line.error("PULSE requires low high delay rise fall width period")
                            );
                        }
                        Waveform::Pulse {
                            low: args[0],
                            high: args[1],
                            delay: args[2],
                            rise: args[3],
                            fall: args[4],
                            width: args[5],
                            period: args[6],
                        }
                    }
                    "sin" => {
                        if !(3..=6).contains(&args.len()) {
                            return Err(line.error(
                                "SIN requires offset amplitude frequency [delay damping phase]",
                            ));
                        }
                        Waveform::Sin {
                            offset: args[0],
                            amplitude: args[1],
                            frequency: args[2],
                            delay: *args.get(3).unwrap_or(&0.0),
                            damping: *args.get(4).unwrap_or(&0.0),
                            phase: *args.get(5).unwrap_or(&0.0),
                        }
                    }
                    _ => {
                        if args.is_empty() || args.len() % 2 != 0 {
                            return Err(line.error("PWL requires time/value pairs"));
                        }
                        Waveform::Pwl(args.chunks(2).map(|v| (v[0], v[1])).collect())
                    }
                });
            }
            _ => {
                if dc {
                    return Err(line.error(format!("unknown source field {}", t[k])));
                }
                s.dc = value(line, &t[k], env)?;
                dc = true;
                k += 1;
            }
        }
    }
    // Without a separate DC field SPICE uses the transient initial value for bias.
    if !dc && let Some(w) = &s.waveform {
        s.dc = w.value(0.0);
    }
    s.validate().map_err(|e| line.error(e))?;
    Ok(s)
}
