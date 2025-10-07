# Changelog

All notable changes to the DAMM v2 Fee Distributor module will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-10-07

### Added
- Initial implementation of DAMM v2 Honorary Quote-Only Fee Position system
- `initialize_policy` instruction for setting up fee distribution parameters
- `initialize_honorary_position` instruction for creating LP positions
- `distribute_fees_crank` instruction with 24-hour gating and pagination support
- `claim_pool_fees` instruction for separate fee claiming
- PolicyPda account for storing distribution configuration
- ProgressPda account for tracking distribution cycles
- HonoraryPosition account for LP position state
- MockPoolState for testing DAMM v2 integration
- Comprehensive event emissions:
  - HonoraryPositionInitialized
  - QuoteFeesClaimed
  - InvestorPayoutPage
  - CreatorPayoutDayClosed
- Full test suite with 23+ test cases covering:
  - Policy initialization (valid/invalid parameters)
  - Honorary position setup and validation
  - Quote-only enforcement
  - 24-hour gating
  - Fee distribution calculations
  - Various locking scenarios
  - Dust handling
  - Daily cap enforcement
  - Pagination logic
  - PDA derivation
  - Overflow protection
- Complete documentation:
  - README.md with architecture, usage examples, and API reference
  - SECURITY.md with security considerations and best practices
  - Inline code documentation
  - TypeScript integration test examples
- Error codes for all failure modes:
  - InvalidFeeShare
  - InvalidPoolMints
  - QuoteMintMismatch
  - BaseFeeDetected
  - TooSoonForNextDistribution
  - BelowMinimumPayout
  - ArithmeticOverflow
  - InvalidPageSequence

### Features
- **Quote-Only Fee Accrual**: Validates and enforces that only quote token fees are earned
- **24-Hour Distribution Cycle**: Automatic gating prevents premature distributions
- **Pro-Rata Distribution**: Fair distribution based on locked token amounts
- **Pagination Support**: Handle large investor lists efficiently
- **Dust-Free Accounting**: Floor rounding with proper dust handling
- **Daily Distribution Caps**: Configurable limits on distribution amounts
- **Minimum Payout Thresholds**: Prevent dust payouts to investors
- **Locked Ratio Calculation**: Dynamic investor share based on vesting progress
- **Safe Arithmetic**: u128 intermediate calculations prevent overflow
- **Deterministic PDAs**: All addresses use deterministic seed derivation
- **Idempotent Operations**: Retry-safe distribution logic
- **Event-Driven Architecture**: Complete event emissions for indexing

### Technical Details
- Built with Anchor Framework 0.29.0
- Solana SDK 1.18.26
- Rust 2021 edition
- Zero unsafe code
- Comprehensive input validation
- Atomic state transitions

### Testing
- 23+ unit tests in Rust
- Integration tests in TypeScript
- Mock implementations for Streamflow and DAMM v2
- Test coverage includes:
  - Happy path scenarios
  - Edge cases (0%, 100% locked)
  - Error conditions
  - Mathematical precision
  - PDA derivation
  - Event emissions

### Documentation
- Complete README with:
  - Architecture overview
  - PDA seed documentation
  - Instruction reference
  - Fee distribution formulas
  - Usage examples
  - Failure mode descriptions
- Security documentation
- Inline code comments
- TypeScript usage examples

### Performance
- Efficient u128 arithmetic for large amounts
- Minimal account space usage:
  - PolicyPda: 83 bytes
  - ProgressPda: 125 bytes
  - HonoraryPosition: 81 bytes
- Pagination support for large investor lists
- No unnecessary CPI calls

### Security
- All PDAs use deterministic derivation
- Bump seeds validated and stored
- No unchecked accounts without validation
- Arithmetic overflow protection
- Access control on all mutations
- 24-hour gating prevents spam
- Daily caps limit exposure

## [Unreleased]

### Planned
- Production Streamflow integration
- Production DAMM v2 CP-AMM integration
- Additional fee distribution strategies
- Multi-pool support
- Enhanced monitoring and analytics
- Optimistic distribution for gas savings
- Batch investor operations

### Under Consideration
- Governance integration
- Fee distribution schedules beyond 24h
- Dynamic fee share adjustments
- Emergency pause mechanism
- Multi-signature policy updates
- On-chain analytics aggregation

---

## Version History

- **v0.1.0** (2025-10-07): Initial release with core functionality
