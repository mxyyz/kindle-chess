# Epic e01: Refactor & Unit Tests

> **Status**: Planned  
> **Priority**: High  
> **Total Estimate**: ~39 hours  
> **Total Tasks**: 15

## Quick Start

This epic addresses the technical debt and test coverage gaps identified in the codebase analysis. It's organized into 5 phases that can be executed sequentially.

### Prerequisites

- Rust toolchain installed
- Cargo available
- Git configured

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for specific module
cargo test --lib bitboard
cargo test --lib puzzle

# Run with verbose output
cargo test -- --nocapture

# Check test count
cargo test --lib 2>&1 | grep "test result"
```

## Directory Structure

```
specs/epics/e01-refactor-and-tests/
├── README.md              # This file
├── e01-refactor-and-tests.md  # Epic overview and detailed task descriptions
├── tasks.yaml            # Machine-readable task specifications
└── samples/
    ├── bitboard_tests_sample.rs   # Sample test code for bitboard module
    ├── mock_http_sample.rs        # Sample mock HTTP client implementation
    └── chess_error_sample.rs       # Sample ChessError implementation
```

## Phases Overview

### Phase 1: Test Infrastructure (P0) - 8 hours
**Goal**: Establish foundation for automated testing

| Task | ID | Estimate | Status |
|------|----|---------|--------|
| Create Test Directory Structure | t01 | 1h | pending |
| Add Mock HTTP Client | t02 | 2h | pending |
| Expand Bitboard Tests | t03 | 3h | pending |
| Expand Puzzle Tests | t04 | 2h | pending |

**Deliverables**:
- Test infrastructure in place
- Mock HTTP client for API testing
- 40+ new tests (bitboard: 20+, puzzle: 11+)

### Phase 2: Code Organization (P1) - 12 hours
**Goal**: Improve code maintainability by extracting monolithic files

| Task | ID | Estimate | Status |
|------|----|---------|--------|
| Extract Screen Structs | t05 | 4h | pending |
| Extract Screen Impls | t06 | 6h | pending |
| Create Layout Constants | t07 | 2h | pending |

**Deliverables**:
- 9 screen files extracted from `models/ui.rs`
- Screen impls co-located with structs
- Centralized layout constants
- `models/ui.rs` reduced from ~700 to ~100 lines
- `ui/screens.rs` reduced from ~1200 to ~200 lines

### Phase 3: Additional Test Coverage (P1) - 10 hours
**Goal**: Expand test coverage to core modules

| Task | ID | Estimate | Status |
|------|----|---------|--------|
| Add Game Module Tests | t08 | 2h | pending |
| Add OAuth Module Tests | t09 | 3h | pending |
| Add Board API Tests | t10 | 3h | pending |
| Add Widget Tests | t11 | 2h | pending |

**Deliverables**:
- 47+ new tests (game: 10+, oauth: 12+, board_api: 10+, widgets: 15+)
- Total test count: 92+

### Phase 4: Error Handling Improvements (P2) - 5 hours
**Goal**: Consistent error handling across the application

| Task | ID | Estimate | Status |
|------|----|---------|--------|
| Create ChessError Enum | t12 | 3h | pending |
| Migrate API Module to ChessError | t13 | 2h | pending |

**Deliverables**:
- Unified `ChessError` type
- All API functions return `ChessError`
- Consistent error handling pattern

### Phase 5: Continuous Integration (P2) - 4 hours
**Goal**: Automated testing in CI pipeline

| Task | ID | Estimate | Status |
|------|----|---------|--------|
| Add GitHub Actions Workflow | t14 | 2h | pending |
| Add Test Coverage Reporting | t15 | 2h | pending |

**Deliverables**:
- CI pipeline running on push/PR
- Coverage reporting with badge
- Initial coverage threshold: 20%

## Execution Strategy

### Recommended Order

1. **Start with Phase 1** - Test infrastructure is the foundation
2. **Phase 2** - Organization improvements (can be done in parallel with Phase 3)
3. **Phase 3** - Test coverage expansion
4. **Phase 4** - Error handling (requires Phase 1 foundation)
5. **Phase 5** - CI setup (can be done anytime after Phase 1)

### Parallel Execution

The following tasks have **no dependencies** and can be worked in parallel:

- **t01, t03, t04** (Phase 1) - All test infrastructure tasks
- **t05, t06, t07** (Phase 2) - All organization tasks
- **t08, t09, t10, t11** (Phase 3) - All test coverage tasks
- **t12, t13** (Phase 4) - Error handling tasks
- **t14, t15** (Phase 5) - CI tasks

### Review Checkpoints

1. **After Phase 1**: Review test infrastructure before moving to organization
2. **After Phase 2**: Review file organization before adding more tests
3. **After Phase 3**: Review test coverage before error handling changes
4. **After Phase 4**: Review error type design before CI setup

## Test Coverage Goals

| Phase | Target Coverage | Current | Delta |
|-------|----------------|---------|-------|
| Start | ~5% | ~5% | - |
| Phase 1 | ~20% | ~5% | +15% |
| Phase 2 | ~20% | ~20% | - |
| Phase 3 | ~60% | ~20% | +40% |
| Phase 4 | ~60% | ~60% | - |
| Phase 5 | ~60% | ~60% | - |

**Note**: Coverage percentages are estimates based on line counts. Actual coverage may vary.

## Sample Code

The `samples/` directory contains example implementations for key tasks:

- **bitboard_tests_sample.rs**: Complete test suite for bitboard module (Task t03)
- **mock_http_sample.rs**: Mock HTTP client implementation (Task t02)
- **chess_error_sample.rs**: ChessError enum and implementations (Task t12)

These samples demonstrate:
- The actual code that would be added
- Best practices for testing
- Integration patterns

## Quality Gates

Each task must pass these checks before being marked complete:

- [ ] Code compiles without warnings
- [ ] All existing tests pass
- [ ] New tests pass
- [ ] `cargo fmt` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] Commit message follows Conventional Commits

## Success Metrics

| Metric | Start | Target | Status |
|--------|-------|--------|--------|
| Test Count | 10 | 92+ | ❌ |
| Coverage | ~5% | 60%+ | ❌ |
| Monolithic Files | 2 | 0 | ❌ |
| Error Type | Multiple | Unified | ❌ |
| CI Pipeline | None | Active | ❌ |

## Getting Help

### Understanding the Codebase

- Read `specs/tech-architecture/tech-stack.md` for complete analysis
- Read `CLAUDE.md` for project conventions and build instructions

### Build Commands

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Run specific test
cargo test test_name

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy -- -D warnings
```

### Cross-Compilation (for Kindle)

```bash
cd kindle_chess
RUSTFLAGS="-C target-feature=+crt-static" \
    cross build --target armv7-unknown-linux-musleabi --release
```

## Related Documents

- [`specs/tech-architecture/tech-stack.md`](../tech-architecture/tech-stack.md) - Source analysis
- [`CLAUDE.md`](../../CLAUDE.md) - Project conventions
- [`kindle_chess/Cargo.toml`](../../kindle_chess/Cargo.toml) - Dependencies
