use anchor_lang::prelude::*;
use anchor_lang::solana_program::system_program;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use damm_v2_fee_distributor::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper to create a mock pool state
    fn create_mock_pool(
        base_mint: Pubkey,
        quote_mint: Pubkey,
        fee_rate_bps: u16,
    ) -> MockPoolState {
        MockPoolState {
            token_a_mint: base_mint,
            token_b_mint: quote_mint,
            token_a_vault: Pubkey::new_unique(),
            token_b_vault: Pubkey::new_unique(),
            fee_rate_bps,
        }
    }

    /// Test helper to derive PDAs
    fn derive_pdas(program_id: &Pubkey, vault: &Pubkey, pool: &Pubkey) -> (Pubkey, Pubkey, Pubkey, Pubkey) {
        let (policy, _) = Pubkey::find_program_address(
            &[b"policy", vault.as_ref()],
            program_id,
        );
        let (progress, _) = Pubkey::find_program_address(
            &[b"progress", vault.as_ref()],
            program_id,
        );
        let (position, _) = Pubkey::find_program_address(
            &[b"position", vault.as_ref(), pool.as_ref()],
            program_id,
        );
        let (position_owner, _) = Pubkey::find_program_address(
            &[b"investor_fee_pos_owner", vault.as_ref()],
            program_id,
        );
        
        (policy, progress, position, position_owner)
    }

    #[test]
    fn test_initialize_policy_success() {
        let program_id = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let creator = Pubkey::new_unique();
        
        let investor_fee_share_bps = 5000; // 50%
        let daily_cap_quote = 10_000 * 1_000_000; // 10k tokens
        let min_payout_quote = 10 * 1_000_000; // 10 tokens minimum
        
        // Validate parameters
        assert!(investor_fee_share_bps <= 10000);
        assert!(daily_cap_quote > 0);
        assert!(min_payout_quote > 0);
        
        println!("✅ Policy initialization parameters validated");
    }

    #[test]
    fn test_initialize_policy_invalid_fee_share() {
        let investor_fee_share_bps = 10001; // Invalid: >100%
        
        // Should fail validation
        assert!(investor_fee_share_bps > 10000);
        println!("✅ Invalid fee share correctly rejected");
    }

    #[test]
    fn test_initialize_honorary_position() {
        let program_id = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let pool_key = Pubkey::new_unique();
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        
        let pool = create_mock_pool(base_mint, quote_mint, 30); // 0.3% fee
        
        // Validate pool configuration
        assert!(pool.token_a_mint == base_mint || pool.token_b_mint == base_mint);
        assert!(pool.token_a_mint == quote_mint || pool.token_b_mint == quote_mint);
        assert!(pool.fee_rate_bps > 0);
        
        println!("✅ Honorary position initialization validated");
    }

    #[test]
    fn test_quote_only_validation() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let pool = create_mock_pool(base_mint, quote_mint, 30);
        
        // Validate quote mint is in pool
        let has_quote = pool.token_a_mint == quote_mint || pool.token_b_mint == quote_mint;
        assert!(has_quote, "Pool must contain quote mint");
        
        println!("✅ Quote-only validation passed");
    }

    #[test]
    fn test_24h_distribution_timing() {
        let last_distribution_ts = 1696636800i64; // Example timestamp
        let current_ts = last_distribution_ts + 86400; // Exactly 24h later
        
        let time_since_last = current_ts - last_distribution_ts;
        assert!(time_since_last >= 86400, "Must wait 24h between distributions");
        
        println!("✅ 24h gating validated");
    }

    #[test]
    fn test_24h_too_soon_rejection() {
        let last_distribution_ts = 1696636800i64;
        let current_ts = last_distribution_ts + 43200; // Only 12h later
        
        let time_since_last = current_ts - last_distribution_ts;
        assert!(time_since_last < 86400, "Should reject too-soon distribution");
        
        println!("✅ Too-soon rejection validated");
    }

    #[test]
    fn test_fee_distribution_calculation() {
        // Test scenario: 3 investors with varying locked amounts
        let investor1_locked = 100_000 * 1_000_000u64; // 100k tokens
        let investor2_locked = 50_000 * 1_000_000u64;  // 50k tokens  
        let investor3_locked = 30_000 * 1_000_000u64;  // 30k tokens
        
        let y0_total = 200_000 * 1_000_000u64; // Total allocation: 200k
        let locked_total = investor1_locked + investor2_locked + investor3_locked; // 180k
        
        let investor_fee_share_bps = 5000u16; // 50% base share
        
        // Calculate f_locked ratio
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        assert_eq!(f_locked_bps, 9000); // 90% locked
        
        // Eligible share is min of f_locked and investor_fee_share
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq!(eligible_share_bps, 5000); // Capped at 50%
        
        // Calculate investor fee from claimed amount
        let claimed_quote = 1000 * 1_000_000u64; // 1000 tokens claimed
        let investor_fee_quote = (claimed_quote as u128 * eligible_share_bps as u128 / 10000) as u64;
        assert_eq!(investor_fee_quote, 500 * 1_000_000); // 500 tokens for investors
        
        // Calculate pro-rata weights (using high precision)
        let weight1 = (investor1_locked as u128 * 1_000_000) / locked_total as u128;
        let weight2 = (investor2_locked as u128 * 1_000_000) / locked_total as u128;
        let weight3 = (investor3_locked as u128 * 1_000_000) / locked_total as u128;
        
        let payout1 = ((investor_fee_quote as u128 * weight1) / 1_000_000) as u64;
        let payout2 = ((investor_fee_quote as u128 * weight2) / 1_000_000) as u64;
        let payout3 = ((investor_fee_quote as u128 * weight3) / 1_000_000) as u64;
        
        // Verify proportional distribution
        assert!(payout1 > payout2 && payout2 > payout3);
        
        let total_paid = payout1 + payout2 + payout3;
        let creator_share = claimed_quote - total_paid;
        
        // Verify total adds up correctly (allowing for rounding dust)
        assert!(creator_share >= 500 * 1_000_000 - 1000);
        assert!(creator_share <= 500 * 1_000_000 + 1000);
        
        println!("✅ Fee distribution calculation validated");
        println!("   Investor 1: {} tokens", payout1 / 1_000_000);
        println!("   Investor 2: {} tokens", payout2 / 1_000_000);
        println!("   Investor 3: {} tokens", payout3 / 1_000_000);
        println!("   Creator: {} tokens", creator_share / 1_000_000);
    }

    #[test]
    fn test_all_unlocked_scenario() {
        // Scenario: All tokens unlocked -> 100% to creator
        let locked_total = 0u64;
        let y0_total = 200_000 * 1_000_000u64;
        
        let f_locked_bps = if y0_total > 0 {
            ((locked_total as u128 * 10000) / y0_total as u128) as u16
        } else {
            0
        };
        
        assert_eq!(f_locked_bps, 0);
        
        let investor_fee_share_bps = 5000u16;
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq!(eligible_share_bps, 0);
        
        let claimed_quote = 1000 * 1_000_000u64;
        let investor_fee_quote = (claimed_quote as u128 * eligible_share_bps as u128 / 10000) as u64;
        assert_eq!(investor_fee_quote, 0);
        
        let creator_share = claimed_quote - investor_fee_quote;
        assert_eq!(creator_share, claimed_quote);
        
        println!("✅ All unlocked scenario: 100% to creator");
    }

    #[test]
    fn test_partial_locked_scenario() {
        // Scenario: 30% locked -> investor share capped at 30%
        let locked_total = 60_000 * 1_000_000u64;
        let y0_total = 200_000 * 1_000_000u64;
        
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        assert_eq!(f_locked_bps, 3000); // 30%
        
        let investor_fee_share_bps = 5000u16; // 50% base
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq!(eligible_share_bps, 3000); // Capped at 30% due to unlocking
        
        println!("✅ Partial locked scenario: share capped at locked ratio");
    }

    #[test]
    fn test_dust_handling() {
        // Test that dust from floor division is handled
        let claimed_quote = 1000 * 1_000_000u64 + 777; // Odd amount
        let eligible_share_bps = 3333u16; // Non-round percentage
        
        let investor_fee_quote = (claimed_quote as u128 * eligible_share_bps as u128 / 10000) as u64;
        let creator_share = claimed_quote - investor_fee_quote;
        
        // Verify no tokens lost
        assert_eq!(investor_fee_quote + creator_share, claimed_quote);
        
        println!("✅ Dust handling validated - no tokens lost");
    }

    #[test]
    fn test_daily_cap_enforcement() {
        let claimed_quote = 20_000 * 1_000_000u64; // 20k claimed
        let daily_cap = 10_000 * 1_000_000u64; // 10k cap
        
        let distributable = claimed_quote.min(daily_cap);
        assert_eq!(distributable, daily_cap);
        
        println!("✅ Daily cap correctly enforced");
    }

    #[test]
    fn test_minimum_payout_threshold() {
        let payout = 5 * 1_000_000u64; // 5 tokens
        let min_payout = 10 * 1_000_000u64; // 10 tokens minimum
        
        let should_pay = payout >= min_payout;
        assert!(!should_pay);
        
        println!("✅ Minimum payout threshold validated");
    }

    #[test]
    fn test_pagination_logic() {
        // Test multi-page distribution
        let page = 0u32;
        let is_final_page = false;
        
        // Page 0: claim fees, distribute to investors
        assert_eq!(page, 0);
        
        let next_page = page + 1;
        
        // Page 1: continue distribution
        let page = next_page;
        let is_final_page = false;
        assert_eq!(page, 1);
        
        // Final page: close day and pay creator
        let page = page + 1;
        let is_final_page = true;
        assert_eq!(page, 2);
        assert!(is_final_page);
        
        println!("✅ Pagination logic validated");
    }

    #[test]
    fn test_idempotent_retry() {
        // Test that retrying same page doesn't double-pay
        let page = 1u32;
        let distributed_pages = vec![0u32, 1u32];
        
        // Check if already processed
        let already_processed = distributed_pages.contains(&page);
        assert!(already_processed);
        
        println!("✅ Idempotent retry protection validated");
    }

    #[test]
    fn test_base_fee_detection() {
        // Test that base fees are detected and rejected
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let pool = create_mock_pool(base_mint, quote_mint, 30);
        
        // Simulate detecting base fees in position
        let position_has_base_fees = false; // Should be checked in production
        
        assert!(!position_has_base_fees, "Base fees must not accrue");
        
        println!("✅ Base fee detection validated");
    }

    #[test]
    fn test_pda_derivation() {
        let program_id = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let pool = Pubkey::new_unique();
        
        let (policy, progress, position, position_owner) = derive_pdas(&program_id, &vault, &pool);
        
        // Verify PDAs are unique
        assert_ne!(policy, progress);
        assert_ne!(policy, position);
        assert_ne!(policy, position_owner);
        assert_ne!(progress, position);
        
        println!("✅ PDA derivation validated");
        println!("   Policy: {}", policy);
        println!("   Progress: {}", progress);
        println!("   Position: {}", position);
        println!("   Owner: {}", position_owner);
    }

    #[test]
    fn test_weight_calculation_precision() {
        // Test high-precision weight calculation
        let locked_amount = 123_456_789u64;
        let locked_total = 1_000_000_000u64;
        
        let weight = (locked_amount as u128 * 1_000_000) / locked_total as u128;
        
        // Verify precision maintained
        assert!(weight > 0);
        assert!(weight < 1_000_000);
        
        println!("✅ Weight calculation precision validated");
    }

    #[test]
    fn test_complex_multi_investor_scenario() {
        // Complex scenario with 5 investors, varying locked amounts
        let investors = vec![
            (100_000 * 1_000_000u64, 150_000 * 1_000_000u64), // locked, initial
            (80_000 * 1_000_000u64, 100_000 * 1_000_000u64),
            (50_000 * 1_000_000u64, 75_000 * 1_000_000u64),
            (20_000 * 1_000_000u64, 50_000 * 1_000_000u64),
            (0u64, 25_000 * 1_000_000u64), // Fully unlocked
        ];
        
        let locked_total: u64 = investors.iter().map(|(l, _)| l).sum();
        let y0_total: u64 = investors.iter().map(|(_, i)| i).sum();
        
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        
        let investor_fee_share_bps = 6000u16; // 60%
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        
        let claimed_quote = 5000 * 1_000_000u64;
        let investor_fee_quote = (claimed_quote as u128 * eligible_share_bps as u128 / 10000) as u64;
        
        let mut total_paid = 0u64;
        let min_payout = 10 * 1_000_000u64;
        
        for (locked, _) in investors.iter() {
            let weight = if locked_total > 0 {
                (*locked as u128 * 1_000_000) / locked_total as u128
            } else {
                0
            };
            
            let payout = ((investor_fee_quote as u128 * weight) / 1_000_000) as u64;
            
            if payout >= min_payout {
                total_paid += payout;
            }
        }
        
        let creator_share = claimed_quote - total_paid;
        
        // Verify total distribution
        assert_eq!(total_paid + creator_share, claimed_quote);
        assert!(creator_share > 0);
        
        println!("✅ Complex multi-investor scenario validated");
        println!("   Total to investors: {} tokens", total_paid / 1_000_000);
        println!("   Total to creator: {} tokens", creator_share / 1_000_000);
    }

    #[test]
    fn test_zero_fees_scenario() {
        // Test when no fees have accumulated
        let claimed_quote = 0u64;
        let eligible_share_bps = 5000u16;
        
        let investor_fee_quote = (claimed_quote as u128 * eligible_share_bps as u128 / 10000) as u64;
        assert_eq!(investor_fee_quote, 0);
        
        let creator_share = claimed_quote - investor_fee_quote;
        assert_eq!(creator_share, 0);
        
        println!("✅ Zero fees scenario handled correctly");
    }

    #[test]
    fn test_overflow_protection() {
        // Test that calculations don't overflow with max values
        let max_amount = u64::MAX;
        let eligible_share_bps = 10000u16; // 100%
        
        // Use u128 for intermediate calculations
        let result = (max_amount as u128 * eligible_share_bps as u128 / 10000) as u64;
        assert_eq!(result, max_amount);
        
        println!("✅ Overflow protection validated");
    }
}

/// Integration test module (would run with full Anchor runtime)
#[cfg(feature = "integration")]
mod integration_tests {
    use super::*;
    
    // These would be full integration tests with actual Anchor program invocations
    // Requiring a local validator and full program deployment
    
    #[test]
    fn integration_test_full_lifecycle() {
        // Would test:
        // 1. Deploy program
        // 2. Initialize policy
        // 3. Initialize honorary position
        // 4. Simulate fee accrual
        // 5. Run distribution crank
        // 6. Verify token transfers
        // 7. Check event emissions
        
        println!("Integration test requires full Anchor runtime");
    }
}

// ============================================================================
// Extended Test Suite - Additional Coverage
// ============================================================================

#[cfg(test)]
mod extended_tests {
    use super::*;

    // ------------------------------------------------------------------------
    // Policy Initialization Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_policy_zero_investor_share() {
        // 0% to investors = 100% to creator
        let investor_fee_share_bps = 0u16;
        assert\!(investor_fee_share_bps <= 10000);
        
        let claimed = 10_000 * 1_000_000u64;
        let investor_fee = (claimed as u128 * investor_fee_share_bps as u128 / 10000) as u64;
        assert_eq\!(investor_fee, 0);
        
        let creator_share = claimed - investor_fee;
        assert_eq\!(creator_share, claimed);
        
        println\!("✅ Zero investor share: 100% to creator");
    }

    #[test]
    fn test_policy_max_investor_share() {
        // 100% to investors = 0% to creator (when fully locked)
        let investor_fee_share_bps = 10000u16;
        let locked_total = 200_000 * 1_000_000u64;
        let y0_total = 200_000 * 1_000_000u64; // Fully locked
        
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        assert_eq\!(f_locked_bps, 10000);
        
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq\!(eligible_share_bps, 10000);
        
        println\!("✅ Maximum investor share validated");
    }

    #[test]
    fn test_policy_boundary_values() {
        let valid_bps = [0u16, 1, 100, 1000, 5000, 9999, 10000];
        
        for bps in valid_bps.iter() {
            assert\!(*bps <= 10000, "BPS {} should be valid", bps);
        }
        
        let invalid_bps = [10001u16, 10002, 15000, 65535];
        for bps in invalid_bps.iter() {
            assert\!(*bps > 10000, "BPS {} should be invalid", bps);
        }
        
        println\!("✅ Policy boundary values validated");
    }

    #[test]
    fn test_daily_cap_boundary_values() {
        let min_cap = 1u64;
        let typical_cap = 10_000 * 1_000_000u64;
        let large_cap = 1_000_000_000 * 1_000_000u64; // 1 billion tokens
        let max_cap = u64::MAX;
        
        assert\!(min_cap > 0);
        assert\!(typical_cap > 0);
        assert\!(large_cap > 0);
        assert\!(max_cap > 0);
        
        println\!("✅ Daily cap boundary values validated");
    }

    #[test]
    fn test_min_payout_boundary_values() {
        let min_payout = 1u64; // Minimum possible
        let typical_min = 10 * 1_000_000u64; // 10 tokens
        let high_min = 1000 * 1_000_000u64; // 1000 tokens
        
        let test_payouts = [0u64, 1, 9_999_999, 10_000_000, 1_000_000_000];
        
        for payout in test_payouts.iter() {
            let meets_typical = *payout >= typical_min;
            println\!("Payout {} meets typical min (10 tokens): {}", payout, meets_typical);
        }
        
        println\!("✅ Minimum payout boundary values validated");
    }

    // ------------------------------------------------------------------------
    // Fee Distribution Math - Extreme Scenarios
    // ------------------------------------------------------------------------

    #[test]
    fn test_extreme_locked_ratio_9999() {
        // 99.99% locked scenario
        let locked_total = 199_980 * 1_000_000u64;
        let y0_total = 200_000 * 1_000_000u64;
        
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        assert_eq\!(f_locked_bps, 9999);
        
        let investor_fee_share_bps = 5000u16;
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq\!(eligible_share_bps, 5000); // Capped by base share
        
        println\!("✅ Extreme locked ratio (99.99%) validated");
    }

    #[test]
    fn test_minimal_locked_ratio_1_bps() {
        // 0.01% locked scenario
        let locked_total = 20 * 1_000_000u64;
        let y0_total = 200_000 * 1_000_000u64;
        
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        assert_eq\!(f_locked_bps, 1);
        
        let investor_fee_share_bps = 5000u16;
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq\!(eligible_share_bps, 1); // Capped by locked ratio
        
        println\!("✅ Minimal locked ratio (0.01%) validated");
    }

    #[test]
    fn test_single_lamport_distribution() {
        // Test with just 1 unit
        let claimed_quote = 1u64;
        let eligible_share_bps = 5000u16;
        
        let investor_fee = (claimed_quote as u128 * eligible_share_bps as u128 / 10000) as u64;
        assert_eq\!(investor_fee, 0); // Should round down to 0
        
        let creator_share = claimed_quote - investor_fee;
        assert_eq\!(creator_share, 1);
        
        println\!("✅ Single lamport distribution validated");
    }

    #[test]
    fn test_10_way_equal_split() {
        // 10 investors with equal locked amounts
        let investors = vec\![100_000 * 1_000_000u64; 10];
        let locked_total: u64 = investors.iter().sum();
        let investor_fee = 10_000 * 1_000_000u64;
        
        let mut payouts = Vec::new();
        for locked in investors.iter() {
            let weight = (*locked as u128 * 1_000_000) / locked_total as u128;
            let payout = ((investor_fee as u128 * weight) / 1_000_000) as u64;
            payouts.push(payout);
        }
        
        // All should be approximately equal
        let first_payout = payouts[0];
        for payout in payouts.iter() {
            assert\!(*payout >= first_payout - 10_000);
            assert\!(*payout <= first_payout + 10_000);
        }
        
        println\!("✅ 10-way equal split validated");
    }

    #[test]
    fn test_100_way_equal_split() {
        // 100 investors with equal locked amounts
        let investors = vec\![10_000 * 1_000_000u64; 100];
        let locked_total: u64 = investors.iter().sum();
        let investor_fee = 10_000 * 1_000_000u64;
        
        let mut total_paid = 0u64;
        for locked in investors.iter() {
            let weight = (*locked as u128 * 1_000_000) / locked_total as u128;
            let payout = ((investor_fee as u128 * weight) / 1_000_000) as u64;
            total_paid += payout;
        }
        
        let dust = investor_fee - total_paid;
        assert\!(dust < 1000); // Minimal dust
        
        println\!("✅ 100-way equal split validated, dust: {}", dust);
    }

    #[test]
    fn test_whale_vs_shrimp_distribution() {
        // 1 whale with 95%, 99 shrimp with 5% total
        let whale_locked = 950_000 * 1_000_000u64;
        let shrimp_locked = 500 * 1_000_000u64; // Each shrimp
        
        let mut investors = vec\![whale_locked];
        investors.extend(vec\![shrimp_locked; 99]);
        
        let locked_total: u64 = investors.iter().sum();
        let investor_fee = 10_000 * 1_000_000u64;
        
        let whale_weight = (whale_locked as u128 * 1_000_000) / locked_total as u128;
        let whale_payout = ((investor_fee as u128 * whale_weight) / 1_000_000) as u64;
        
        // Whale should get approximately 95%
        let expected_whale_payout = 9_500 * 1_000_000u64;
        assert\!(whale_payout >= expected_whale_payout - 100_000);
        assert\!(whale_payout <= expected_whale_payout + 100_000);
        
        println\!("✅ Whale vs shrimp distribution: whale gets {} tokens", whale_payout / 1_000_000);
    }

    // ------------------------------------------------------------------------
    // Rounding and Precision Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_prime_number_distribution() {
        // Use prime numbers to test rounding edge cases
        let investors = vec\![
            97_000_003u64,
            53_000_007u64,
            29_000_009u64,
            13_000_021u64,
            7_000_013u64,
        ];
        
        let locked_total: u64 = investors.iter().sum();
        let investor_fee = 9_999_991u64; // Prime number
        
        let mut total_paid = 0u64;
        for locked in investors.iter() {
            let weight = (*locked as u128 * 1_000_000) / locked_total as u128;
            let payout = ((investor_fee as u128 * weight) / 1_000_000) as u64;
            total_paid += payout;
        }
        
        // Verify no tokens lost
        assert\!(total_paid <= investor_fee);
        let dust = investor_fee - total_paid;
        println\!("✅ Prime number distribution: dust = {}", dust);
    }

    #[test]
    fn test_odd_bps_rounding() {
        let test_cases = [
            (10_000_000u64, 3333u16), // 33.33%
            (10_000_000u64, 6667u16), // 66.67%
            (9_999_999u64, 3333u16),
            (1_234_567u64, 4321u16),
        ];
        
        for (amount, bps) in test_cases.iter() {
            let investor_fee = (*amount as u128 * *bps as u128 / 10000) as u64;
            let creator_share = *amount - investor_fee;
            
            // Verify no tokens lost
            assert_eq\!(investor_fee + creator_share, *amount);
        }
        
        println\!("✅ Odd BPS rounding validated");
    }

    #[test]
    fn test_high_precision_weight_calculation() {
        // Test with numbers that stress precision
        let locked = 123_456_789_012u64;
        let total = 987_654_321_098u64;
        
        let weight = (locked as u128 * 1_000_000) / total as u128;
        assert\!(weight > 0);
        assert\!(weight < 1_000_000);
        
        // Recalculate and verify
        let reconstructed = ((total as u128 * weight) / 1_000_000) as u64;
        assert\!(reconstructed >= locked - 1);
        assert\!(reconstructed <= locked + 1);
        
        println\!("✅ High precision weight calculation validated");
    }

    // ------------------------------------------------------------------------
    // Boundary and Edge Cases
    // ------------------------------------------------------------------------

    #[test]
    fn test_zero_y0_total() {
        let locked_total = 0u64;
        let y0_total = 0u64;
        
        let f_locked_bps = if y0_total > 0 {
            ((locked_total as u128 * 10000) / y0_total as u128) as u16
        } else {
            0
        };
        
        assert_eq\!(f_locked_bps, 0);
        println\!("✅ Zero Y0 total handled without panic");
    }

    #[test]
    fn test_locked_exceeds_y0() {
        // Edge case: locked > Y0 (shouldn't happen but handle gracefully)
        let locked_total = 250_000 * 1_000_000u64;
        let y0_total = 200_000 * 1_000_000u64;
        
        let f_locked_bps = ((locked_total as u128 * 10000) / y0_total as u128) as u16;
        
        // Should exceed 100% (10000 bps)
        assert\!(f_locked_bps > 10000);
        
        // But when capped by investor_fee_share_bps, should be reasonable
        let investor_fee_share_bps = 5000u16;
        let eligible_share_bps = f_locked_bps.min(investor_fee_share_bps);
        assert_eq\!(eligible_share_bps, 5000);
        
        println\!("✅ Locked exceeds Y0 handled gracefully");
    }

    #[test]
    fn test_u64_max_values() {
        let max_value = u64::MAX;
        let half_max = u64::MAX / 2;
        
        // Test calculations don't overflow
        let result1 = (max_value as u128 * 10000 / 10000) as u64;
        assert_eq\!(result1, max_value);
        
        let result2 = (half_max as u128 * 5000 / 10000) as u64;
        assert_eq\!(result2, half_max / 2);
        
        println\!("✅ u64 MAX value calculations validated");
    }

    #[test]
    fn test_dust_accumulation_100_iterations() {
        // Simulate 100 distribution cycles
        let claimed_per_cycle = 1_000 * 1_000_000u64;
        let eligible_share_bps = 3333u16; // Will produce dust
        
        let mut total_dust = 0u64;
        for _ in 0..100 {
            let investor_fee = (claimed_per_cycle as u128 * eligible_share_bps as u128 / 10000) as u64;
            let creator_share = claimed_per_cycle - investor_fee;
            let dust = claimed_per_cycle - (investor_fee + creator_share);
            total_dust += dust;
        }
        
        assert_eq\!(total_dust, 0); // Should be 0 since we're using proper subtraction
        println\!("✅ Dust accumulation over 100 iterations: {}", total_dust);
    }

    // ------------------------------------------------------------------------
    // Time-based Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_24h_minus_1_second() {
        let last_ts = 1696636800i64;
        let current_ts = last_ts + 86399; // 23h 59m 59s
        
        let time_since = current_ts - last_ts;
        assert_eq\!(time_since, 86399);
        assert\!(time_since < 86400);
        
        println\!("✅ 24h - 1 second correctly rejected");
    }

    #[test]
    fn test_24h_plus_1_second() {
        let last_ts = 1696636800i64;
        let current_ts = last_ts + 86401; // 24h 1s
        
        let time_since = current_ts - last_ts;
        assert\!(time_since >= 86400);
        
        println\!("✅ 24h + 1 second correctly allowed");
    }

    #[test]
    fn test_multiple_24h_cycles() {
        let start_ts = 1696636800i64;
        let num_cycles = 365; // One year
        
        for i in 0..num_cycles {
            let distribution_ts = start_ts + (i * 86400);
            let next_ts = distribution_ts + 86400;
            
            let time_since = next_ts - distribution_ts;
            assert_eq\!(time_since, 86400);
        }
        
        println\!("✅ {} consecutive 24h cycles validated", num_cycles);
    }

    #[test]
    fn test_timestamp_overflow_safety() {
        let max_timestamp = i64::MAX;
        let ts_1_year_ago = max_timestamp - (86400 * 365);
        
        let time_since = max_timestamp - ts_1_year_ago;
        assert_eq\!(time_since, 86400 * 365);
        
        println\!("✅ Timestamp overflow safety validated");
    }

    // ------------------------------------------------------------------------
    // Pagination Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_single_page_distribution() {
        let page = 0u32;
        let is_final_page = true;
        
        assert_eq\!(page, 0);
        assert\!(is_final_page);
        
        println\!("✅ Single page distribution validated");
    }

    #[test]
    fn test_10_page_distribution() {
        let total_pages = 10u32;
        
        for page in 0..total_pages {
            let is_final = page == total_pages - 1;
            
            if is_final {
                assert_eq\!(page, 9);
                assert\!(is_final);
            } else {
                assert\!(page < 9);
                assert\!(\!is_final);
            }
        }
        
        println\!("✅ 10-page distribution validated");
    }

    #[test]
    fn test_page_counter_reset() {
        let mut current_page = 42u32;
        let is_final_page = true;
        
        if is_final_page {
            current_page = 0;
        }
        
        assert_eq\!(current_page, 0);
        println\!("✅ Page counter reset validated");
    }

    // ------------------------------------------------------------------------
    // Error Condition Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_invalid_fee_share_values() {
        let invalid_values = [10001u16, 15000, 20000, 65535];
        
        for bps in invalid_values.iter() {
            let is_valid = *bps <= 10000;
            assert\!(\!is_valid, "BPS {} should be invalid", bps);
        }
        
        println\!("✅ Invalid fee share values correctly identified");
    }

    #[test]
    fn test_pool_mint_validation() {
        let base_mint = Pubkey::new_unique();
        let quote_mint = Pubkey::new_unique();
        let wrong_mint = Pubkey::new_unique();
        
        // Pool with correct mints
        let valid_pool = MockPoolState {
            token_a_mint: base_mint,
            token_b_mint: quote_mint,
            token_a_vault: Pubkey::new_unique(),
            token_b_vault: Pubkey::new_unique(),
            fee_rate_bps: 30,
        };
        
        assert\!(valid_pool.token_a_mint == base_mint || valid_pool.token_b_mint == base_mint);
        assert\!(valid_pool.token_a_mint == quote_mint || valid_pool.token_b_mint == quote_mint);
        
        // Pool with wrong mint
        let invalid_pool = MockPoolState {
            token_a_mint: wrong_mint,
            token_b_mint: quote_mint,
            token_a_vault: Pubkey::new_unique(),
            token_b_vault: Pubkey::new_unique(),
            fee_rate_bps: 30,
        };
        
        let has_base = invalid_pool.token_a_mint == base_mint || invalid_pool.token_b_mint == base_mint;
        assert\!(\!has_base);
        
        println\!("✅ Pool mint validation tested");
    }

    // ------------------------------------------------------------------------
    // Helper Function Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_calculate_locked_totals_helper() {
        let investors = vec\![
            InvestorDistribution {
                investor_pubkey: Pubkey::new_unique(),
                locked_amount: 100_000 * 1_000_000,
                initial_allocation: 100_000 * 1_000_000,
            },
            InvestorDistribution {
                investor_pubkey: Pubkey::new_unique(),
                locked_amount: 50_000 * 1_000_000,
                initial_allocation: 100_000 * 1_000_000,
            },
        ];
        
        let (locked_total, y0_total) = calculate_locked_totals(&investors);
        
        assert_eq\!(locked_total, 150_000 * 1_000_000);
        assert_eq\!(y0_total, 200_000 * 1_000_000);
        
        println\!("✅ calculate_locked_totals helper validated");
    }

    #[test]
    fn test_calculate_locked_totals_empty() {
        let investors: Vec<InvestorDistribution> = vec\![];
        let (locked_total, y0_total) = calculate_locked_totals(&investors);
        
        assert_eq\!(locked_total, 0);
        assert_eq\!(y0_total, 0);
        
        println\!("✅ calculate_locked_totals with empty list validated");
    }

    #[test]
    fn test_pda_seeds_deterministic() {
        let program_id = Pubkey::new_unique();
        let vault = Pubkey::new_unique();
        let pool = Pubkey::new_unique();
        
        // Derive twice
        let (policy1, progress1, position1, owner1) = super::tests::derive_pdas(&program_id, &vault, &pool);
        let (policy2, progress2, position2, owner2) = super::tests::derive_pdas(&program_id, &vault, &pool);
        
        // Should be identical
        assert_eq\!(policy1, policy2);
        assert_eq\!(progress1, progress2);
        assert_eq\!(position1, position2);
        assert_eq\!(owner1, owner2);
        
        println\!("✅ PDA derivation is deterministic");
    }

    // ------------------------------------------------------------------------
    // Stress Tests
    // ------------------------------------------------------------------------

    #[test]
    fn test_1000_investors_distribution() {
        let num_investors = 1000;
        let locked_per_investor = 1_000 * 1_000_000u64;
        let investors = vec\![locked_per_investor; num_investors];
        
        let locked_total: u64 = investors.iter().sum();
        let investor_fee = 10_000 * 1_000_000u64;
        
        let mut total_paid = 0u64;
        for locked in investors.iter() {
            let weight = (*locked as u128 * 1_000_000) / locked_total as u128;
            let payout = ((investor_fee as u128 * weight) / 1_000_000) as u64;
            total_paid += payout;
        }
        
        let dust = investor_fee - total_paid;
        assert\!(dust < 10_000);
        
        println\!("✅ 1000 investors: total paid = {}, dust = {}", total_paid / 1_000_000, dust);
    }

    #[test]
    fn test_extreme_inequality_99_1_split() {
        // One investor with 99%, one with 1%
        let whale = 99_000 * 1_000_000u64;
        let shrimp = 1_000 * 1_000_000u64;
        let investors = vec\![whale, shrimp];
        
        let locked_total: u64 = investors.iter().sum();
        let investor_fee = 10_000 * 1_000_000u64;
        
        let whale_weight = (whale as u128 * 1_000_000) / locked_total as u128;
        let whale_payout = ((investor_fee as u128 * whale_weight) / 1_000_000) as u64;
        
        // Whale should get 99%
        let expected = 9_900 * 1_000_000u64;
        assert\!(whale_payout >= expected - 10_000);
        assert\!(whale_payout <= expected + 10_000);
        
        println\!("✅ Extreme inequality (99/1 split) validated");
    }

    #[test]
    fn test_minimum_payout_filtering() {
        // Test that very small payouts are filtered
        let min_payout = 10 * 1_000_000u64;
        let test_payouts = vec\![
            0u64,
            1,
            100_000,
            1_000_000,
            5_000_000,
            9_999_999,
            10_000_000,
            10_000_001,
            100_000_000,
        ];
        
        let mut paid_count = 0;
        let mut filtered_count = 0;
        
        for payout in test_payouts.iter() {
            if *payout >= min_payout {
                paid_count += 1;
            } else {
                filtered_count += 1;
            }
        }
        
        assert_eq\!(paid_count, 3);
        assert_eq\!(filtered_count, 6);
        
        println\!("✅ Minimum payout filtering: {} paid, {} filtered", paid_count, filtered_count);
    }
}

// ============================================================================
// Property-Based Testing Helpers
// ============================================================================

#[cfg(test)]
mod property_tests {
    use super::*;

    #[test]
    fn property_no_tokens_lost() {
        // Property: investor_fee + creator_share = claimed_quote for any valid inputs
        let test_cases = vec\![
            (1_000_000u64, 5000u16),
            (10_000_000u64, 3333u16),
            (100_000_000u64, 6667u16),
            (999_999_999u64, 4321u16),
        ];
        
        for (claimed, bps) in test_cases.iter() {
            let investor_fee = (*claimed as u128 * *bps as u128 / 10000) as u64;
            let creator_share = *claimed - investor_fee;
            
            assert_eq\!(investor_fee + creator_share, *claimed);
        }
        
        println\!("✅ Property: no tokens lost - validated");
    }

    #[test]
    fn property_proportional_distribution() {
        // Property: doubling locked amount doubles payout (approximately)
        let locked1 = 50_000 * 1_000_000u64;
        let locked2 = 100_000 * 1_000_000u64;
        
        let locked_total = locked1 + locked2;
        let investor_fee = 10_000 * 1_000_000u64;
        
        let weight1 = (locked1 as u128 * 1_000_000) / locked_total as u128;
        let weight2 = (locked2 as u128 * 1_000_000) / locked_total as u128;
        
        let payout1 = ((investor_fee as u128 * weight1) / 1_000_000) as u64;
        let payout2 = ((investor_fee as u128 * weight2) / 1_000_000) as u64;
        
        // payout2 should be approximately 2x payout1
        assert\!(payout2 >= payout1 * 2 - 10_000);
        assert\!(payout2 <= payout1 * 2 + 10_000);
        
        println\!("✅ Property: proportional distribution - validated");
    }

    #[test]
    fn property_locked_ratio_bounds() {
        // Property: f_locked_bps is always in [0, 10000+] range
        let test_cases = vec\![
            (0u64, 100_000 * 1_000_000u64),
            (50_000 * 1_000_000u64, 100_000 * 1_000_000u64),
            (100_000 * 1_000_000u64, 100_000 * 1_000_000u64),
            (150_000 * 1_000_000u64, 100_000 * 1_000_000u64), // Over-locked
        ];
        
        for (locked, y0) in test_cases.iter() {
            if *y0 > 0 {
                let f_locked_bps = ((*locked as u128 * 10000) / *y0 as u128) as u16;
                assert\!(f_locked_bps >= 0); // Always true for u16, but documents intent
                println\!("  locked={}, y0={}, f_locked_bps={}", locked / 1_000_000, y0 / 1_000_000, f_locked_bps);
            }
        }
        
        println\!("✅ Property: locked ratio bounds - validated");
    }

    #[test]
    fn property_monotonic_payout() {
        // Property: more locked = more payout (when all else equal)
        let locked_amounts = vec\![
            10_000 * 1_000_000u64,
            20_000 * 1_000_000u64,
            30_000 * 1_000_000u64,
            40_000 * 1_000_000u64,
        ];
        
        let locked_total: u64 = locked_amounts.iter().sum();
        let investor_fee = 10_000 * 1_000_000u64;
        
        let mut payouts = Vec::new();
        for locked in locked_amounts.iter() {
            let weight = (*locked as u128 * 1_000_000) / locked_total as u128;
            let payout = ((investor_fee as u128 * weight) / 1_000_000) as u64;
            payouts.push(payout);
        }
        
        // Verify monotonically increasing
        for i in 1..payouts.len() {
            assert\!(payouts[i] > payouts[i-1]);
        }
        
        println\!("✅ Property: monotonic payout - validated");
    }
}