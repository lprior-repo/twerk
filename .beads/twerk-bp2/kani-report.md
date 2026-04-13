Kani Rust Verifier 0.67.0 (cargo plugin)
   Compiling twerk-core v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-core)
warning: unused variable: `e`
   --> crates/twerk-core/src/domain_types.rs:191:30
    |
191 |             .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              ^  ------------------------------------------------------------- you might have meant to use string interpolation in this string literal
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
help: if this is intentional, prefix it with an underscore
    |
191 |             .unwrap_or_else(|_e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              +

   Compiling twerk-infrastructure v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-infrastructure)
   Compiling twerk-app v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-app)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.64s
warning: unused variable: `e`
   --> crates/twerk-core/src/domain_types.rs:191:30
    |
191 |             .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              ^  ------------------------------------------------------------- you might have meant to use string interpolation in this string literal
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
help: if this is intentional, prefix it with an underscore
    |
191 |             .unwrap_or_else(|_e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              +

   Compiling twerk-cli v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-cli)
   Compiling twerk-app v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-app)
   Compiling twerk-web v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-web)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.95s
warning: unused variable: `e`
   --> crates/twerk-core/src/domain_types.rs:191:30
    |
191 |             .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              ^  ------------------------------------------------------------- you might have meant to use string interpolation in this string literal
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
help: if this is intentional, prefix it with an underscore
    |
191 |             .unwrap_or_else(|_e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              +

   Compiling twerk-cli v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.08s
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
   Compiling twerk-core v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-core)
warning: unused variable: `e`
   --> crates/twerk-core/src/domain_types.rs:191:30
    |
191 |             .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              ^  ------------------------------------------------------------- you might have meant to use string interpolation in this string literal
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
help: if this is intentional, prefix it with an underscore
    |
191 |             .unwrap_or_else(|_e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              +

    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s
   Compiling twerk-core v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-core)
warning: unused variable: `e`
   --> crates/twerk-core/src/domain_types.rs:191:30
    |
191 |             .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              ^  ------------------------------------------------------------- you might have meant to use string interpolation in this string literal
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
help: if this is intentional, prefix it with an underscore
    |
191 |             .unwrap_or_else(|_e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              +

   Compiling twerk-infrastructure v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-infrastructure)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.91s
warning: unused variable: `e`
   --> crates/twerk-core/src/domain_types.rs:191:30
    |
191 |             .unwrap_or_else(|e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              ^  ------------------------------------------------------------- you might have meant to use string interpolation in this string literal
    |
    = note: `#[warn(unused_variables)]` (part of `#[warn(unused)]`) on by default
help: if this is intentional, prefix it with an underscore
    |
191 |             .unwrap_or_else(|_e| unreachable!("GoDuration was validated at construction: {e}"))
    |                              +

   Compiling twerk-web v0.1.0 (/home/lewis/src/twerk-bp2-r1/crates/twerk-web)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.97s
Manual Harness Summary:
No proof harnesses (functions with #[kani::proof]) were found to verify.
