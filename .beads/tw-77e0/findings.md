# GO-PLAN: twerk module plan 15 — eval module (twerk-core)

## Module Identification

Module #15 (alphabetically sorted, 1-indexed) = **`eval`** in **`twerk-core`**.

### Module Map (1-30)
1. api, 2. asl, 3. banner, 4. broker, 5. cache, 6. cli, 7. commands, 8. conf,
9. constants, 10. datastore, 11. domain, 12. engine, 13. env, 14. error,
**15. eval**, 16. fns, 17. handlers, 18. health, 19. helpers, 20. host, 21. httpx,
22. id, 23. job, 24. locker, 25. logging, 26. middleware, 27. migrate, 28. mount,
29. node, 30. redact

---

## Module Overview

The `eval` module provides expression evaluation with `{{ expression }}` template syntax and built-in functions. It targets 100% parity with Go's `internal/eval` package.

**Total size**: 2,260 lines across 13 files.

### Submodules

| File | Lines | Purpose |
|------|-------|---------|
| `mod.rs` | 66 | Module root, re-exports, `EvalError` enum, template regex |
| `condition.rs` | 142 | Boolean condition evaluation for job/task contexts |
| `context.rs` | 245 | Context building, JSON<->evalexpr conversion, function registration |
| `data_flow.rs` | 223 | ASL JSONPath data flow: input_path, result_path, output_path |
| `functions.rs` | 124 | Built-in functions: randomInt, sequence, split |
| `intrinsics.rs` | 323 | ASL intrinsic functions: format, hash, base64, array ops, math |
| `task.rs` | 255 | Recursive task template evaluation |
| `template.rs` | 179 | `{{ expr }}` template rendering engine |
| `transform.rs` | 33 | Expression sanitization and operator transforms (and/or -> &&/\|\|) |
| `state_dispatch.rs` | 81 | State machine evaluation dispatcher |
| `state_dispatch/arms.rs` | 215 | State-kind arm builders (Task, Choice, Parallel, Map) |
| `state_dispatch/metadata.rs` | 80 | State/machine metadata attachment |
| `tests/property_tests.rs` | 294 | Proptest + unit tests for template/transform/eval |

### Dependencies

- **evalexpr**: Core expression evaluation engine
- **regex**: Template `{{ }}` pattern matching
- **serde_json**: JSON value conversion
- **rand**: Random number generation
- **sha2**, **md5**: Hash functions
- **base64**: Encoding/decoding
- **uuid**: UUID generation
- **indexmap**: Ordered map for state machines
- **thiserror**: Error derivation

---

## Analysis

### Strengths
1. **Clean separation**: Each submodule has a single responsibility
2. **Functional design**: Pure functions dominate, side effects at boundaries
3. **Good test coverage**: Property-based tests via proptest for template engine
4. **Go parity claim**: Clear target behavior documented
5. **Error taxonomy**: `EvalError` with 4 variants covers the main failure modes

### Issues Found

#### 1. Condition evaluation has duplicated logic (DRY violation)
`evaluate_condition` and `evaluate_task_condition` in `condition.rs` share ~80% identical code — context building, sanitization, transform, eval. Only the context variables differ. Should extract a shared `evaluate_bool_expr(expr, context: &HashMap<String, Value>)` helper.

#### 2. `context.rs` has repetitive function registration (23 registrations)
Each function is registered with the same boilerplate pattern. Could use a declarative table-driven approach with a macro or `const` array of function pointers.

#### 3. `intrinsics.rs` O(n^2) uniqueness check
`array_unique_fn` uses `Vec::contains` which is O(n^2). For large arrays, use a `HashSet`-based dedup.

#### 4. `task.rs` exceeds 250 lines
`evaluate_task` function is 220+ lines with many sequential field evaluations. Consider extracting sub-evaluators (eval_string_fields, eval_collections, eval_subjob).

#### 5. Missing error context in data_flow.rs
`DataFlowError::PathNotFound` includes available keys but not the full path traversal trace for nested objects. Debugging deep path failures is harder.

#### 6. `transform_operators` is naive
`"and"` / `"or"` replacement via simple string replace will false-positive on identifiers containing "and"/"or" (e.g., variable name "band", "morbid"). Should use word-boundary-aware replacement.

#### 7. No caching of compiled regex
`get_template_regex()` compiles a new regex on every call. Should be `lazy_static` / `std::sync::LazyLock`.

#### 8. `state_dispatch` doesn't actually evaluate templates
Despite the name, `evaluate_state` dispatches through ASL constructors but never calls `evaluate_template` on expression fields. The `context` parameter is only passed to recursive `evaluate_state_machine` calls — actual template substitution in state fields appears to be handled elsewhere.

### Test Coverage Gaps
- No tests for `data_flow.rs` (input_path, result_path, output_path, apply_data_flow)
- No tests for `condition.rs` (evaluate_condition, evaluate_task_condition)
- No tests for `intrinsics.rs` individual functions
- No tests for `state_dispatch` module
- Property tests only cover template/transform, not intrinsics or data flow

---

## Implementation Plan

### Priority 1: Fix bugs and correctness issues
1. **Fix `transform_operators` false positives** — Use word-boundary regex instead of string replace
2. **Cache template regex** — Use `std::sync::LazyLock` for the regex
3. **Fix `array_unique_fn` O(n^2)** — Use `IndexSet` or `HashSet` for dedup

### Priority 2: DRY refactoring
4. **Extract shared condition evaluator** — DRY up `condition.rs` duplicated logic
5. **Table-driven function registration** — Replace repetitive registration in `context.rs`

### Priority 3: Test coverage
6. **Add data_flow tests** — Unit tests for JSONPath parsing and resolution
7. **Add condition evaluation tests** — Test job/task condition evaluation
8. **Add intrinsic function tests** — Test each intrinsic function individually
9. **Add state_dispatch tests** — Test state evaluation dispatcher

### Priority 4: Code quality
10. **Split `evaluate_task`** — Extract sub-evaluators to keep under 100 lines per function
11. **Improve DataFlowError** — Add path traversal trace for better debugging

### Out of scope
- Replacing evalexpr with a different expression engine
- Adding new intrinsic functions not in Go's implementation
- Performance optimization beyond the identified issues
