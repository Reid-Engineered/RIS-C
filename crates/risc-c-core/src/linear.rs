use crate::{Complex64, Error, Result};
use faer::sparse::linalg::solvers::{Lu, SymbolicLu};
use faer::sparse::{Pair, SparseColMatRef, SymbolicSparseColMat};
use faer::{Mat, prelude::Solve};
use std::collections::{BTreeSet, HashMap};

#[derive(Debug)]
pub(crate) struct Pattern {
    pub n: usize,
    pub symbolic: SymbolicSparseColMat<usize>,
    pub lu: SymbolicLu<usize>,
    pub coords: Vec<(usize, usize)>,
    slots: HashMap<(usize, usize), usize>,
    row_slots: Vec<Vec<usize>>,
}
impl Pattern {
    pub fn new(n: usize, entries: &BTreeSet<(usize, usize)>) -> Result<Self> {
        let indices: Vec<_> = entries.iter().map(|&(r, c)| Pair::new(r, c)).collect();
        let (symbolic, _) = SymbolicSparseColMat::try_new_from_indices(n, n, &indices)
            .map_err(|e| Error::Circuit(format!("matrix pattern: {e:?}")))?;
        let lu = SymbolicLu::try_new(symbolic.as_ref())
            .map_err(|e| Error::Singular(format!("symbolic: {e:?}")))?;
        let mut coords = Vec::new();
        for c in 0..n {
            for &r in &symbolic.row_idx()[symbolic.col_ptr()[c]..symbolic.col_ptr()[c + 1]] {
                coords.push((r, c));
            }
        }
        let slots = coords
            .iter()
            .copied()
            .enumerate()
            .map(|(k, rc)| (rc, k))
            .collect();
        let mut row_slots = vec![Vec::new(); n];
        for (k, &(r, _)) in coords.iter().enumerate() {
            row_slots[r].push(k);
        }
        Ok(Self {
            n,
            symbolic,
            lu,
            coords,
            slots,
            row_slots,
        })
    }
    pub fn slot(&self, r: usize, c: usize) -> usize {
        self.slots[&(r, c)]
    }
    pub fn row_slots(&self, r: usize) -> &[usize] {
        &self.row_slots[r]
    }
    pub fn zeros(&self) -> Vec<f64> {
        vec![0.0; self.coords.len()]
    }
    pub fn solve(&self, a: &[f64], b: &[f64]) -> Result<Vec<f64>> {
        if !a.iter().chain(b).all(|x| x.is_finite()) {
            return Err(Error::NonFinite("real linear solve".into()));
        }
        let mut row = vec![0.0_f64; self.n];
        for (k, &(r, _)) in self.coords.iter().enumerate() {
            row[r] = row[r].max(a[k].abs());
        }
        if row.contains(&0.0) {
            return Err(Error::Singular("empty equation".into()));
        }
        let mut col = vec![0.0_f64; self.n];
        for (k, &(r, c)) in self.coords.iter().enumerate() {
            col[c] = col[c].max(a[k].abs() / row[r]);
        }
        if col.contains(&0.0) {
            return Err(Error::Singular("unconstrained unknown".into()));
        }
        let scaled: Vec<_> = self
            .coords
            .iter()
            .enumerate()
            .map(|(k, &(r, c))| a[k] / row[r] / col[c])
            .collect();
        let mat = SparseColMatRef::new(self.symbolic.as_ref(), &scaled);
        let lu = Lu::try_new_with_symbolic(self.lu.clone(), mat)
            .map_err(|e| Error::Singular(format!("real LU: {e:?}")))?;
        let rhs = Mat::from_fn(self.n, 1, |r, _| b[r] / row[r]);
        let x = lu.solve(&rhs);
        let mut out: Vec<_> = (0..self.n).map(|r| x[(r, 0)] / col[r]).collect();
        if out.iter().any(|v| !v.is_finite()) {
            return Err(Error::Singular("nonfinite factorization result".into()));
        }
        for attempt in 0..3 {
            // An ideal grounded voltage source can require an exactly zero
            // Newton update. LU roundoff in that singleton equation otherwise
            // has unit componentwise backward error, however small it is.
            for (r, &target) in b.iter().enumerate() {
                let mut nonzero = self.row_slots(r).iter().copied().filter(|&k| a[k] != 0.0);
                if let Some(k) = nonzero.next()
                    && nonzero.next().is_none()
                {
                    out[self.coords[k].1] = target / a[k];
                }
            }
            let mut residual = b.to_vec();
            let mut scale: Vec<_> = b.iter().map(|v| v.abs()).collect();
            for (k, &(r, c)) in self.coords.iter().enumerate() {
                residual[r] -= a[k] * out[c];
                scale[r] += a[k].abs() * out[c].abs();
            }
            if residual
                .iter()
                .zip(&scale)
                .all(|(r, s)| r.abs() <= 1e-8 * s.max(1e-30))
            {
                return Ok(out);
            }
            if attempt < 2 {
                // Reuse the factorization to refine difficult scaled MNA solves;
                // the same backward-error requirement still gates acceptance.
                let rhs = Mat::from_fn(self.n, 1, |r, _| residual[r] / row[r]);
                let correction = lu.solve(&rhs);
                for r in 0..self.n {
                    out[r] += correction[(r, 0)] / col[r];
                }
            }
        }
        Err(Error::Singular(
            "linear residual exceeds backward-error tolerance".into(),
        ))
    }

    pub fn solve_complex(&self, a: &[Complex64], b: &[Complex64]) -> Result<Vec<Complex64>> {
        if !a
            .iter()
            .chain(b)
            .all(|x| x.re.is_finite() && x.im.is_finite())
        {
            return Err(Error::NonFinite("linear solve".into()));
        }
        let mut row = vec![0.0_f64; self.n];
        for (k, &(r, _)) in self.coords.iter().enumerate() {
            row[r] = row[r].max(a[k].norm());
        }
        if let Some(r) = row.iter().position(|&v| v == 0.0) {
            return Err(Error::Singular(format!("empty equation {r}")));
        }
        let mut col = vec![0.0_f64; self.n];
        for (k, &(r, c)) in self.coords.iter().enumerate() {
            col[c] = col[c].max(a[k].norm() / row[r]);
        }
        if col.contains(&0.0) {
            return Err(Error::Singular("unconstrained unknown".into()));
        }
        let scaled: Vec<faer::c64> = self
            .coords
            .iter()
            .enumerate()
            .map(|(k, &(r, c))| {
                let v = a[k] / row[r] / col[c];
                faer::c64::new(v.re, v.im)
            })
            .collect();
        let mat = SparseColMatRef::new(self.symbolic.as_ref(), &scaled);
        let lu = Lu::try_new_with_symbolic(self.lu.clone(), mat)
            .map_err(|e| Error::Singular(format!("LU: {e:?}")))?;
        let rhs = Mat::from_fn(self.n, 1, |r, _| {
            let v = b[r] / row[r];
            faer::c64::new(v.re, v.im)
        });
        let x = lu.solve(&rhs);
        let out: Vec<_> = (0..self.n)
            .map(|r| {
                let v = x[(r, 0)];
                Complex64::new(v.re, v.im) / col[r]
            })
            .collect();
        if out.iter().any(|v| !v.re.is_finite() || !v.im.is_finite()) {
            return Err(Error::Singular("nonfinite factorization result".into()));
        }
        let mut residual = b.to_vec();
        let mut scale: Vec<_> = b.iter().map(|v| v.norm()).collect();
        for (k, &(r, c)) in self.coords.iter().enumerate() {
            residual[r] -= a[k] * out[c];
            scale[r] += a[k].norm() * out[c].norm();
        }
        if residual
            .iter()
            .zip(&scale)
            .any(|(v, s)| v.norm() > 1e-8 * s.max(1e-30))
        {
            return Err(Error::Singular(
                "linear residual exceeds backward-error tolerance".into(),
            ));
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peroxide::fuga::{LinearAlgebra, Row, matrix};
    #[test]
    fn peroxide_dense_oracle_and_complex() {
        // An MNA-like nonsymmetric matrix with a zero branch diagonal.
        let dense = vec![1e-12, -1e-12, 1.0, -1e-12, 2e-12, 0.0, 1.0, 0.0, 0.0];
        let entries = (0..3).flat_map(|r| (0..3).map(move |c| (r, c))).collect();
        let p = Pattern::new(3, &entries).unwrap();
        let a: Vec<_> = p.coords.iter().map(|&(r, c)| dense[r * 3 + c]).collect();
        let rhs = vec![0.0, 0.0, 10.0];
        let x = p.solve(&a, &rhs).unwrap();
        let oracle = matrix(dense, 3, 3, Row).solve(&rhs, peroxide::traits::matrix::SolveKind::LU);
        for (a, b) in x.iter().zip(oracle) {
            assert!((a - b).abs() < 1e-8);
        }
        let ac: Vec<_> = a.iter().map(|v| Complex64::new(*v, 0.0)).collect();
        let xc = p
            .solve_complex(
                &ac,
                &rhs.iter()
                    .map(|v| Complex64::new(0.0, *v))
                    .collect::<Vec<_>>(),
            )
            .unwrap();
        assert!((xc[1] - Complex64::new(0.0, 5.0)).norm() < 1e-10);
    }
}
