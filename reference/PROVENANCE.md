# Numerical reference

- Upstream: ngspice **47**, released August 11, 2026.
- Archive: https://downloads.sourceforge.net/project/ngspice/ng-spice-rework/47/ngspice-47.tar.gz
- SHA-256: `894e649651f1838a14095e5a5439e7d3aa63e87ede14d283173fda4fcdef675f`
- Manual: https://ngspice.sourceforge.io/docs/ngspice-47-manual.pdf
- Rebuild: `scripts/build-reference.sh` (C compiler, make, curl, tar, Python 3.11+).
- Configure: `--without-x --enable-xspice --disable-cider --disable-openmp --with-readline=no`.
- The `--disable-xspice` configuration fails to build the unmodified release (`OPT_ENH_RSHUNT` undeclared). Enabling it makes the external oracle build. No XSPICE code is linked into the Rust runtime.
- The archive, extracted source, build artifacts, and local toolchain are ignored. The archive is verified before extraction.

## Algorithm mapping

| Rust subsystem | ngspice 47 reference paths |
| --- | --- |
| MNA and circuit lifecycle | `src/spicelib/analysis/cktsetup.c`, `cktload.c`, `cktdojob.c`; device `*setup.c`/`*load.c` |
| Newton/convergence | `src/maths/ni/niiter.c`; `src/spicelib/analysis/cktconv.c` |
| Bias continuation | `src/spicelib/analysis/cktop.c` |
| Integration and step control | `src/maths/ni/niinteg.c`; `src/spicelib/analysis/dctran.c` |
| Diode equations/temperature/defaults | `src/spicelib/devices/dio/dioload.c`, `diotemp.c`, `diosetup.c` |
| Gummel–Poon equations/temperature | `src/spicelib/devices/bjt/bjtload.c`, `bjttemp.c`, `bjtsetup.c` |
| MOS1 equations/temperature | `src/spicelib/devices/mos1/mos1load.c`, `mos1temp.c`, `mos1set.c` |
| Meyer capacitances | `src/spicelib/devices/devsup.c`, `DEVqmeyer` |

The parser and circuit ownership model are new Rust implementations. Netlist parsing does **not** port `frontend/numparam` (LGPL). Sparse factorization uses faer, not ngspice Sparse/KLU. Integration error control uses step doubling rather than ngspice's divided-difference truncation estimator. Neither internal timestep sequences nor bitwise agreement are promised.

The device implementation adapts the equations in the listed Berkeley/ngspice sources. Original file copyrights include Regents of the University of California (1990), Thomas L. Quarles (1985 author), AlansFixes (2000 modifications), and diode changes by Paolo Nenzi (2003), Dietmar Warning (2012), and Arpad Buermen (2025). Retained license text is in `THIRD_PARTY_NOTICES` at the workspace root. No manual passages or LGPL solver/parser implementations are copied.
