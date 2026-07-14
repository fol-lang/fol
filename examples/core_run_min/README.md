# Core Run Minimal

This package is an executable `fol_model = "core"` artifact with no bundled
`std` dependency. It proves that a no-heap program can use the ordinary
`fol code build` and `fol code run` path without gaining access to hosted
language APIs.

Frontend process launching is separate from the source-language capability
model.

Its `build.fol` sets `fol_model = "core"` on `graph.add_exe(...)` and then
passes the resulting artifact to `graph.add_run(...)`. The run declaration
does not promote the artifact to `memo` or bundled `std`; it only gives the
frontend a runnable graph target.

This example assumes a host-compatible target. A cross-target build still
needs an appropriate external runner.
