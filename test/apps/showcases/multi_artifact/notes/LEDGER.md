# Ledger notes

Amounts are whole cents everywhere. Text amounts are parsed once, on the way
in, and formatted once, on the way out; nothing in between touches a float.

Settlement rounds a total to the nearest five cents, halves away from zero.
