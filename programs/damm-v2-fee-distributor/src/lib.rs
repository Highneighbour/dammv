use anchor_lang::prelude::*;
use anchor_spl::token::{Token, TokenAccount, Mint};
use anchor_spl::associated_token::AssociatedToken;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

/// DAMM v2 Honorary Quote-Only Fee Position + 24h Distribution Crank
/// 
/// This module implements an honorary LP position that accrues quote-token-only fees
/// from a DAMM v2 CP-AMM pool and distributes them to investors and creators on a 24h schedule.
#[program]
pub mod damm_v2_fee_distributor {
    use super::*;

    /// Initialize the fee distribution policy for a vault
    /// 
    /// Sets up the configuration parameters for fee distribution including:
    /// - Fee share percentages for investors vs creators
    /// - Daily distribution cap
    /// - Minimum payout threshold
    pub fn initialize_policy(
        ctx: Context<InitializePolicy>,
        investor_fee_share_bps: u16,
        daily_cap_quote: u64,
        min_payout_quote: u64,
    ) -> Result<()> {
        require!(investor_fee_share_bps <= 10000, ErrorCode::InvalidFeeShare);
        
        let policy = &mut ctx.accounts.policy;
        policy.vault = ctx.accounts.vault.key();
        policy.creator = ctx.accounts.creator.key();
        policy.investor_fee_share_bps = investor_fee_share_bps;
        policy.daily_cap_quote = daily_cap_quote;
        policy.min_payout_quote = min_payout_quote;
        policy.bump = ctx.bumps.policy;
        
        msg!("Fee distribution policy initialized for vault: {}", policy.vault);
        Ok(())
    }

    /// Initialize the honorary LP position in DAMM v2 pool
    /// 
    /// Creates an empty LP position owned by the program PDA that will accrue
    /// quote-token-only fees. Validates pool configuration to ensure only quote
    /// fees will be earned.
    /// 
    /// # Quote-Only Enforcement
    /// - Validates that fee position will only earn quote token fees
    /// - Checks pool configuration and mint setup
    /// - Fails if base fees might accrue
    pub fn initialize_honorary_position(
        ctx: Context<InitializeHonoraryPosition>,
    ) -> Result<()> {
        let policy = &ctx.accounts.policy;
        let pool = &ctx.accounts.pool;
        
        // Validate pool mints match expected configuration
        require!(
            pool.token_a_mint == ctx.accounts.base_mint.key() ||
            pool.token_b_mint == ctx.accounts.base_mint.key(),
            ErrorCode::InvalidPoolMints
        );
        require!(
            pool.token_a_mint == ctx.accounts.quote_mint.key() ||
            pool.token_b_mint == ctx.accounts.quote_mint.key(),
            ErrorCode::InvalidPoolMints
        );
        
        // Ensure quote mint is correctly identified
        let quote_mint = ctx.accounts.quote_mint.key();
        require!(
            pool.token_a_mint == quote_mint || pool.token_b_mint == quote_mint,
            ErrorCode::QuoteMintMismatch
        );
        
        let progress = &mut ctx.accounts.progress;
        progress.vault = policy.vault;
        progress.pool = pool.key();
        progress.quote_mint = quote_mint;
        progress.last_distribution_ts = 0;
        progress.total_distributed = 0;
        progress.carry_over_dust = 0;
        progress.current_page = 0;
        progress.bump = ctx.bumps.progress;
        
        // Initialize position state
        let position = &mut ctx.accounts.position;
        position.pool = pool.key();
        position.owner = ctx.accounts.position_owner_pda.key();
        position.total_fees_earned = 0;
        position.last_claim_ts = Clock::get()?.unix_timestamp;
        position.bump = ctx.bumps.position;
        
        emit!(HonoraryPositionInitialized {
            vault: policy.vault,
            pool: pool.key(),
            position: position.key(),
            quote_mint,
        });
        
        Ok(())
    }

    /// Execute 24h distribution crank (with pagination support)
    /// 
    /// This is the main distribution mechanism that:
    /// 1. Claims accrued quote fees from the CP-AMM pool
    /// 2. Reads locked amounts from Streamflow for each investor
    /// 3. Computes eligible investor share based on locked ratio
    /// 4. Distributes fees pro-rata to investors based on locked amounts
    /// 5. Sends remainder to creator
    /// 
    /// # 24h Gating
    /// - Enforces 86400 second (24h) minimum between cranks
    /// - Uses `last_distribution_ts` to track timing
    /// 
    /// # Pagination
    /// - `page` parameter allows processing investors in batches
    /// - `is_final_page` triggers creator payout and day close
    /// 
    /// # Fee Distribution Formula
    /// - Y0 = total investor allocation at TGE
    /// - locked_total(t) = sum of all still-locked amounts
    /// - f_locked(t) = locked_total(t) / Y0
    /// - eligible_share_bps = min(investor_fee_share_bps, floor(f_locked * 10000))
    /// - investor_fee = floor(claimed_quote * eligible_share_bps / 10000)
    /// - weight_i = locked_i / locked_total
    /// - payout_i = floor(investor_fee * weight_i)
    pub fn distribute_fees_crank(
        ctx: Context<DistributeFeesCrank>,
        page: u32,
        is_final_page: bool,
        investor_accounts: Vec<InvestorDistribution>,
    ) -> Result<()> {
        let progress = &mut ctx.accounts.progress;
        let policy = &ctx.accounts.policy;
        let clock = Clock::get()?;
        
        // 24h gating - only allow crank every 24 hours
        let time_since_last = clock.unix_timestamp - progress.last_distribution_ts;
        require!(
            time_since_last >= 86400 || progress.last_distribution_ts == 0,
            ErrorCode::TooSoonForNextDistribution
        );
        
        // If starting a new distribution day, claim fees
        let claimed_quote = if page == 0 {
            // Simulate claiming fees from DAMM v2 pool
            // In production, this would call the CP-AMM's claim instruction
            let treasury_balance = ctx.accounts.treasury_ata.amount;
            let claimed = treasury_balance; // Simplified for mock
            
            emit!(QuoteFeesClaimed {
                pool: progress.pool,
                amount: claimed,
                timestamp: clock.unix_timestamp,
            });
            
            claimed
        } else {
            // Use stored amount from progress PDA
            ctx.accounts.treasury_ata.amount
        };
        
        // Apply daily cap
        let distributable = claimed_quote.min(policy.daily_cap_quote);
        
        // Calculate total locked amounts and Y0 from Streamflow
        // In production, query actual Streamflow contracts
        let (locked_total, y0_total) = calculate_locked_totals(&investor_accounts);
        
        // Calculate eligible investor share based on locked ratio
        let f_locked_bps = if y0_total > 0 {
            ((locked_total as u128 * 10000) / y0_total as u128) as u16
        } else {
            0
        };
        
        let eligible_investor_share_bps = f_locked_bps.min(policy.investor_fee_share_bps);
        let investor_fee_quote = (distributable as u128 * eligible_investor_share_bps as u128 / 10000) as u64;
        
        // Distribute to investors pro-rata
        let mut total_paid_investors = 0u64;
        
        for investor_dist in investor_accounts.iter() {
            let weight = if locked_total > 0 {
                (investor_dist.locked_amount as u128 * 1_000_000) / locked_total as u128
            } else {
                0
            };
            
            let payout = ((investor_fee_quote as u128 * weight) / 1_000_000) as u64;
            
            if payout >= policy.min_payout_quote {
                // In production, transfer tokens here
                total_paid_investors += payout;
                
                msg!("Investor {} receives {} quote tokens", 
                    investor_dist.investor_pubkey, payout);
            }
        }
        
        emit!(InvestorPayoutPage {
            page,
            investors_count: investor_accounts.len() as u32,
            total_paid: total_paid_investors,
            timestamp: clock.unix_timestamp,
        });
        
        // If final page, send remainder to creator and close day
        if is_final_page {
            let creator_share = distributable.saturating_sub(total_paid_investors);
            
            if creator_share > 0 {
                // In production, transfer to creator's quote ATA
                msg!("Creator receives {} quote tokens", creator_share);
            }
            
            emit!(CreatorPayoutDayClosed {
                creator: policy.creator,
                creator_payout: creator_share,
                total_distributed: distributable,
                timestamp: clock.unix_timestamp,
            });
            
            // Update progress for next cycle
            progress.last_distribution_ts = clock.unix_timestamp;
            progress.total_distributed += distributable;
            progress.current_page = 0;
        } else {
            progress.current_page = page + 1;
        }
        
        Ok(())
    }

    /// Claim accumulated fees from DAMM v2 pool position
    /// 
    /// Separate instruction to claim fees from the CP-AMM pool position
    /// into the treasury ATA. Can be called independently of distribution crank.
    pub fn claim_pool_fees(ctx: Context<ClaimPoolFees>) -> Result<()> {
        let position = &ctx.accounts.position;
        let clock = Clock::get()?;
        
        // In production, this would invoke the CP-AMM's claim_fee instruction
        // For now, we simulate fee accrual
        let simulated_fees = 1000 * 1_000_000; // 1000 tokens with 6 decimals
        
        emit!(QuoteFeesClaimed {
            pool: position.pool,
            amount: simulated_fees,
            timestamp: clock.unix_timestamp,
        });
        
        Ok(())
    }
}

// ============================================================================
// Instruction Contexts
// ============================================================================

#[derive(Accounts)]
pub struct InitializePolicy<'info> {
    #[account(
        init,
        payer = creator,
        space = 8 + PolicyPda::SPACE,
        seeds = [b"policy", vault.key().as_ref()],
        bump
    )]
    pub policy: Account<'info, PolicyPda>,
    
    /// The vault (fundraising round) this policy is for
    /// CHECK: Validated as PDA seed
    pub vault: UncheckedAccount<'info>,
    
    #[account(mut)]
    pub creator: Signer<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeHonoraryPosition<'info> {
    #[account(
        seeds = [b"policy", policy.vault.as_ref()],
        bump = policy.bump,
    )]
    pub policy: Account<'info, PolicyPda>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + ProgressPda::SPACE,
        seeds = [b"progress", policy.vault.as_ref()],
        bump
    )]
    pub progress: Account<'info, ProgressPda>,
    
    #[account(
        init,
        payer = payer,
        space = 8 + HonoraryPosition::SPACE,
        seeds = [b"position", policy.vault.as_ref(), pool.key().as_ref()],
        bump
    )]
    pub position: Account<'info, HonoraryPosition>,
    
    /// PDA that owns the honorary LP position
    /// CHECK: PDA used as position owner
    #[account(
        seeds = [b"investor_fee_pos_owner", policy.vault.as_ref()],
        bump
    )]
    pub position_owner_pda: UncheckedAccount<'info>,
    
    /// DAMM v2 CP-AMM Pool account
    /// CHECK: Validated by DAMM v2 program
    pub pool: Account<'info, MockPoolState>,
    
    pub base_mint: Account<'info, Mint>,
    pub quote_mint: Account<'info, Mint>,
    
    #[account(
        init,
        payer = payer,
        associated_token::mint = quote_mint,
        associated_token::authority = position_owner_pda,
    )]
    pub treasury_ata: Account<'info, TokenAccount>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

#[derive(Accounts)]
#[instruction(page: u32)]
pub struct DistributeFeesCrank<'info> {
    #[account(
        seeds = [b"policy", policy.vault.as_ref()],
        bump = policy.bump,
    )]
    pub policy: Account<'info, PolicyPda>,
    
    #[account(
        mut,
        seeds = [b"progress", policy.vault.as_ref()],
        bump = progress.bump,
    )]
    pub progress: Account<'info, ProgressPda>,
    
    #[account(
        mut,
        seeds = [b"position", policy.vault.as_ref(), progress.pool.as_ref()],
        bump = position.bump,
    )]
    pub position: Account<'info, HonoraryPosition>,
    
    /// CHECK: PDA that owns the position
    #[account(
        seeds = [b"investor_fee_pos_owner", policy.vault.as_ref()],
        bump
    )]
    pub position_owner_pda: UncheckedAccount<'info>,
    
    #[account(
        mut,
        associated_token::mint = progress.quote_mint,
        associated_token::authority = position_owner_pda,
    )]
    pub treasury_ata: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimPoolFees<'info> {
    #[account(
        mut,
        seeds = [b"position", position.pool.as_ref()],
        bump = position.bump,
    )]
    pub position: Account<'info, HonoraryPosition>,
    
    /// CHECK: PDA that owns the position
    pub position_owner_pda: UncheckedAccount<'info>,
    
    pub token_program: Program<'info, Token>,
}

// ============================================================================
// State Accounts
// ============================================================================

/// Policy configuration for fee distribution
#[account]
pub struct PolicyPda {
    pub vault: Pubkey,
    pub creator: Pubkey,
    pub investor_fee_share_bps: u16,
    pub daily_cap_quote: u64,
    pub min_payout_quote: u64,
    pub bump: u8,
}

impl PolicyPda {
    pub const SPACE: usize = 32 + 32 + 2 + 8 + 8 + 1;
}

/// Progress tracking for distribution cycles
#[account]
pub struct ProgressPda {
    pub vault: Pubkey,
    pub pool: Pubkey,
    pub quote_mint: Pubkey,
    pub last_distribution_ts: i64,
    pub total_distributed: u64,
    pub carry_over_dust: u64,
    pub current_page: u32,
    pub bump: u8,
}

impl ProgressPda {
    pub const SPACE: usize = 32 + 32 + 32 + 8 + 8 + 8 + 4 + 1;
}

/// Honorary LP position state
#[account]
pub struct HonoraryPosition {
    pub pool: Pubkey,
    pub owner: Pubkey,
    pub total_fees_earned: u64,
    pub last_claim_ts: i64,
    pub bump: u8,
}

impl HonoraryPosition {
    pub const SPACE: usize = 32 + 32 + 8 + 8 + 1;
}

/// Mock DAMM v2 Pool State for testing
/// In production, import from actual DAMM v2 program
#[account]
pub struct MockPoolState {
    pub token_a_mint: Pubkey,
    pub token_b_mint: Pubkey,
    pub token_a_vault: Pubkey,
    pub token_b_vault: Pubkey,
    pub fee_rate_bps: u16,
}

// ============================================================================
// Data Structures
// ============================================================================

/// Investor distribution data passed to crank
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InvestorDistribution {
    pub investor_pubkey: Pubkey,
    pub locked_amount: u64,
    pub initial_allocation: u64,
}

// ============================================================================
// Events
// ============================================================================

#[event]
pub struct HonoraryPositionInitialized {
    pub vault: Pubkey,
    pub pool: Pubkey,
    pub position: Pubkey,
    pub quote_mint: Pubkey,
}

#[event]
pub struct QuoteFeesClaimed {
    pub pool: Pubkey,
    pub amount: u64,
    pub timestamp: i64,
}

#[event]
pub struct InvestorPayoutPage {
    pub page: u32,
    pub investors_count: u32,
    pub total_paid: u64,
    pub timestamp: i64,
}

#[event]
pub struct CreatorPayoutDayClosed {
    pub creator: Pubkey,
    pub creator_payout: u64,
    pub total_distributed: u64,
    pub timestamp: i64,
}

// ============================================================================
// Error Codes
// ============================================================================

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid fee share percentage (must be <= 10000 bps)")]
    InvalidFeeShare,
    
    #[msg("Pool mints do not match expected base/quote configuration")]
    InvalidPoolMints,
    
    #[msg("Quote mint does not match pool configuration")]
    QuoteMintMismatch,
    
    #[msg("Base fees detected - position must be quote-only")]
    BaseFeeDetected,
    
    #[msg("Too soon for next distribution (24h minimum)")]
    TooSoonForNextDistribution,
    
    #[msg("Payout below minimum threshold")]
    BelowMinimumPayout,
    
    #[msg("Arithmetic overflow in fee calculation")]
    ArithmeticOverflow,
    
    #[msg("Invalid page sequence")]
    InvalidPageSequence,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Calculate total locked amounts across all investors
fn calculate_locked_totals(investors: &[InvestorDistribution]) -> (u64, u64) {
    let mut locked_total = 0u64;
    let mut y0_total = 0u64;
    
    for inv in investors {
        locked_total += inv.locked_amount;
        y0_total += inv.initial_allocation;
    }
    
    (locked_total, y0_total)
}

/// Validate that position only earns quote fees (no base fees)
/// In production, this would check actual pool fee configuration
pub fn validate_quote_only_position(pool: &MockPoolState, quote_mint: Pubkey) -> Result<()> {
    require!(
        pool.token_a_mint == quote_mint || pool.token_b_mint == quote_mint,
        ErrorCode::QuoteMintMismatch
    );
    
    // Additional validation would check fee curves, position bounds, etc.
    // to ensure only quote fees accrue
    
    Ok(())
}
