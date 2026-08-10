# terminal_dashboard

`machine pulse` -- a live terminal dashboard over `/proc`, written in FOL.

It samples the 1-minute load average, memory pressure and the runnable-process
count, paints them as coloured gauges with a Unicode sparkline of the load
history, and reacts to keystrokes. It paints a fixed number of frames and then
stops, so the same binary is usable interactively and from a script.

```
cd test/apps/showcases/terminal_dashboard
folc --package-store-root ../../../../lang/library code run -- --frames 24
```

Keys: `q` quit, `space` pause, `r` reset the history, `+`/`-` change the tick.

Options: `--frames N`, `--interval MS`, `--width COLS`, `--help`.

## Modules

- `src/input` -- turns the raw bytes from `std::io::read_key_ms` into named
  commands, including CSI escape sequences for the arrow keys
- `src/render` -- ANSI control sequences and the widgets (gauges, sparkline,
  padded columns); every routine returns a string so one frame is one write
- `src/app` -- the `/proc` parsers, the rolling state, the frame composition
  and the loop
- `src/main.fol` -- argument handling and terminal setup/teardown

`test/checks.fol` is the check suite (`folc code test`): it runs the parsers
and the widget arithmetic against fixtures and exits with the failure count.

## Notes

The sample history is a string of `'0'`..`'7'` digits rather than a `vec[int]`:
`vec` values cannot currently be appended to, assigned into by index, or
concatenated, so a string is the only growable sequence available.
