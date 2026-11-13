//! Consistent initial voltage differences, including floating capacitor/device networks.
use crate::{circuit::CompiledData, *};
use std::collections::{HashMap, HashSet};
type Graph = Vec<Vec<(usize, f64)>>;
fn edge(graph: &mut Graph, a: usize, b: usize, value: f64) {
    graph[a].push((b, -value));
    graph[b].push((a, value));
}
fn solve(graph: &Graph, seed: &[f64], ground: usize) -> Result<(Vec<usize>, Vec<f64>)> {
    let mut labels = vec![usize::MAX; graph.len()];
    let mut values = vec![0.0; graph.len()];
    for root in std::iter::once(ground).chain(0..ground) {
        if labels[root] != usize::MAX {
            continue;
        }
        labels[root] = root;
        values[root] = if root == ground { 0.0 } else { seed[root] };
        let mut stack = vec![root];
        while let Some(a) = stack.pop() {
            for &(b, offset) in &graph[a] {
                let value = values[a] + offset;
                if labels[b] == usize::MAX {
                    labels[b] = root;
                    values[b] = value;
                    stack.push(b);
                } else if (values[b] - value).abs()
                    > 1e-10 + 1e-10 * values[b].abs().max(value.abs())
                {
                    return Err(Error::Analysis(
                        "conflicting initial voltage differences or ideal source constraints"
                            .into(),
                    ));
                }
            }
        }
    }
    Ok((labels, values))
}
pub(crate) fn constraints(
    data: &CompiledData,
    uic: bool,
) -> Result<(Vec<f64>, HashMap<usize, f64>)> {
    let ground = data.nvolt;
    let mut x = data.initial.clone();
    let mut seed = x[..ground].to_vec();
    seed.push(0.0);
    let mut original = vec![vec![]; ground + 1];
    if !uic {
        for d in &data.devices {
            match &d.element {
                Element::Voltage(s) => edge(
                    &mut original,
                    d.nodes[0].unwrap_or(ground),
                    d.nodes[1].unwrap_or(ground),
                    s.dc,
                ),
                Element::Inductor { .. } => edge(
                    &mut original,
                    d.nodes[0].unwrap_or(ground),
                    d.nodes[1].unwrap_or(ground),
                    0.0,
                ),
                _ => {}
            }
        }
    }
    let (existing, _) = solve(&original, &seed, ground)?;
    let mut graph = original;
    let mut affected = Vec::new();
    let mut impose = |a: Option<usize>, b: Option<usize>, value: f64| {
        let a = a.unwrap_or(ground);
        let b = b.unwrap_or(ground);
        edge(&mut graph, a, b, value);
        affected.push(a);
        affected.push(b);
    };
    for (&node, &value) in &data.ic {
        impose(Some(node), None, value);
    }
    if uic {
        for d in &data.devices {
            match &d.element {
                Element::Inductor { ic: Some(v), .. } => x[d.branch.unwrap()] = *v,
                Element::Capacitor { ic: Some(v), .. } => impose(d.nodes[0], d.nodes[1], *v),
                Element::Semiconductor { model, geometry } => {
                    let ic = geometry.initial_voltages;
                    match model.kind {
                        ModelKind::Diode => {
                            if let Some(v) = ic[0] {
                                impose(d.nodes[0], d.nodes[1], v);
                            }
                        }
                        ModelKind::Npn | ModelKind::Pnp => {
                            if let Some(v) = ic[0] {
                                impose(d.nodes[1], d.nodes[2], v);
                            }
                            if let Some(v) = ic[1] {
                                impose(d.nodes[0], d.nodes[2], v);
                            }
                        }
                        _ => {
                            for (k, terminal) in [0, 1, 3].into_iter().enumerate() {
                                if let Some(v) = ic[k] {
                                    impose(d.nodes[terminal], d.nodes[2], v);
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    let (components, values) = solve(&graph, &seed, ground)?;
    let touched: HashSet<_> = affected.iter().map(|&n| components[n]).collect();
    let mut assigned = HashSet::new();
    let mut constraints = HashMap::new();
    for node in 0..ground {
        x[node] = values[node];
        if touched.contains(&components[node])
            && existing[node] != existing[ground]
            && assigned.insert(existing[node])
        {
            constraints.insert(node, values[node]);
        }
    }
    Ok((x, constraints))
}
