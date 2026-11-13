use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Number(f64),
    Name(String),
    Op(char),
    End,
}
fn tokens(text: &str) -> Result<Vec<Token>, String> {
    let text = text.trim().trim_start_matches('{').trim_end_matches('}');
    let s = text.as_bytes();
    let mut k = 0;
    let mut out = Vec::new();
    while k < s.len() {
        let ch = s[k] as char;
        if ch.is_whitespace() {
            k += 1;
            continue;
        }
        if "+-*/^(),".contains(ch) {
            out.push(Token::Op(ch));
            k += 1;
            continue;
        }
        if ch.is_ascii_digit() || ch == '.' {
            let start = k;
            k += 1;
            while k < s.len() && (s[k].is_ascii_digit() || s[k] == b'.') {
                k += 1;
            }
            if k < s.len() && (s[k] == b'e' || s[k] == b'E') {
                // 'e' starts an exponent only when followed by a signed or unsigned digit.
                let mut j = k + 1;
                if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
                    j += 1;
                }
                if j < s.len() && s[j].is_ascii_digit() {
                    k = j + 1;
                    while k < s.len() && s[k].is_ascii_digit() {
                        k += 1;
                    }
                }
            }
            let number_end = k;
            while k < s.len() && s[k].is_ascii_alphabetic() {
                k += 1;
            }
            let base = text[start..number_end]
                .parse::<f64>()
                .map_err(|_| format!("invalid number {}", &text[start..k]))?;
            let suffix = text[number_end..k].to_ascii_lowercase();
            let multiplier = if suffix.starts_with("meg") {
                1e6
            } else if suffix.starts_with("mil") {
                25.4e-6
            } else {
                match suffix.chars().next() {
                    None => 1.0,
                    Some('t') => 1e12,
                    Some('g') => 1e9,
                    Some('k') => 1e3,
                    Some('m') => 1e-3,
                    Some('u') => 1e-6,
                    Some('n') => 1e-9,
                    Some('p') => 1e-12,
                    Some('f') => 1e-15,
                    Some(_) => return Err(format!("unknown numeric suffix {suffix}")),
                }
            };
            out.push(Token::Number(base * multiplier));
            continue;
        }
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = k;
            k += 1;
            while k < s.len() && ((s[k] as char).is_ascii_alphanumeric() || s[k] == b'_') {
                k += 1;
            }
            out.push(Token::Name(text[start..k].to_ascii_lowercase()));
            continue;
        }
        return Err(format!("unexpected expression character {ch}"));
    }
    out.push(Token::End);
    Ok(out)
}
struct Parser<'a, F: FnMut(&str) -> Result<f64, String>> {
    tokens: Vec<Token>,
    k: usize,
    depth: usize,
    lookup: &'a mut F,
}
impl<F: FnMut(&str) -> Result<f64, String>> Parser<'_, F> {
    fn expr(&mut self, min: u8) -> Result<f64, String> {
        if self.depth >= 64 {
            return Err("expression nesting limit (64)".into());
        }
        self.depth += 1;
        let result = self.expr_inner(min);
        self.depth -= 1;
        result
    }
    fn expr_inner(&mut self, min: u8) -> Result<f64, String> {
        let token = self.tokens[self.k].clone();
        self.k += 1;
        let mut left = match token {
            Token::Number(x) => x,
            Token::Op('+') => self.expr(5)?,
            Token::Op('-') => -self.expr(5)?,
            Token::Op('(') => {
                let v = self.expr(0)?;
                self.expect(')')?;
                v
            }
            Token::Name(name) => {
                if self.tokens.get(self.k) == Some(&Token::Op('(')) {
                    self.k += 1;
                    let a = self.expr(0)?;
                    let b = if self.tokens.get(self.k) == Some(&Token::Op(',')) {
                        self.k += 1;
                        Some(self.expr(0)?)
                    } else {
                        None
                    };
                    self.expect(')')?;
                    match (name.as_str(), b) {
                        ("abs", None) => a.abs(),
                        ("sqrt", None) => a.sqrt(),
                        ("exp", None) => a.exp(),
                        ("log", None) | ("ln", None) => a.ln(),
                        ("sin", None) => a.sin(),
                        ("cos", None) => a.cos(),
                        ("min", Some(b)) => a.min(b),
                        ("max", Some(b)) => a.max(b),
                        _ => return Err(format!("unsupported function/arity {name}")),
                    }
                } else if name == "pi" {
                    std::f64::consts::PI
                } else {
                    (self.lookup)(&name)?
                }
            }
            _ => return Err("expected expression".into()),
        };
        loop {
            let (op, l, r) = match self.tokens.get(self.k) {
                Some(Token::Op('+')) => ('+', 1, 2),
                Some(Token::Op('-')) => ('-', 1, 2),
                Some(Token::Op('*')) => ('*', 3, 4),
                Some(Token::Op('/')) => ('/', 3, 4),
                Some(Token::Op('^')) => ('^', 7, 6),
                _ => break,
            };
            if l < min {
                break;
            }
            self.k += 1;
            let right = self.expr(r)?;
            left = match op {
                '+' => left + right,
                '-' => left - right,
                '*' => left * right,
                '/' => left / right,
                '^' => left.powf(right),
                _ => unreachable!(),
            };
        }
        if left.is_finite() {
            Ok(left)
        } else {
            Err("nonfinite expression result".into())
        }
    }
    fn expect(&mut self, c: char) -> Result<(), String> {
        if self.tokens.get(self.k) == Some(&Token::Op(c)) {
            self.k += 1;
            Ok(())
        } else {
            Err(format!("expected '{c}'"))
        }
    }
}
pub fn evaluate_with<F: FnMut(&str) -> Result<f64, String>>(
    text: &str,
    mut lookup: F,
) -> Result<f64, String> {
    if text.len() > 4096 {
        return Err("expression length limit (4096 bytes)".into());
    }
    let mut p = Parser {
        tokens: tokens(text)?,
        k: 0,
        depth: 0,
        lookup: &mut lookup,
    };
    let v = p.expr(0)?;
    if p.tokens.get(p.k) != Some(&Token::End) {
        Err("trailing expression tokens".into())
    } else {
        Ok(v)
    }
}
pub fn evaluate(text: &str, env: &HashMap<String, f64>) -> Result<f64, String> {
    evaluate_with(text, |name| {
        env.get(name)
            .copied()
            .ok_or_else(|| format!("unresolved parameter {name}"))
    })
}
pub fn resolve(
    parent: &HashMap<String, f64>,
    defs: &HashMap<String, String>,
    overrides: &HashMap<String, f64>,
) -> Result<HashMap<String, f64>, String> {
    fn get(
        name: &str,
        parent: &HashMap<String, f64>,
        defs: &HashMap<String, String>,
        done: &mut HashMap<String, f64>,
        stack: &mut Vec<String>,
    ) -> Result<f64, String> {
        if let Some(v) = done.get(name) {
            return Ok(*v);
        }
        if stack.len() >= 64 {
            return Err("parameter dependency nesting limit".into());
        }
        if stack.iter().any(|n| n == name) {
            return Err(format!(
                "cyclic parameter dependency: {} -> {name}",
                stack.join(" -> ")
            ));
        }
        if let Some(expr) = defs.get(name) {
            stack.push(name.into());
            let v = evaluate_with(expr, |n| get(n, parent, defs, done, stack))?;
            stack.pop();
            done.insert(name.into(), v);
            Ok(v)
        } else {
            parent
                .get(name)
                .copied()
                .ok_or_else(|| format!("unresolved parameter {name}"))
        }
    }
    let mut done = overrides.clone();
    for name in defs.keys() {
        get(name, parent, defs, &mut done, &mut vec![])?;
    }
    let mut env = parent.clone();
    env.extend(done);
    Ok(env)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expressions() {
        let e = HashMap::from([("r".into(), 2.0)]);
        assert_eq!(evaluate("{r * 1MEG + 1M}", &e).unwrap(), 2e6 + 0.001);
        assert_eq!(evaluate("-2^2 + max(1,3)", &e).unwrap(), -1.0);
        assert!(evaluate("sqrt(-1)", &e).is_err());
    }
    #[test]
    fn cycles() {
        let d = HashMap::from([("a".into(), "b".into()), ("b".into(), "a".into())]);
        assert!(resolve(&HashMap::new(), &d, &HashMap::new()).is_err());
    }
}
