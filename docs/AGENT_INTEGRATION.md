# RIS-C integration brief for coding agents

Use this document when adding RIS-C to another project. For supported electrical
behavior, read [COMPATIBILITY.md](COMPATIBILITY.md). For full recipes and failure
contracts, read [INTEGRATION.md](INTEGRATION.md).

## Entry points

- Rust package: `risc-c`; import: `risc_c`.
- Add it with `cargo add risc-c --git https://github.com/Reid-Engineered/RIS-C`.
  Pin a reviewed commit with `--rev <full-commit-sha>`; commit app `Cargo.lock`.
  Do not invent a crates.io version, release tag, or FFI API.
- Simple in-memory deck: `risc_c::run_str(text)`.
- Deck and includes on disk: `risc_c::run_file(path)`.
- Custom settings: `risc_c::netlist::parse_str` / `parse_file`, change the parsed
  options or analyses, then `risc_c::run(parsed)`.
- Generated circuits or streaming: re-exported `CircuitBuilder`, `Element`,
  `NodeId`, `Simulator`, and `TransientOptions`.
- CLI package: `risc-c-cli`; executable: `spice`.
  `cargo install --git https://github.com/Reid-Engineered/RIS-C --rev <full-commit-sha> --locked risc-c-cli`.
- CLI invocation: `["spice", "run", deck_path, "--analysis", "op", "--output", "-"]`.
  Use a subprocess argument array, check exit status, parse stdout as CSV, and
  keep stderr separate. Use `ac` or `tran` only when declared in the deck.

## Contracts to preserve

1. First physical netlist line is the title; ground is `0`; end with `.end`.
   Runner calls require a declared `.op`, `.dc`, `.ac`, or `.tran`.
2. Rust values use SI units; temperature APIs use Celsius. SPICE `M` is milli;
   use `MEG` or `1e6` for mega.
3. `RunOutput.analyses` is in declaration order. Match `AnalysisData` explicitly.
   `RunOutput.operating_point()` returns the first declared OP result, if any.
4. Look up signals by name using `sample.get(&output.circuit, "v(out)")`.
   Never silently substitute zero for an unknown signal. AC results are complex;
   `norm()` is magnitude and `arg()` is phase in radians.
5. Propagate `RunError`; preserve parse locations and analysis identity. Do not
   report partial success if the runner returns an error. Streamed samples are
   partial until `transient_with` succeeds; cancellation returns `Error::Cancelled`.
6. Compile once when reusing a circuit; create independent simulators for
   concurrent jobs. Each runner call owns fresh numerical state.
7. Specify an appropriate maximum transient step and verify important waveforms
   under refinement. Do not change tolerances just to make a failed test pass.
8. The API and CLI are the integration boundary. Keep the solver in its dependency
   rather than copying internal device equations or relying on private modules.

## Working examples

- [Netlist and expected 5 V result](../crates/risc-c/examples/netlist.rs).
- [Generated circuit](../crates/risc-c/examples/builder.rs).
- [Transient with custom tolerances](../crates/risc-c/examples/transient.rs).
- [Complex AC response](../crates/risc-c/examples/ac.rs).
- [Streaming and cancellation](../crates/risc-c/examples/stream.rs).
- [Language-neutral CLI via Python](../examples/integrate_cli.py).

## Copyable implementation brief

> Add RIS-C to this project using its `risc-c` Git package, pinned to a reviewed
> commit. Use `run_str` for in-memory decks or `run_file` for relative includes.
> Preserve the typed analysis results, units, and `RunError` diagnostics. Add an
> integration test for a 10 V, equal-resistor divider whose output is 5 V within
> 1e-9 V. Run the host project's normal checks. If using the CLI instead, install
> `risc-c-cli`, execute `spice` with an argument array, check status, and parse
> named CSV columns. Do not introduce a language binding unless required.

Repository maintainers can run `python scripts/check_integration.py` to exercise
these recipes from a clean external consumer. It is included in native CI.
