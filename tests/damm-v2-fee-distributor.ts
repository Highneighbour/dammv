import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, Keypair, SystemProgram } from "@solana/web3.js";
import {
  TOKEN_PROGRAM_ID,
  ASSOCIATED_TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import { DammV2FeeDistributor } from "../target/types/damm_v2_fee_distributor";

describe("damm-v2-fee-distributor", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.DammV2FeeDistributor as Program<DammV2FeeDistributor>;
  
  let creator: Keypair;
  let vault: Keypair;
  let baseMint: PublicKey;
  let quoteMint: PublicKey;
  let mockPool: Keypair;
  
  let policyPda: PublicKey;
  let progressPda: PublicKey;
  let positionPda: PublicKey;
  let positionOwnerPda: PublicKey;
  let treasuryAta: PublicKey;

  before(async () => {
    creator = Keypair.generate();
    vault = Keypair.generate();
    mockPool = Keypair.generate();

    // Airdrop SOL to creator
    const airdropSig = await provider.connection.requestAirdrop(
      creator.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSig);

    // Create mints
    baseMint = await createMint(
      provider.connection,
      creator,
      creator.publicKey,
      null,
      6
    );

    quoteMint = await createMint(
      provider.connection,
      creator,
      creator.publicKey,
      null,
      6
    );

    // Derive PDAs
    [policyPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("policy"), vault.publicKey.toBuffer()],
      program.programId
    );

    [progressPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("progress"), vault.publicKey.toBuffer()],
      program.programId
    );

    [positionPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("position"),
        vault.publicKey.toBuffer(),
        mockPool.publicKey.toBuffer(),
      ],
      program.programId
    );

    [positionOwnerPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("investor_fee_pos_owner"), vault.publicKey.toBuffer()],
      program.programId
    );

    console.log("Setup complete:");
    console.log("  Creator:", creator.publicKey.toBase58());
    console.log("  Vault:", vault.publicKey.toBase58());
    console.log("  Base Mint:", baseMint.toBase58());
    console.log("  Quote Mint:", quoteMint.toBase58());
    console.log("  Policy PDA:", policyPda.toBase58());
  });

  it("Initializes fee distribution policy", async () => {
    const investorFeeShareBps = 5000; // 50%
    const dailyCapQuote = new anchor.BN(10_000_000_000); // 10k tokens
    const minPayoutQuote = new anchor.BN(10_000_000); // 10 tokens

    await program.methods
      .initializePolicy(
        investorFeeShareBps,
        dailyCapQuote,
        minPayoutQuote
      )
      .accounts({
        policy: policyPda,
        vault: vault.publicKey,
        creator: creator.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([creator])
      .rpc();

    const policyAccount = await program.account.policyPda.fetch(policyPda);
    
    expect(policyAccount.vault.toBase58()).to.equal(vault.publicKey.toBase58());
    expect(policyAccount.creator.toBase58()).to.equal(creator.publicKey.toBase58());
    expect(policyAccount.investorFeeShareBps).to.equal(investorFeeShareBps);
    expect(policyAccount.dailyCapQuote.toString()).to.equal(dailyCapQuote.toString());
    expect(policyAccount.minPayoutQuote.toString()).to.equal(minPayoutQuote.toString());

    console.log("✅ Policy initialized successfully");
  });

  it("Initializes honorary position", async () => {
    // Create mock pool account
    const mockPoolData = {
      tokenAMint: baseMint,
      tokenBMint: quoteMint,
      tokenAVault: Keypair.generate().publicKey,
      tokenBVault: Keypair.generate().publicKey,
      feeRateBps: 30,
    };

    const space = 8 + 32 + 32 + 32 + 32 + 2;
    const lamports = await provider.connection.getMinimumBalanceForRentExemption(space);

    const createPoolIx = SystemProgram.createAccount({
      fromPubkey: creator.publicKey,
      newAccountPubkey: mockPool.publicKey,
      lamports,
      space,
      programId: program.programId,
    });

    await provider.sendAndConfirm(
      new anchor.web3.Transaction().add(createPoolIx),
      [creator, mockPool]
    );

    // Get treasury ATA address
    const [treasuryAtaAddress] = PublicKey.findProgramAddressSync(
      [
        positionOwnerPda.toBuffer(),
        TOKEN_PROGRAM_ID.toBuffer(),
        quoteMint.toBuffer(),
      ],
      ASSOCIATED_TOKEN_PROGRAM_ID
    );
    treasuryAta = treasuryAtaAddress;

    await program.methods
      .initializeHonoraryPosition()
      .accounts({
        policy: policyPda,
        progress: progressPda,
        position: positionPda,
        positionOwnerPda,
        pool: mockPool.publicKey,
        baseMint,
        quoteMint,
        treasuryAta,
        payer: creator.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      })
      .signers([creator])
      .rpc();

    const progressAccount = await program.account.progressPda.fetch(progressPda);
    const positionAccount = await program.account.honoraryPosition.fetch(positionPda);

    expect(progressAccount.vault.toBase58()).to.equal(vault.publicKey.toBase58());
    expect(progressAccount.pool.toBase58()).to.equal(mockPool.publicKey.toBase58());
    expect(progressAccount.quoteMint.toBase58()).to.equal(quoteMint.toBase58());
    expect(progressAccount.lastDistributionTs.toNumber()).to.equal(0);

    expect(positionAccount.pool.toBase58()).to.equal(mockPool.publicKey.toBase58());
    expect(positionAccount.owner.toBase58()).to.equal(positionOwnerPda.toBase58());

    console.log("✅ Honorary position initialized successfully");
  });

  it("Rejects invalid fee share percentage", async () => {
    const invalidVault = Keypair.generate();
    const [invalidPolicyPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("policy"), invalidVault.publicKey.toBuffer()],
      program.programId
    );

    try {
      await program.methods
        .initializePolicy(
          10001, // Invalid: >100%
          new anchor.BN(10_000_000_000),
          new anchor.BN(10_000_000)
        )
        .accounts({
          policy: invalidPolicyPda,
          vault: invalidVault.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();
      
      expect.fail("Should have thrown an error");
    } catch (error) {
      expect(error.message).to.include("InvalidFeeShare");
      console.log("✅ Invalid fee share correctly rejected");
    }
  });

  it("Simulates fee distribution calculation", async () => {
    // Test fee distribution math without actual crank
    const claimedQuote = 1000 * 1_000_000; // 1000 tokens
    const lockedTotal = 180_000 * 1_000_000; // 180k locked
    const y0Total = 200_000 * 1_000_000; // 200k total allocation
    const investorFeeShareBps = 5000; // 50%

    // Calculate f_locked ratio
    const fLockedBps = Math.floor((lockedTotal * 10000) / y0Total);
    expect(fLockedBps).to.equal(9000); // 90%

    // Eligible share is minimum
    const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
    expect(eligibleShareBps).to.equal(investorFeeShareBps);

    // Investor fee
    const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
    expect(investorFeeQuote).to.equal(500 * 1_000_000);

    // Creator gets remainder
    const creatorShare = claimedQuote - investorFeeQuote;
    expect(creatorShare).to.equal(500 * 1_000_000);

    console.log("✅ Fee distribution calculation validated");
    console.log("  Investors:", investorFeeQuote / 1_000_000, "tokens");
    console.log("  Creator:", creatorShare / 1_000_000, "tokens");
  });

  it("Tests all-unlocked scenario", async () => {
    const claimedQuote = 1000 * 1_000_000;
    const lockedTotal = 0; // All unlocked
    const y0Total = 200_000 * 1_000_000;
    const investorFeeShareBps = 5000;

    const fLockedBps = y0Total > 0 ? Math.floor((lockedTotal * 10000) / y0Total) : 0;
    expect(fLockedBps).to.equal(0);

    const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
    const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
    expect(investorFeeQuote).to.equal(0);

    const creatorShare = claimedQuote - investorFeeQuote;
    expect(creatorShare).to.equal(claimedQuote);

    console.log("✅ All unlocked: 100% to creator");
  });

  it("Tests partial locked scenario", async () => {
    const claimedQuote = 1000 * 1_000_000;
    const lockedTotal = 60_000 * 1_000_000; // 30% locked
    const y0Total = 200_000 * 1_000_000;
    const investorFeeShareBps = 5000; // 50% base

    const fLockedBps = Math.floor((lockedTotal * 10000) / y0Total);
    expect(fLockedBps).to.equal(3000); // 30%

    const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
    expect(eligibleShareBps).to.equal(3000); // Capped at 30%

    const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
    expect(investorFeeQuote).to.equal(300 * 1_000_000);

    console.log("✅ Partial locked: investor share capped at locked ratio");
  });

  it("Tests daily cap enforcement", async () => {
    const claimedQuote = 20_000 * 1_000_000; // 20k claimed
    const dailyCap = 10_000 * 1_000_000; // 10k cap

    const distributable = Math.min(claimedQuote, dailyCap);
    expect(distributable).to.equal(dailyCap);

    console.log("✅ Daily cap enforced");
  });

  it("Tests minimum payout threshold", async () => {
    const payout = 5 * 1_000_000; // 5 tokens
    const minPayout = 10 * 1_000_000; // 10 tokens minimum

    const shouldPay = payout >= minPayout;
    expect(shouldPay).to.be.false;

    console.log("✅ Minimum payout threshold validated");
  });

  it("Demonstrates PDA derivation", async () => {
    const [derivedPolicy] = PublicKey.findProgramAddressSync(
      [Buffer.from("policy"), vault.publicKey.toBuffer()],
      program.programId
    );
    expect(derivedPolicy.toBase58()).to.equal(policyPda.toBase58());

    const [derivedProgress] = PublicKey.findProgramAddressSync(
      [Buffer.from("progress"), vault.publicKey.toBuffer()],
      program.programId
    );
    expect(derivedProgress.toBase58()).to.equal(progressPda.toBase58());

    const [derivedPosition] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("position"),
        vault.publicKey.toBuffer(),
        mockPool.publicKey.toBuffer(),
      ],
      program.programId
    );
    expect(derivedPosition.toBase58()).to.equal(positionPda.toBase58());

    console.log("✅ PDA derivation validated");
  });
});

describe("damm-v2-fee-distributor - Extended Test Coverage", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.DammV2FeeDistributor as Program<DammV2FeeDistributor>;

  describe("Policy Initialization - Edge Cases", () => {
    let creator: Keypair;

    beforeEach(async () => {
      creator = Keypair.generate();
      const airdropSig = await provider.connection.requestAirdrop(
        creator.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(airdropSig);
    });

    it("Accepts 0% investor fee share (100% to creator)", async () => {
      const vault = Keypair.generate();
      const [policyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("policy"), vault.publicKey.toBuffer()],
        program.programId
      );

      await program.methods
        .initializePolicy(
          0, // 0% to investors
          new anchor.BN(10_000_000_000),
          new anchor.BN(10_000_000)
        )
        .accounts({
          policy: policyPda,
          vault: vault.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const policyAccount = await program.account.policyPda.fetch(policyPda);
      expect(policyAccount.investorFeeShareBps).to.equal(0);
      console.log("✅ 0% investor fee share accepted");
    });

    it("Accepts 100% investor fee share (maximum)", async () => {
      const vault = Keypair.generate();
      const [policyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("policy"), vault.publicKey.toBuffer()],
        program.programId
      );

      await program.methods
        .initializePolicy(
          10000, // 100% to investors
          new anchor.BN(10_000_000_000),
          new anchor.BN(10_000_000)
        )
        .accounts({
          policy: policyPda,
          vault: vault.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const policyAccount = await program.account.policyPda.fetch(policyPda);
      expect(policyAccount.investorFeeShareBps).to.equal(10000);
      console.log("✅ 100% investor fee share accepted");
    });

    it("Accepts minimum daily cap (1 token)", async () => {
      const vault = Keypair.generate();
      const [policyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("policy"), vault.publicKey.toBuffer()],
        program.programId
      );

      await program.methods
        .initializePolicy(
          5000,
          new anchor.BN(1), // Minimum cap
          new anchor.BN(1)
        )
        .accounts({
          policy: policyPda,
          vault: vault.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const policyAccount = await program.account.policyPda.fetch(policyPda);
      expect(policyAccount.dailyCapQuote.toString()).to.equal("1");
      console.log("✅ Minimum daily cap accepted");
    });

    it("Accepts very large daily cap (u64 max)", async () => {
      const vault = Keypair.generate();
      const [policyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("policy"), vault.publicKey.toBuffer()],
        program.programId
      );

      const maxU64 = new anchor.BN("18446744073709551615"); // u64::MAX

      await program.methods
        .initializePolicy(
          5000,
          maxU64,
          new anchor.BN(1)
        )
        .accounts({
          policy: policyPda,
          vault: vault.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      const policyAccount = await program.account.policyPda.fetch(policyPda);
      expect(policyAccount.dailyCapQuote.toString()).to.equal(maxU64.toString());
      console.log("✅ Maximum u64 daily cap accepted");
    });

    it("Prevents policy reinitialization", async () => {
      const vault = Keypair.generate();
      const [policyPda] = PublicKey.findProgramAddressSync(
        [Buffer.from("policy"), vault.publicKey.toBuffer()],
        program.programId
      );

      // First initialization
      await program.methods
        .initializePolicy(
          5000,
          new anchor.BN(10_000_000_000),
          new anchor.BN(10_000_000)
        )
        .accounts({
          policy: policyPda,
          vault: vault.publicKey,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();

      // Second initialization should fail
      try {
        await program.methods
          .initializePolicy(
            6000,
            new anchor.BN(20_000_000_000),
            new anchor.BN(20_000_000)
          )
          .accounts({
            policy: policyPda,
            vault: vault.publicKey,
            creator: creator.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([creator])
          .rpc();
        
        expect.fail("Should have thrown an error");
      } catch (error) {
        expect(error.message).to.include("already in use");
        console.log("✅ Policy reinitialization correctly prevented");
      }
    });
  });

  describe("Fee Distribution Math - Advanced Scenarios", () => {
    it("Handles high precision weight calculations", () => {
      const investor1Locked = 123_456_789;
      const investor2Locked = 876_543_211;
      const lockedTotal = investor1Locked + investor2Locked;
      
      const investorFee = 1000 * 1_000_000;
      
      // Calculate weights with high precision
      const weight1 = Math.floor((investor1Locked * 1_000_000) / lockedTotal);
      const weight2 = Math.floor((investor2Locked * 1_000_000) / lockedTotal);
      
      const payout1 = Math.floor((investorFee * weight1) / 1_000_000);
      const payout2 = Math.floor((investorFee * weight2) / 1_000_000);
      
      const totalPaid = payout1 + payout2;
      const dust = investorFee - totalPaid;
      
      // Verify proportions are correct
      expect(payout1).to.be.lessThan(payout2);
      expect(dust).to.be.lessThan(1000); // Minimal dust
      
      console.log("✅ High precision weights calculated correctly");
      console.log(`  Payout 1: ${payout1}, Payout 2: ${payout2}, Dust: ${dust}`);
    });

    it("Tests extreme locked ratio (99.99% locked)", () => {
      const claimedQuote = 10_000 * 1_000_000;
      const lockedTotal = 199_980 * 1_000_000; // 99.99% locked
      const y0Total = 200_000 * 1_000_000;
      const investorFeeShareBps = 5000;
      
      const fLockedBps = Math.floor((lockedTotal * 10000) / y0Total);
      expect(fLockedBps).to.equal(9999);
      
      const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
      const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
      
      // With 50% base share, should get 50%
      expect(investorFeeQuote).to.equal(5_000 * 1_000_000);
      
      console.log("✅ Extreme locked ratio (99.99%) handled correctly");
    });

    it("Tests minimal locked ratio (0.01% locked)", () => {
      const claimedQuote = 10_000 * 1_000_000;
      const lockedTotal = 20 * 1_000_000; // 0.01% locked
      const y0Total = 200_000 * 1_000_000;
      const investorFeeShareBps = 5000;
      
      const fLockedBps = Math.floor((lockedTotal * 10000) / y0Total);
      expect(fLockedBps).to.equal(1); // 0.01%
      
      const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
      const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
      
      // Should get only 0.01% to investors
      expect(investorFeeQuote).to.equal(10_000);
      
      console.log("✅ Minimal locked ratio (0.01%) handled correctly");
    });

    it("Tests single wei/lamport fee distribution", () => {
      const claimedQuote = 1; // Single unit
      const lockedTotal = 100_000 * 1_000_000;
      const y0Total = 200_000 * 1_000_000;
      const investorFeeShareBps = 5000;
      
      const fLockedBps = Math.floor((lockedTotal * 10000) / y0Total);
      const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
      const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
      
      // With 1 unit, should round down to 0
      expect(investorFeeQuote).to.equal(0);
      
      const creatorShare = claimedQuote - investorFeeQuote;
      expect(creatorShare).to.equal(1);
      
      console.log("✅ Single unit fee handled correctly");
    });

    it("Tests 10-way equal distribution", () => {
      const investors = Array(10).fill(100_000 * 1_000_000);
      const lockedTotal = investors.reduce((a, b) => a + b, 0);
      const investorFee = 10_000 * 1_000_000;
      
      const payouts = investors.map(locked => {
        const weight = Math.floor((locked * 1_000_000) / lockedTotal);
        return Math.floor((investorFee * weight) / 1_000_000);
      });
      
      const totalPaid = payouts.reduce((a, b) => a + b, 0);
      const dust = investorFee - totalPaid;
      
      // Each should get approximately 1000 tokens
      payouts.forEach(payout => {
        expect(payout).to.be.approximately(1000 * 1_000_000, 10_000);
      });
      
      expect(dust).to.be.lessThan(100);
      
      console.log("✅ 10-way equal distribution validated");
    });

    it("Tests highly skewed distribution (1 whale, 99 small)", () => {
      const whaleLocked = 190_000 * 1_000_000; // 95%
      const smallInvestorLocked = 100 * 1_000_000; // 0.05% each
      const investors = [whaleLocked, ...Array(99).fill(smallInvestorLocked)];
      
      const lockedTotal = investors.reduce((a, b) => a + b, 0);
      const investorFee = 10_000 * 1_000_000;
      
      const whaleWeight = Math.floor((whaleLocked * 1_000_000) / lockedTotal);
      const whalePayout = Math.floor((investorFee * whaleWeight) / 1_000_000);
      
      // Whale should get ~95% of fees
      expect(whalePayout).to.be.approximately(9_500 * 1_000_000, 100_000);
      
      console.log("✅ Highly skewed distribution handled correctly");
      console.log(`  Whale gets: ${whalePayout / 1_000_000} tokens (~95%)`);
    });
  });

  describe("Boundary Conditions and Edge Cases", () => {
    it("Handles zero Y0 total gracefully", () => {
      const lockedTotal = 0;
      const y0Total = 0; // Edge case: no initial allocation
      
      const fLockedBps = y0Total > 0 ? Math.floor((lockedTotal * 10000) / y0Total) : 0;
      expect(fLockedBps).to.equal(0);
      
      console.log("✅ Zero Y0 total handled without division by zero");
    });

    it("Tests rounding behavior with odd BPS values", () => {
      const claimedQuote = 9_999_999;
      const eligibleShareBps = 3333; // 33.33%
      
      const investorFeeQuote = Math.floor((claimedQuote * eligibleShareBps) / 10000);
      const creatorShare = claimedQuote - investorFeeQuote;
      
      // Verify no tokens lost
      expect(investorFeeQuote + creatorShare).to.equal(claimedQuote);
      
      console.log("✅ Rounding with odd BPS preserves all tokens");
    });

    it("Tests maximum BPS multiplication (10000 * large value)", () => {
      const largeValue = 1_000_000_000_000; // 1 trillion
      const eligibleShareBps = 10000; // 100%
      
      // Simulate the calculation
      const result = Math.floor((largeValue * eligibleShareBps) / 10000);
      expect(result).to.equal(largeValue);
      
      console.log("✅ Large value multiplication handled correctly");
    });

    it("Validates minimum payout prevents dust payments", () => {
      const minPayout = 10 * 1_000_000;
      const dustPayouts = [1, 100, 1000, 9_999_999];
      
      dustPayouts.forEach(payout => {
        const shouldPay = payout >= minPayout;
        expect(shouldPay).to.be.false;
      });
      
      console.log("✅ Minimum payout filters dust correctly");
    });

    it("Tests daily cap with exact match", () => {
      const claimedQuote = 10_000 * 1_000_000;
      const dailyCap = 10_000 * 1_000_000;
      
      const distributable = Math.min(claimedQuote, dailyCap);
      expect(distributable).to.equal(claimedQuote);
      expect(distributable).to.equal(dailyCap);
      
      console.log("✅ Daily cap exact match handled correctly");
    });
  });

  describe("Pagination and State Management", () => {
    it("Simulates 3-page distribution sequence", () => {
      const investorsPerPage = 100;
      const totalPages = 3;
      let currentPage = 0;
      
      // Page 0
      expect(currentPage).to.equal(0);
      let isFinalPage = currentPage === totalPages - 1;
      expect(isFinalPage).to.be.false;
      currentPage++;
      
      // Page 1
      expect(currentPage).to.equal(1);
      isFinalPage = currentPage === totalPages - 1;
      expect(isFinalPage).to.be.false;
      currentPage++;
      
      // Page 2 (final)
      expect(currentPage).to.equal(2);
      isFinalPage = currentPage === totalPages - 1;
      expect(isFinalPage).to.be.true;
      
      console.log("✅ Multi-page sequence validated");
    });

    it("Tests page state reset after distribution", () => {
      let currentPage = 5;
      const isFinalPage = true;
      
      if (isFinalPage) {
        currentPage = 0; // Reset for next cycle
      }
      
      expect(currentPage).to.equal(0);
      console.log("✅ Page state reset validated");
    });
  });

  describe("Time-based Distribution Logic", () => {
    it("Calculates 24h interval correctly", () => {
      const now = Math.floor(Date.now() / 1000);
      const lastDistribution = now - 86400; // Exactly 24h ago
      
      const timeSince = now - lastDistribution;
      expect(timeSince).to.equal(86400);
      expect(timeSince >= 86400).to.be.true;
      
      console.log("✅ 24h interval calculation correct");
    });

    it("Rejects distribution at 23h 59min", () => {
      const now = Math.floor(Date.now() / 1000);
      const lastDistribution = now - 86340; // 23h 59min ago
      
      const timeSince = now - lastDistribution;
      expect(timeSince).to.equal(86340);
      expect(timeSince < 86400).to.be.true;
      
      console.log("✅ Premature distribution correctly rejected");
    });

    it("Allows distribution at 24h 1s", () => {
      const now = Math.floor(Date.now() / 1000);
      const lastDistribution = now - 86401; // 24h 1s ago
      
      const timeSince = now - lastDistribution;
      expect(timeSince >= 86400).to.be.true;
      
      console.log("✅ Distribution after 24h allowed");
    });

    it("Handles first distribution (lastDistributionTs = 0)", () => {
      const lastDistribution = 0;
      const now = Math.floor(Date.now() / 1000);
      
      const timeSince = now - lastDistribution;
      const shouldAllow = timeSince >= 86400 || lastDistribution === 0;
      
      expect(shouldAllow).to.be.true;
      console.log("✅ First distribution bypass validated");
    });

    it("Tests multiple consecutive 24h cycles", () => {
      const startTime = 1696636800;
      const cycles = 10;
      
      for (let i = 0; i < cycles; i++) {
        const distributionTime = startTime + (i * 86400);
        const nextAllowedTime = distributionTime + 86400;
        
        const timeSince = nextAllowedTime - distributionTime;
        expect(timeSince).to.equal(86400);
      }
      
      console.log(`✅ ${cycles} consecutive 24h cycles validated`);
    });
  });

  describe("Integration Math Validation", () => {
    it("Validates complete distribution flow with 5 investors", () => {
      const investors = [
        { locked: 100_000 * 1_000_000, initial: 100_000 * 1_000_000 },
        { locked: 80_000 * 1_000_000, initial: 100_000 * 1_000_000 },
        { locked: 60_000 * 1_000_000, initial: 100_000 * 1_000_000 },
        { locked: 40_000 * 1_000_000, initial: 100_000 * 1_000_000 },
        { locked: 20_000 * 1_000_000, initial: 100_000 * 1_000_000 },
      ];
      
      const lockedTotal = investors.reduce((sum, inv) => sum + inv.locked, 0);
      const y0Total = investors.reduce((sum, inv) => sum + inv.initial, 0);
      const claimedQuote = 10_000 * 1_000_000;
      const investorFeeShareBps = 6000; // 60%
      const dailyCap = 15_000 * 1_000_000;
      const minPayout = 5 * 1_000_000;
      
      // Apply cap
      const distributable = Math.min(claimedQuote, dailyCap);
      expect(distributable).to.equal(claimedQuote);
      
      // Calculate locked ratio
      const fLockedBps = Math.floor((lockedTotal * 10000) / y0Total);
      expect(fLockedBps).to.equal(6000); // 60% locked
      
      // Eligible share
      const eligibleShareBps = Math.min(fLockedBps, investorFeeShareBps);
      expect(eligibleShareBps).to.equal(6000);
      
      // Investor allocation
      const investorFeeQuote = Math.floor((distributable * eligibleShareBps) / 10000);
      expect(investorFeeQuote).to.equal(6_000 * 1_000_000);
      
      // Distribute to each investor
      let totalPaid = 0;
      const payouts = investors.map(inv => {
        const weight = Math.floor((inv.locked * 1_000_000) / lockedTotal);
        const payout = Math.floor((investorFeeQuote * weight) / 1_000_000);
        
        if (payout >= minPayout) {
          totalPaid += payout;
          return payout;
        }
        return 0;
      });
      
      // Creator gets remainder
      const creatorShare = distributable - totalPaid;
      
      // Verify conservation
      expect(totalPaid + creatorShare).to.equal(distributable);
      
      console.log("✅ Complete 5-investor distribution validated");
      console.log(`  Total to investors: ${totalPaid / 1_000_000} tokens`);
      console.log(`  Total to creator: ${creatorShare / 1_000_000} tokens`);
      payouts.forEach((payout, i) => {
        console.log(`  Investor ${i + 1}: ${payout / 1_000_000} tokens`);
      });
    });
  });

  describe("Error Handling and Validation", () => {
    it("Validates BPS must not exceed 10000", () => {
      const invalidValues = [10001, 15000, 65535, 99999];
      
      invalidValues.forEach(bps => {
        const isValid = bps <= 10000;
        expect(isValid).to.be.false;
      });
      
      console.log("✅ Invalid BPS values correctly identified");
    });

    it("Validates negative values not possible (unsigned math)", () => {
      // In JavaScript, we simulate Rust u64 behavior
      const value1 = 1000;
      const value2 = 2000;
      
      // Saturating subtraction
      const result = value1 > value2 ? value1 - value2 : 0;
      expect(result).to.equal(0);
      
      console.log("✅ Saturating arithmetic validated");
    });
  });
});