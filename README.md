# DAMM v2 Honorary Quote-Only Fee Position + 24h Distribution Crank

A production-ready Anchor module for the Star fundraising protocol that implements an honorary LP position in DAMM v2 CP-AMM pools, accruing quote-token-only fees and distributing them to investors and creators on a 24-hour schedule.

## 🎯 Overview

This module enables automated, fair distribution of LP fees from DAMM v2 pools to fundraising participants based on their remaining vesting locks. It ensures:

- **Quote-only fee accrual** - No base token fees, only quote token fees
- **24-hour distribution cycle** - Permissionless crank runs once per day
- **Pro-rata distribution** - Investors receive fees proportional to locked amounts
- **Pagination support** - Handle large investor lists efficiently
- **Dust-free accounting** - Floor rounding with carryover to prevent token loss
- **Daily caps** - Configurable limits on distribution amounts

## 🏗️ Architecture

### Core Components

#### 1. **PolicyPda** - Fee Distribution Configuration
Stores the parameters for how fees are split between investors and creators:
- `investor_fee_share_bps`: Base percentage for investors (in basis points)
- `daily_cap_quote`: Maximum tokens to distribute per 24h cycle
- `min_payout_quote`: Minimum payout threshold to prevent dust

#### 2. **ProgressPda** - Distribution State Tracking
Maintains the current state of distribution cycles:
- `last_distribution_ts`: Timestamp of last distribution (for 24h gating)
- `total_distributed`: Cumulative amount distributed
- `carry_over_dust`: Dust from rounding to carry forward
- `current_page`: Current pagination cursor

#### 3. **HonoraryPosition** - LP Position State
Tracks the program-owned LP position in the DAMM v2 pool:
- `pool`: The CP-AMM pool address
- `owner`: Position owner PDA
- `total_fees_earned`: Cumulative fees earned
- `last_claim_ts`: Last fee claim timestamp

### PDA Seeds

| PDA | Seeds | Purpose |
|-----|-------|---------|
| **PolicyPda** | `["policy", vault]` | Fee distribution configuration |
| **ProgressPda** | `["progress", vault]` | Distribution cycle state |
| **HonoraryPosition** | `["position", vault, pool]` | LP position state |
| **InvestorFeePositionOwnerPda** | `["investor_fee_pos_owner", vault]` | Position owner authority |

## 📋 Instructions

### 1. `initialize_policy`

Sets up fee distribution parameters for a vault.

**Parameters:**
- `investor_fee_share_bps: u16` - Base investor share (0-10000)
- `daily_cap_quote: u64` - Max distribution per day
- `min_payout_quote: u64` - Minimum payout threshold

**Accounts:**
- `policy` - PolicyPda to initialize (PDA)
- `vault` - Vault/fundraising round identifier
- `creator` - Creator's pubkey (signer)
- `system_program` - System program

**Validation:**
- `investor_fee_share_bps <= 10000`

**Example:**
```rust
initialize_policy(
    investor_fee_share_bps: 5000, // 50%
    daily_cap_quote: 10_000 * 1_000_000, // 10k tokens
    min_payout_quote: 10 * 1_000_000, // 10 tokens
)
```

### 2. `initialize_honorary_position`

Creates the honorary LP position in a DAMM v2 pool.

**Accounts:**
- `policy` - PolicyPda (must exist)
- `progress` - ProgressPda to initialize (PDA)
- `position` - HonoraryPosition to initialize (PDA)
- `position_owner_pda` - PDA that owns the position
- `pool` - DAMM v2 CP-AMM pool account
- `base_mint` - Base token mint
- `quote_mint` - Quote token mint
- `treasury_ata` - Treasury token account (created)
- `payer` - Transaction fee payer (signer)

**Validation:**
- Pool contains both base and quote mints
- Quote mint correctly identified
- Pool configuration supports quote-only fees

**Events Emitted:**
- `HonoraryPositionInitialized`

**Example:**
```rust
initialize_honorary_position()
```

### 3. `distribute_fees_crank`

Executes the 24-hour distribution cycle (with pagination).

**Parameters:**
- `page: u32` - Current page number (0-indexed)
- `is_final_page: bool` - True on last page to close cycle
- `investor_accounts: Vec<InvestorDistribution>` - Investor data for this page

**Accounts:**
- `policy` - PolicyPda
- `progress` - ProgressPda (mutable)
- `position` - HonoraryPosition (mutable)
- `position_owner_pda` - Position owner PDA
- `treasury_ata` - Treasury token account (mutable)
- `token_program` - Token program

**Validation:**
- 24h elapsed since `last_distribution_ts` (or first run)
- Page sequence is valid
- Investor data is valid

**Distribution Formula:**

1. **Calculate locked ratio:**
   ```
   f_locked(t) = locked_total(t) / Y0
   ```

2. **Determine eligible investor share:**
   ```
   eligible_share_bps = min(investor_fee_share_bps, floor(f_locked * 10000))
   ```

3. **Calculate investor pool:**
   ```
   investor_fee_quote = floor(claimed_quote * eligible_share_bps / 10000)
   ```

4. **Distribute pro-rata:**
   ```
   weight_i = locked_i / locked_total
   payout_i = floor(investor_fee_quote * weight_i)
   ```

5. **Remainder to creator:**
   ```
   creator_share = claimed_quote - sum(payout_i)
   ```

**Events Emitted:**
- `QuoteFeesClaimed` (page 0)
- `InvestorPayoutPage` (every page)
- `CreatorPayoutDayClosed` (final page)

**Example:**
```rust
// Page 0: First batch of investors
distribute_fees_crank(
    page: 0,
    is_final_page: false,
    investor_accounts: vec![
        InvestorDistribution {
            investor_pubkey: investor1,
            locked_amount: 100_000 * 1_000_000,
            initial_allocation: 150_000 * 1_000_000,
        },
        // ... more investors
    ],
)

// Final page: Last batch + close day
distribute_fees_crank(
    page: 2,
    is_final_page: true,
    investor_accounts: vec![/* last batch */],
)
```

### 4. `claim_pool_fees`

Claims accumulated fees from the DAMM v2 pool position (separate from distribution).

**Accounts:**
- `position` - HonoraryPosition (mutable)
- `position_owner_pda` - Position owner PDA
- `token_program` - Token program

**Events Emitted:**
- `QuoteFeesClaimed`

## 🔐 Protocol Invariants

### 1. Quote-Only Enforcement
- Position MUST only accrue quote token fees
- Any base token fees → transaction fails
- Validated during position initialization and fee claims

### 2. 24-Hour Gating
- Minimum 86400 seconds between distribution cycles
- Enforced via `last_distribution_ts` check
- First distribution (ts=0) always allowed

### 3. Dust Handling
- All calculations use floor rounding
- Dust from rounding goes to creator on final page
- No tokens lost due to rounding errors

### 4. Mathematical Safety
- All fee calculations use u128 for intermediate values
- Prevents overflow on large amounts
- Results cast back to u64 after safe computation

### 5. Deterministic Seeds
- All PDAs use deterministic seed derivation
- No unsafe `UncheckedAccount` without validation
- Bump seeds stored in account state

## 📊 Fee Distribution Examples

### Example 1: 90% Locked, 50% Base Share

**Setup:**
- Total allocation (Y0): 200k tokens
- Currently locked: 180k tokens (90%)
- Claimed fees: 1000 tokens
- Base investor share: 50%

**Calculation:**
```
f_locked = 180k / 200k = 0.90 = 9000 bps
eligible_share = min(5000, 9000) = 5000 bps
investor_fee = floor(1000 * 5000 / 10000) = 500 tokens
creator_share = 1000 - 500 = 500 tokens
```

**Result:** 50/50 split

### Example 2: 30% Locked, 50% Base Share

**Setup:**
- Total allocation (Y0): 200k tokens
- Currently locked: 60k tokens (30%)
- Claimed fees: 1000 tokens
- Base investor share: 50%

**Calculation:**
```
f_locked = 60k / 200k = 0.30 = 3000 bps
eligible_share = min(5000, 3000) = 3000 bps
investor_fee = floor(1000 * 3000 / 10000) = 300 tokens
creator_share = 1000 - 300 = 700 tokens
```

**Result:** 30% to investors (capped by locked ratio), 70% to creator

### Example 3: Fully Unlocked

**Setup:**
- Total allocation (Y0): 200k tokens
- Currently locked: 0 tokens (0%)
- Claimed fees: 1000 tokens
- Base investor share: 50%

**Calculation:**
```
f_locked = 0 / 200k = 0 = 0 bps
eligible_share = min(5000, 0) = 0 bps
investor_fee = floor(1000 * 0 / 10000) = 0 tokens
creator_share = 1000 - 0 = 1000 tokens
```

**Result:** 100% to creator

## 🧪 Testing

### Unit Tests (Rust)

Located in `tests/fee_distributor_test.rs`:

```bash
cargo test
```

**Test Coverage:**
- ✅ Policy initialization with valid/invalid parameters
- ✅ Honorary position setup and validation
- ✅ Quote-only enforcement
- ✅ 24h gating (success and rejection)
- ✅ Fee distribution calculations
- ✅ All unlocked scenario (100% to creator)
- ✅ Partial locked scenario (capped distribution)
- ✅ Dust handling
- ✅ Daily cap enforcement
- ✅ Minimum payout threshold
- ✅ Pagination logic
- ✅ Idempotent retry protection
- ✅ Base fee detection
- ✅ PDA derivation
- ✅ Complex multi-investor scenarios
- ✅ Overflow protection

### Integration Tests (TypeScript)

Located in `tests/damm-v2-fee-distributor.ts`:

```bash
anchor test
```

**Test Coverage:**
- ✅ Full program deployment
- ✅ Policy initialization
- ✅ Honorary position creation
- ✅ Invalid parameter rejection
- ✅ Fee distribution math validation
- ✅ Various locking scenarios
- ✅ Daily cap and minimum payout enforcement
- ✅ PDA derivation verification

## 🚀 Setup & Deployment

### Prerequisites

- Rust 1.70+
- Solana CLI 1.16+
- Anchor 0.29.0+
- Node.js 18+

### Build

```bash
# Install dependencies
npm install

# Build program
anchor build

# Run tests
anchor test
```

### Local Deployment

```bash
# Start local validator
solana-test-validator

# Deploy program
anchor deploy

# Run tests against local validator
anchor test --skip-local-validator
```

### Devnet/Mainnet Deployment

```bash
# Configure cluster
solana config set --url devnet

# Deploy
anchor deploy --provider.cluster devnet
```

## 📝 Usage Example

### Complete Lifecycle

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";

// 1. Initialize policy
await program.methods
  .initializePolicy(
    5000, // 50% base investor share
    new anchor.BN(10_000_000_000), // 10k daily cap
    new anchor.BN(10_000_000) // 10 token minimum
  )
  .accounts({
    policy: policyPda,
    vault: vaultPubkey,
    creator: creatorKeypair.publicKey,
    systemProgram: SystemProgram.programId,
  })
  .signers([creatorKeypair])
  .rpc();

// 2. Initialize honorary position
await program.methods
  .initializeHonoraryPosition()
  .accounts({
    policy: policyPda,
    progress: progressPda,
    position: positionPda,
    positionOwnerPda,
    pool: poolPubkey,
    baseMint,
    quoteMint,
    treasuryAta,
    payer: payerKeypair.publicKey,
    // ... other accounts
  })
  .signers([payerKeypair])
  .rpc();

// 3. Run distribution crank (after 24h)
const investors = [
  {
    investorPubkey: investor1,
    lockedAmount: new anchor.BN(100_000_000_000),
    initialAllocation: new anchor.BN(150_000_000_000),
  },
  // ... more investors
];

await program.methods
  .distributeFeesCrank(
    0, // page
    true, // is_final_page
    investors
  )
  .accounts({
    policy: policyPda,
    progress: progressPda,
    position: positionPda,
    positionOwnerPda,
    treasuryAta,
    tokenProgram: TOKEN_PROGRAM_ID,
  })
  .rpc();
```

## 🔍 Events

All events are emitted for indexing and monitoring:

### `HonoraryPositionInitialized`
```rust
pub struct HonoraryPositionInitialized {
    pub vault: Pubkey,
    pub pool: Pubkey,
    pub position: Pubkey,
    pub quote_mint: Pubkey,
}
```

### `QuoteFeesClaimed`
```rust
pub struct QuoteFeesClaimed {
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}
```

### `InvestorPayoutPage`
```rust
pub struct InvestorPayoutPage {
    pub page: u32,
    pub investors_count: u32,
    pub total_paid: u64,
    pub timestamp: i64,
}
```

### `CreatorPayoutDayClosed`
```rust
pub struct CreatorPayoutDayClosed {
    pub creator: Pubkey,
    pub creator_payout: u64,
    pub total_distributed: u64,
    pub timestamp: i64,
}
```

## ⚠️ Failure Modes

### `InvalidFeeShare`
- Cause: `investor_fee_share_bps > 10000`
- Fix: Use value between 0-10000 (0% to 100%)

### `InvalidPoolMints`
- Cause: Pool doesn't contain expected base/quote mints
- Fix: Verify pool address and mint configuration

### `QuoteMintMismatch`
- Cause: Provided quote mint not in pool
- Fix: Ensure quote mint matches pool's token_a or token_b

### `BaseFeeDetected`
- Cause: Position would accrue base token fees
- Fix: Adjust pool configuration or position bounds

### `TooSoonForNextDistribution`
- Cause: Less than 24h since last distribution
- Fix: Wait until 86400 seconds have elapsed

### `BelowMinimumPayout`
- Cause: Calculated payout below `min_payout_quote`
- Fix: Accumulate more fees or lower minimum threshold

### `ArithmeticOverflow`
- Cause: Calculation exceeds u64::MAX
- Fix: This should never happen with proper u128 intermediates

### `InvalidPageSequence`
- Cause: Pages called out of order
- Fix: Call pages sequentially (0, 1, 2, ...)

## 🔗 Integration with Streamflow

The module is designed to integrate with Streamflow for reading locked token amounts. In production:

1. Query Streamflow contracts for each investor
2. Sum locked amounts across all streams
3. Pass aggregated data to `distribute_fees_crank`

**Mock Implementation (for testing):**
```rust
// In production, replace with actual Streamflow CPI calls
let locked_amount = get_streamflow_locked_amount(investor_pubkey, stream_id)?;
```

## 🔗 Integration with DAMM v2

The module interfaces with DAMM v2 CP-AMM pools. In production:

1. Use actual DAMM v2 program IDs
2. Call `claim_fee` instruction on pool
3. Verify position only earns quote fees
4. Monitor fee accrual via pool state

**Reference:** https://github.com/MeteoraAg/damm-v2

## 🛡️ Security Considerations

1. **PDA Validation:** All PDAs use deterministic derivation and bump validation
2. **Arithmetic Safety:** u128 intermediates prevent overflow
3. **Access Control:** Only appropriate signers can initialize policies
4. **Reentrancy:** No cross-program invocations during critical sections
5. **Dust Protection:** Floor rounding ensures no token loss

## 📄 License

MIT License - See LICENSE file for details

## 🤝 Contributing

This is a bounty submission for the Star fundraising protocol. For production use, please conduct a thorough security audit.

## 📞 Support

For issues or questions:
- GitHub Issues: Create an issue in this repository
- Documentation: Refer to inline code comments and this README

---

**Built with ❤️ for the Star protocol using Anchor Framework**
