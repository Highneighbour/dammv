# Deployment Guide

Complete guide for deploying the DAMM v2 Fee Distributor module to Solana networks.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Network Configuration](#network-configuration)
- [Build and Deploy](#build-and-deploy)
- [Initialization](#initialization)
- [Verification](#verification)
- [Production Checklist](#production-checklist)
- [Monitoring](#monitoring)
- [Troubleshooting](#troubleshooting)

## Prerequisites

### Required Tools

1. **Solana CLI** (1.16+)
   ```bash
   sh -c "$(curl -sSfL https://release.solana.com/stable/install)"
   solana --version
   ```

2. **Anchor CLI** (0.29.0+)
   ```bash
   cargo install --git https://github.com/coral-xyz/anchor --tag v0.29.0 anchor-cli
   anchor --version
   ```

3. **Rust** (1.70+)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustc --version
   ```

4. **Node.js** (18+)
   ```bash
   node --version
   npm --version
   ```

### Wallet Setup

Generate a deployment wallet:
```bash
solana-keygen new -o ~/.config/solana/deployer.json
```

Or use an existing keypair:
```bash
solana config set --keypair ~/.config/solana/deployer.json
```

## Network Configuration

### Localnet (Testing)

Start local validator:
```bash
solana-test-validator
```

Configure CLI:
```bash
solana config set --url localhost
```

Get test SOL:
```bash
solana airdrop 10
```

### Devnet (Integration Testing)

Configure CLI:
```bash
solana config set --url devnet
```

Get test SOL:
```bash
solana airdrop 5
```

Check balance:
```bash
solana balance
```

### Mainnet-Beta (Production)

⚠️ **Warning**: Only deploy to mainnet after thorough testing and security audit.

Configure CLI:
```bash
solana config set --url mainnet-beta
```

Ensure sufficient SOL:
```bash
solana balance
# Recommended: At least 5 SOL for deployment and buffer
```

## Build and Deploy

### Step 1: Build the Program

```bash
cd /path/to/damm-v2-fee-distributor

# Clean previous builds
anchor clean

# Build program
anchor build
```

Verify build success:
```bash
ls -lh target/deploy/damm_v2_fee_distributor.so
```

### Step 2: Update Program ID

Get the program ID:
```bash
solana address -k target/deploy/damm_v2_fee_distributor-keypair.json
```

Update `declare_id!` in `programs/damm-v2-fee-distributor/src/lib.rs`:
```rust
declare_id!("YourProgramIDHere");
```

Update `Anchor.toml`:
```toml
[programs.localnet]
damm_v2_fee_distributor = "YourProgramIDHere"
```

Rebuild:
```bash
anchor build
```

### Step 3: Deploy

Deploy to configured network:
```bash
anchor deploy
```

Or deploy with specific keypair:
```bash
anchor deploy --provider.wallet ~/.config/solana/deployer.json
```

Verify deployment:
```bash
solana program show <PROGRAM_ID>
```

### Step 4: Verify IDL

IDL is automatically generated at `target/idl/damm_v2_fee_distributor.json`.

Upload IDL to Anchor registry (optional):
```bash
anchor idl init --filepath target/idl/damm_v2_fee_distributor.json <PROGRAM_ID>
```

Update IDL (if redeploying):
```bash
anchor idl upgrade --filepath target/idl/damm_v2_fee_distributor.json <PROGRAM_ID>
```

## Initialization

### Step 1: Prepare Parameters

Define your policy parameters:
```typescript
const INVESTOR_FEE_SHARE_BPS = 5000; // 50%
const DAILY_CAP_QUOTE = 10_000_000_000; // 10k tokens (with 6 decimals)
const MIN_PAYOUT_QUOTE = 10_000_000; // 10 tokens (with 6 decimals)
```

### Step 2: Initialize Policy

```typescript
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair } from "@solana/web3.js";

// Load program
const program = anchor.workspace.DammV2FeeDistributor;

// Define vault and creator
const vault = Keypair.generate();
const creator = Keypair.generate(); // Use your actual creator keypair

// Derive policy PDA
const [policyPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("policy"), vault.publicKey.toBuffer()],
  program.programId
);

// Initialize policy
await program.methods
  .initializePolicy(
    INVESTOR_FEE_SHARE_BPS,
    new anchor.BN(DAILY_CAP_QUOTE),
    new anchor.BN(MIN_PAYOUT_QUOTE)
  )
  .accounts({
    policy: policyPda,
    vault: vault.publicKey,
    creator: creator.publicKey,
    systemProgram: anchor.web3.SystemProgram.programId,
  })
  .signers([creator])
  .rpc();

console.log("Policy initialized:", policyPda.toBase58());
```

### Step 3: Initialize Honorary Position

```typescript
import { TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token";

// DAMM v2 pool details
const poolPubkey = new PublicKey("YourPoolPublicKeyHere");
const baseMint = new PublicKey("YourBaseMintHere");
const quoteMint = new PublicKey("YourQuoteMintHere");

// Derive PDAs
const [progressPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("progress"), vault.publicKey.toBuffer()],
  program.programId
);

const [positionPda] = PublicKey.findProgramAddressSync(
  [
    Buffer.from("position"),
    vault.publicKey.toBuffer(),
    poolPubkey.toBuffer(),
  ],
  program.programId
);

const [positionOwnerPda] = PublicKey.findProgramAddressSync(
  [Buffer.from("investor_fee_pos_owner"), vault.publicKey.toBuffer()],
  program.programId
);

// Derive treasury ATA
const [treasuryAta] = PublicKey.findProgramAddressSync(
  [
    positionOwnerPda.toBuffer(),
    TOKEN_PROGRAM_ID.toBuffer(),
    quoteMint.toBuffer(),
  ],
  ASSOCIATED_TOKEN_PROGRAM_ID
);

// Initialize position
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
    payer: creator.publicKey,
    systemProgram: anchor.web3.SystemProgram.programId,
    tokenProgram: TOKEN_PROGRAM_ID,
    associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
  })
  .signers([creator])
  .rpc();

console.log("Honorary position initialized:", positionPda.toBase58());
```

## Verification

### Verify Policy Account

```typescript
const policyAccount = await program.account.policyPda.fetch(policyPda);
console.log("Policy:", {
  vault: policyAccount.vault.toBase58(),
  creator: policyAccount.creator.toBase58(),
  investorFeeShareBps: policyAccount.investorFeeShareBps,
  dailyCapQuote: policyAccount.dailyCapQuote.toString(),
  minPayoutQuote: policyAccount.minPayoutQuote.toString(),
});
```

### Verify Progress Account

```typescript
const progressAccount = await program.account.progressPda.fetch(progressPda);
console.log("Progress:", {
  vault: progressAccount.vault.toBase58(),
  pool: progressAccount.pool.toBase58(),
  quoteMint: progressAccount.quoteMint.toBase58(),
  lastDistributionTs: progressAccount.lastDistributionTs.toNumber(),
  totalDistributed: progressAccount.totalDistributed.toString(),
});
```

### Verify Position Account

```typescript
const positionAccount = await program.account.honoraryPosition.fetch(positionPda);
console.log("Position:", {
  pool: positionAccount.pool.toBase58(),
  owner: positionAccount.owner.toBase58(),
  totalFeesEarned: positionAccount.totalFeesEarned.toString(),
});
```

### Verify Events

Monitor program logs for events:
```bash
solana logs <PROGRAM_ID>
```

Look for:
- `HonoraryPositionInitialized`
- `QuoteFeesClaimed`
- `InvestorPayoutPage`
- `CreatorPayoutDayClosed`

## Production Checklist

Before deploying to mainnet:

### Security
- [ ] Complete security audit by reputable firm
- [ ] All tests passing (unit + integration)
- [ ] No compiler warnings
- [ ] Clippy checks passing
- [ ] Input validation reviewed
- [ ] PDA derivation verified
- [ ] Access control reviewed
- [ ] Arithmetic overflow checks in place

### Testing
- [ ] Tested on localnet
- [ ] Tested on devnet
- [ ] Stress tested with large investor counts
- [ ] Edge cases validated (0%, 100% locked)
- [ ] Event emissions verified
- [ ] Error handling tested
- [ ] Gas costs measured and acceptable

### Documentation
- [ ] README complete and accurate
- [ ] API documentation up to date
- [ ] Deployment guide reviewed
- [ ] Security considerations documented
- [ ] Known limitations documented

### Operations
- [ ] Monitoring system set up
- [ ] Alert thresholds configured
- [ ] Incident response plan ready
- [ ] Key management system in place
- [ ] Backup procedures established
- [ ] Rollback plan prepared

### Compliance
- [ ] Legal review completed (if required)
- [ ] License clearly specified
- [ ] Terms of service reviewed
- [ ] Privacy policy reviewed (if applicable)

### Performance
- [ ] Gas costs optimized
- [ ] Account space minimized
- [ ] CPI calls optimized
- [ ] Load testing completed

## Monitoring

### On-Chain Monitoring

Monitor key metrics:

```typescript
// Check program balance
const balance = await connection.getBalance(programId);

// Check account states
const policyAccount = await program.account.policyPda.fetch(policyPda);
const progressAccount = await program.account.progressPda.fetch(progressPda);

// Monitor treasury balance
const treasuryAccount = await getAccount(connection, treasuryAta);
console.log("Treasury balance:", treasuryAccount.amount);
```

### Event Monitoring

Set up event listeners:

```typescript
const listener = program.addEventListener("QuoteFeesClaimed", (event, slot) => {
  console.log("Fees claimed:", {
    pool: event.pool.toBase58(),
    amount: event.amount.toString(),
    timestamp: event.timestamp.toNumber(),
    slot,
  });
});

// Clean up
program.removeEventListener(listener);
```

### Alert Thresholds

Set up alerts for:
- Unusual fee claim amounts
- Failed distribution cranks
- Low treasury balance
- Unexpected state changes
- High transaction failure rates

## Troubleshooting

### Common Issues

#### Build Failures

**Issue**: Compilation errors
```
Solution:
- Ensure Rust version >= 1.70
- Update dependencies: cargo update
- Clean build: anchor clean && anchor build
```

#### Deployment Failures

**Issue**: Insufficient funds
```
Solution:
- Check balance: solana balance
- Request airdrop (devnet): solana airdrop 5
- Transfer SOL (mainnet)
```

**Issue**: Program account already exists
```
Solution:
- Use existing program ID
- Or close existing program: solana program close <PROGRAM_ID>
```

#### Initialization Failures

**Issue**: Invalid fee share percentage
```
Error: InvalidFeeShare
Solution: Ensure investor_fee_share_bps <= 10000
```

**Issue**: PDA already initialized
```
Error: Account already initialized
Solution:
- Use existing account
- Or use different vault keypair
```

#### Distribution Crank Failures

**Issue**: Too soon for next distribution
```
Error: TooSoonForNextDistribution
Solution: Wait 24 hours since last distribution
```

**Issue**: Invalid pool mints
```
Error: InvalidPoolMints
Solution: Verify base and quote mint addresses match pool
```

### Getting Help

1. Check logs:
   ```bash
   solana logs <PROGRAM_ID>
   ```

2. Verify account states:
   ```bash
   solana account <ACCOUNT_ADDRESS>
   ```

3. Check program state:
   ```bash
   solana program show <PROGRAM_ID>
   ```

4. Review transaction details:
   ```bash
   solana confirm -v <TRANSACTION_SIGNATURE>
   ```

## Upgrade Procedure

To upgrade the program:

1. **Build new version**:
   ```bash
   anchor build
   ```

2. **Test thoroughly on devnet**

3. **Upgrade program**:
   ```bash
   anchor upgrade target/deploy/damm_v2_fee_distributor.so --program-id <PROGRAM_ID>
   ```

4. **Verify upgrade**:
   ```bash
   solana program show <PROGRAM_ID>
   ```

5. **Update IDL**:
   ```bash
   anchor idl upgrade --filepath target/idl/damm_v2_fee_distributor.json <PROGRAM_ID>
   ```

## Rollback Procedure

If issues arise after deployment:

1. **Identify last known good version**

2. **Rebuild that version**:
   ```bash
   git checkout <LAST_GOOD_TAG>
   anchor build
   ```

3. **Upgrade to rollback**:
   ```bash
   anchor upgrade target/deploy/damm_v2_fee_distributor.so --program-id <PROGRAM_ID>
   ```

4. **Verify rollback success**

5. **Investigate and fix issue before redeployment**

## Support

For deployment issues:
- Create GitHub issue with full error logs
- Include transaction signatures
- Provide network and environment details
- Include relevant account addresses

---

**Production deployment requires thorough testing and security audit. Deploy at your own risk.**
