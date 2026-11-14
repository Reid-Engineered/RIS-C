use crate::*;
use std::collections::{HashMap, HashSet};

pub(crate) fn compile(
    devices: &[Device],
    branches: &HashMap<String, usize>,
) -> Result<Vec<(usize, usize, f64)>> {
    let windings: HashMap<_, _> = devices
        .iter()
        .filter_map(|d| match d.element {
            Element::Inductor { inductance, .. } => Some((d.name.as_str(), inductance)),
            _ => None,
        })
        .collect();
    let mut pairs = HashSet::new();
    let mut edges = Vec::new();
    let mut graph: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
    for d in devices {
        if let Element::Mutual {
            inductors,
            coupling,
        } = &d.element
        {
            let names = inductors.each_ref().map(|s| s.to_ascii_lowercase());
            let [a, b] = names.each_ref().map(|s| branches.get(s).copied());
            let l1 = windings.get(names[0].as_str());
            let l2 = windings.get(names[1].as_str());
            let (Some(a), Some(b), Some(l1), Some(l2)) = (a, b, l1, l2) else {
                return Err(Error::Circuit(format!(
                    "{}: K references must name inductors",
                    d.name
                )));
            };
            if a == b || !pairs.insert((a.min(b), a.max(b))) {
                return Err(Error::Circuit(format!(
                    "{}: self or duplicate mutual coupling",
                    d.name
                )));
            }
            let m = coupling * l1.sqrt() * l2.sqrt();
            if !m.is_finite() {
                return Err(Error::Circuit("mutual inductance overflow".into()));
            }
            edges.push((a, b, m));
            graph.entry(a).or_default().push((b, *coupling));
            graph.entry(b).or_default().push((a, *coupling));
        }
    }
    // Each connected winding group has a normalized inductance matrix with unit
    // diagonal. Pivoted semidefinite Cholesky also accepts ideal |K|=1 groups.
    let mut visited = HashSet::new();
    for &root in graph.keys() {
        if !visited.insert(root) {
            continue;
        }
        let mut group = vec![root];
        let mut pos = 0;
        while pos < group.len() {
            for &(next, _) in &graph[&group[pos]] {
                if visited.insert(next) {
                    group.push(next);
                }
            }
            pos += 1;
        }
        let n = group.len();
        let index: HashMap<_, _> = group.iter().enumerate().map(|(i, &b)| (b, i)).collect();
        let mut a = vec![vec![0.0; n]; n];
        for (i, &b) in group.iter().enumerate() {
            a[i][i] = 1.0;
            for &(other, k) in &graph[&b] {
                a[i][index[&other]] = k;
            }
        }
        let tol = 64.0 * f64::EPSILON * n as f64;
        for k in 0..n {
            let p = (k..n).max_by(|&i, &j| a[i][i].total_cmp(&a[j][j])).unwrap();
            a.swap(k, p);
            for row in &mut a {
                row.swap(k, p);
            }
            let pivot = a[k][k];
            if pivot < -tol {
                return Err(Error::Circuit(
                    "coupled inductance matrix has negative magnetic energy".into(),
                ));
            }
            if pivot <= tol {
                if a[k..]
                    .iter()
                    .any(|row| row[k..].iter().any(|v| v.abs() > tol))
                {
                    return Err(Error::Circuit(
                        "coupled inductance matrix is not positive semidefinite".into(),
                    ));
                }
                break;
            }
            for i in k + 1..n {
                for j in k + 1..n {
                    a[i][j] -= a[i][k] * a[k][j] / pivot;
                }
            }
        }
    }
    Ok(edges)
}
