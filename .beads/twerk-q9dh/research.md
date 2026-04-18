# Research: axum-oapi Version Compatibility

## Investigation Results

### Key Finding: `axum-oapi` Does Not Exist

The crate `axum-oapi` is **not available** on crates.io:

```
$ cargo info axum-oapi
error: could not find `axum-oapi` in registry `https://github.com/rust-lang/crates.io-index`
```

### Alternative: `utoipa-axum`

The project currently uses `utoipa-axum` v0.2.0, which is the correct and maintained package:

- **Current version**: 0.2.0
- **Utoipa version**: 5.4.0
- **Axum version**: 0.8.8
- **License**: MIT OR Apache-2.0
- **Rust version**: 1.75+
- **Repository**: https://github.com/juhaku/utoipa
- **Documentation**: https://docs.rs/utoipa-axum/0.2.0

### Workspace Configuration

The project's `Cargo.toml` correctly specifies:

```toml
utoipa = { version = "5.4", features = ["time"] }
utoipa-axum = "0.2"
```

This configuration is compatible and up-to-date.

### Conclusion

No action required. The project is already using the correct package (`utoipa-axum`) which is the official Utoipa integration for Axum. The reference to `axum-oapi` appears to be a misnomer or confusion with `utoipa-axum`.

## Recommendation

- Keep current `utoipa-axum = "0.2"` in workspace Cargo.toml
- No version updates needed - 0.2.0 is the latest stable release
- The current setup is fully compatible and functional
