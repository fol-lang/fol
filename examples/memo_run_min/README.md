# Memo Run Minimal

This package is an executable `fol_model = "memo"` artifact with no bundled
`std` dependency. It proves that an alloc-like heap program can use the
ordinary `fol code build` and `fol code run` path without gaining access to
hosted language APIs.

Frontend process launching is separate from the source-language capability
model.

Its `build.fol` omits `fol_model`, selecting the default `memo` mode, and then
passes the executable artifact to `graph.add_run(...)`. That run declaration
does not add bundled `std` or expose hosted APIs.

This example assumes a host-compatible target. A cross-target build still
needs an appropriate external runner.
