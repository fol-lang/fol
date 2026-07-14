# Memo Run Minimal

This package is an executable `fol_model = "memo"` artifact with no bundled
`std` dependency. It proves that an alloc-like heap program can use the
ordinary `fol code build` and `fol code run` path without gaining access to
hosted language APIs.

Frontend process launching is separate from the source-language capability
model.
