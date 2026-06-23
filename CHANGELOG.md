# Changelog

## [0.7.0] - 2026-06-xx

### Added
- Z3 formal verification: cross-module ensures propagation, Expr::Match encoding, string length constraints
- FFI zero-copy struct-by-value (codegen path)
- Standard library: csv.mimi, template.mimi, crypto.mimi
- HTTP codegen: dual_net_tcp_client_echo

### Fixed
- F-16: StructByValue crash protection bypass (🔴)
- F-17: struct_buffers dangling pointer risk (🟠)
- F-18: CALLBACK_GLOBAL_STORE deadlock (🟠)
- Item 3: Z3 verifier unwrap panic (🔴)
- P0.1: Expr::Call unconstrained variables → false positives (🔴)
- P0.2: verify_func_call_silent missing Failed assertion (🔴)

### Security
- Item 2: transmute 'static field order dependency documented (🟠)
- Item 8: Fork async-signal-safety assessment (🟠)

## [0.6.0] - 2026-05-xx

### Added
- Windows target support (x86_64-pc-windows-gnu)
- Actor model: mailbox actor with lifecycle
- Regex builtins (match, find, replace)
- String contract runtime assertions

## [0.5.0] - 2026-04-xx

### Added
- Parasteps spawn+await via pthread (codegen)
- Contract verification (Z3)
- CI/CD: GitHub Actions (test/clippy/fmt/valgrind/ASan/UBSan/Miri/fuzz/cppcheck)

## [0.4.0] - 2026-03-xx

### Added
- Error system: String → Diagnostic replacement
- Arena escape detection (E0306)
- Write-write race detection (W005)
- Shared parameter contract warnings (E0502)

## [0.3.0] - 2026-02-xx

### Added
- Package management
- Documentation generation pipeline
- Dual backend (interpreter + codegen) baseline

## [0.2.0] - 2026-01-xx

### Added
- Basic language features
- LLVM codegen backend
- Contract system foundations
- MimiSpec integration

## [0.1.0] - 2025-12-xx

### Added
- Initial prototype
- Interpreter implementation
- Type checker
- CLI interface
