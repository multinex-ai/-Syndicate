# Fix record

One per remediated finding. Pairs with the original evidence record.

```
finding_id:     the finding this closes
root_cause:     the underlying reason, not the symptom
siblings:       other places the same class appears (checked yes/no, list)
fix:            what changed and why this addresses the root cause
test_before:    regression test + that it FAILS on the unfixed build (receipt path)
test_after:     same test PASSES on the fixed build (receipt path)
repro_after:    the original reproduction no longer works (receipt path)
regressions:    surrounding tests run, result
detection:      rule/signature that fires on an attempt; detects technique not string
fp_rate:        observed or expected false-positive rate of the detection
exposure:       recon re-maps the surface as closed (yes/no, pointer)
downstream:     second/third-order effects of the fix (deps, behaviour, perf)
```

Resolved requires both `test_before` (failed) and `test_after` (passed), or a caught replay. Anything less is "fix proposed," not "resolved."
