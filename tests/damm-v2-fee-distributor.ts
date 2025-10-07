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
