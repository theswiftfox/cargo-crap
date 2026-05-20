# Spec 14 — cfg-gated function deduplication

**Status:** In Progress
**Effort:** Medium
**Modules:** `src/complexity.rs`, `src/coverage.rs`, `src/merge.rs`

## Context

`syn` parses the raw source text without evaluating `#[cfg(...)]` attributes. When
two function definitions share the same qualified name but are mutually exclusive
via feature flags (e.g. `#[cfg(feature = "x")]` / `#[cfg(not(feature = "x"))]`),
both are extracted by the complexity pass. Only one variant is actually compiled
and instrumented by LLVM; the other has no `DA` records in the LCOV file.

Because `coverage_in_span` returns 100% when no executable lines exist in a
function's span, the uncompiled variant appears in the report with a misleadingly
perfect score. This produces report noise (two entries per logical function) and
a false safety signal (uncompiled code looks well-tested).

---

## Acceptance Tests

### Scenario: cfg-gated function excluded when not compiled

```
Given a source file containing two definitions of `do_thing`:
      one guarded by `#[cfg(feature = "foo")]`
      and one guarded by `#[cfg(not(feature = "foo"))]`
And   an LCOV file that contains DA records only for the first variant's lines
When  I run cargo-crap with that LCOV file
Then  only one entry for `do_thing` appears in the output
And   the entry corresponds to the variant whose lines are in the LCOV
```

### Scenario: cfg on parent impl block propagates to methods

```
Given a source file containing:
      #[cfg(feature = "foo")]
      impl Foo {
          fn bar() { ... }
      }
And   an LCOV file that has no DA records in the span of `bar`
When  I run cargo-crap
Then  `Foo::bar` is excluded from the output
```

### Scenario: cfg on parent inline module propagates to functions

```
Given a source file containing:
      #[cfg(feature = "foo")]
      mod platform {
          fn setup() { ... }
      }
And   an LCOV file that has no DA records in the span of `setup`
When  I run cargo-crap
Then  `platform::setup` is excluded from the output
```

### Scenario: non-cfg function with no DA lines keeps 100% coverage

```
Given a source file containing a function with only declarative code
      (e.g. a trait method signature, unreachable macro arm)
And   the function does NOT have a #[cfg(...)] attribute
And   the file is present in the LCOV but has no DA lines in the function's span
When  I run cargo-crap
Then  the function appears in the output with 100% coverage
And   its CRAP score reflects the 100% coverage (preserving existing behavior)
```

### Scenario: cfg-gated function with file absent from LCOV follows missing-coverage policy

```
Given a source file with a #[cfg(feature = "foo")] function
And   the file does NOT appear in the LCOV report at all
When  I run cargo-crap with --missing pessimistic
Then  the function is treated as 0% covered (normal policy, cfg_gated does not override)
```
