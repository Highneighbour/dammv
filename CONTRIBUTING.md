# Contributing to DAMM v2 Fee Distributor

Thank you for your interest in contributing to the DAMM v2 Fee Distributor project! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Submitting Changes](#submitting-changes)
- [Code Style](#code-style)
- [Security](#security)

## Code of Conduct

This project adheres to a code of conduct that all contributors are expected to follow:

- Be respectful and inclusive
- Focus on constructive feedback
- Prioritize security and correctness
- Document your changes thoroughly
- Test extensively before submitting

## Getting Started

### Prerequisites

- Rust 1.70 or higher
- Solana CLI 1.16 or higher
- Anchor 0.29.0 or higher
- Node.js 18 or higher
- Git

### Development Setup

1. Clone the repository:
   ```bash
   git clone <repository-url>
   cd damm-v2-fee-distributor
   ```

2. Install dependencies:
   ```bash
   npm install
   cargo build
   ```

3. Run tests:
   ```bash
   cargo test
   anchor test
   ```

## Making Changes

### Branch Naming

Use descriptive branch names:
- `feature/add-new-distribution-logic`
- `fix/pda-derivation-bug`
- `docs/update-readme`
- `test/add-edge-cases`

### Commit Messages

Follow conventional commit format:
```
type(scope): brief description

Detailed explanation of changes, if necessary.

Fixes #issue_number
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `test`: Test additions or modifications
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `security`: Security-related changes

Examples:
```
feat(crank): add support for dynamic page sizes

Add ability to specify custom page sizes in distribution crank
to optimize gas costs for varying investor counts.

Fixes #123
```

## Testing

### Unit Tests

Run Rust unit tests:
```bash
cargo test --package damm-v2-fee-distributor --lib
```

All new features must include unit tests covering:
- Happy path scenarios
- Edge cases
- Error conditions
- Input validation

### Integration Tests

Run TypeScript integration tests:
```bash
anchor test
```

Integration tests should cover:
- Full instruction flows
- Account state transitions
- Event emissions
- Cross-instruction interactions

### Test Coverage

Aim for high test coverage:
- All new functions should have tests
- All error paths should be tested
- All mathematical formulas should be validated
- All PDAs should have derivation tests

## Submitting Changes

### Pull Request Process

1. **Fork the repository** and create your branch from `main`

2. **Make your changes** following the code style guidelines

3. **Add tests** for your changes

4. **Update documentation** if needed:
   - Update README.md if adding new features
   - Update inline documentation
   - Update CHANGELOG.md
   - Add/update examples

5. **Run all tests** and ensure they pass:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   anchor test
   ```

6. **Commit your changes** with clear commit messages

7. **Push to your fork** and submit a pull request

8. **Respond to feedback** from reviewers

### Pull Request Template

Your PR should include:

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
- [ ] Unit tests added/updated
- [ ] Integration tests added/updated
- [ ] All tests pass

## Documentation
- [ ] Code comments added/updated
- [ ] README updated (if needed)
- [ ] CHANGELOG updated

## Security
- [ ] No new security vulnerabilities introduced
- [ ] Input validation added for new parameters
- [ ] PDA derivation is deterministic

## Checklist
- [ ] Code follows project style guidelines
- [ ] Self-review completed
- [ ] No compiler warnings
- [ ] Tests pass locally
```

## Code Style

### Rust

Follow Rust best practices:
- Use `rustfmt` for formatting
- Use `clippy` for linting
- Prefer explicit types over inference in public APIs
- Add documentation comments for public items
- Use descriptive variable names
- Avoid `unwrap()` in production code
- Use `Result` and `Option` appropriately

Format your code:
```bash
cargo fmt
```

Check for issues:
```bash
cargo clippy -- -D warnings
```

### Documentation

- Add doc comments for all public items:
  ```rust
  /// Calculates the investor fee share based on locked ratio
  ///
  /// # Arguments
  /// * `locked_total` - Total locked amount across all investors
  /// * `y0_total` - Total initial allocation at TGE
  ///
  /// # Returns
  /// The eligible investor share in basis points (0-10000)
  pub fn calculate_investor_share(locked_total: u64, y0_total: u64) -> u16 {
      // Implementation
  }
  ```

- Use clear, concise language
- Include examples for complex functions
- Document edge cases and assumptions

### TypeScript

Follow TypeScript best practices:
- Use TypeScript strict mode
- Add type annotations
- Use async/await over callbacks
- Handle errors explicitly
- Use descriptive variable names

## Security

### Security-Critical Changes

If your changes affect:
- PDA derivation
- Token transfers
- Access control
- Arithmetic calculations
- State transitions

Please:
1. Highlight security implications in PR description
2. Add extra tests for security edge cases
3. Request security review from maintainers
4. Consider potential attack vectors

### Reporting Security Issues

**DO NOT** create public GitHub issues for security vulnerabilities.

Instead:
1. Email the security team directly
2. Include detailed description and steps to reproduce
3. Suggest a fix if possible
4. Allow time for patch before disclosure

## Architecture Guidelines

### Adding New Instructions

When adding new instructions:
1. Define clear account validation
2. Use PDAs with deterministic seeds
3. Add comprehensive input validation
4. Emit events for state changes
5. Document account requirements
6. Add error handling
7. Write tests

### Modifying State Accounts

When modifying state accounts:
1. Consider migration path for existing data
2. Update space calculations
3. Maintain backwards compatibility if possible
4. Update documentation
5. Add tests for new fields

### Adding New Features

When adding new features:
1. Discuss design in an issue first
2. Consider impact on existing functionality
3. Maintain protocol invariants
4. Add comprehensive tests
5. Update all documentation
6. Consider gas costs

## Review Process

### What Reviewers Look For

- **Correctness**: Does the code do what it claims?
- **Security**: Are there any vulnerabilities?
- **Testing**: Is test coverage adequate?
- **Documentation**: Is the code well-documented?
- **Style**: Does it follow project conventions?
- **Performance**: Is it reasonably efficient?
- **Maintainability**: Is the code easy to understand?

### Review Timeline

- Initial review: Within 3-5 business days
- Follow-up reviews: Within 2 business days
- Security reviews: May take longer

## Getting Help

If you need help:
- Check existing documentation
- Search existing issues
- Ask questions in pull requests
- Create a discussion for design questions

## Recognition

Contributors will be:
- Listed in CHANGELOG.md
- Credited in release notes
- Recognized in project documentation

Thank you for contributing to DAMM v2 Fee Distributor!
