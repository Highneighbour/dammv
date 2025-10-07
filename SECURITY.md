# Security Policy

## Overview

This module implements fee distribution logic for the Star fundraising protocol. Security is paramount given the financial nature of the application.

## Critical Security Features

### 1. PDA Validation
- All Program Derived Addresses use deterministic seed derivation
- Bump seeds are stored and validated
- No unsafe `UncheckedAccount` usage without explicit validation

### 2. Arithmetic Safety
- All fee calculations use `u128` for intermediate values
- Results are checked for overflow before casting to `u64`
- Floor rounding prevents fractional token amounts

### 3. Access Control
- Policy initialization requires creator signature
- Distribution crank is permissionless but gated by 24h timer
- Position owner PDA is the only authority for LP position

### 4. Quote-Only Enforcement
- Multiple validation layers ensure only quote fees accrue
- Pool configuration checked at initialization
- Fee claims validate token types

### 5. Reentrancy Protection
- No cross-program invocations during state mutations
- State updates happen atomically
- No external calls within critical sections

## Known Limitations

### 1. Streamflow Integration
The current implementation includes mock Streamflow interfaces. In production:
- Must implement actual Streamflow CPI calls
- Validate Streamflow program ownership
- Verify stream account authenticity

### 2. DAMM v2 Integration
The mock pool state should be replaced with:
- Actual DAMM v2 program integration
- Real CP-AMM pool interactions
- Proper fee claim instruction invocations

### 3. Token Transfers
The distribution crank currently logs transfers. Production must:
- Implement actual SPL token transfers
- Use token program CPI correctly
- Validate token accounts and authorities

## Audit Recommendations

Before production deployment, conduct a thorough security audit covering:

1. **PDA Derivation Logic**
   - Verify all seeds are deterministic
   - Check for PDA collision risks
   - Validate bump seed storage

2. **Fee Distribution Math**
   - Test edge cases (0 locked, 100% locked, etc.)
   - Verify no rounding exploits
   - Check dust handling

3. **Access Control**
   - Validate signer requirements
   - Check for privilege escalation paths
   - Verify PDA authority usage

4. **State Consistency**
   - Test pagination edge cases
   - Verify idempotency
   - Check for state corruption scenarios

5. **Token Security**
   - Validate ATA derivation
   - Check for token drain risks
   - Verify authority delegation

## Reporting Vulnerabilities

If you discover a security vulnerability:

1. **DO NOT** create a public GitHub issue
2. Email the security team directly
3. Include:
   - Detailed description
   - Steps to reproduce
   - Potential impact assessment
   - Suggested fix (if any)

## Security Best Practices for Users

### For Policy Creators
- Use a dedicated keypair for creator role
- Store private keys securely (hardware wallet recommended)
- Verify all parameters before initialization
- Monitor events for unexpected activity

### For Crank Operators
- Validate investor data before submission
- Use read-only RPC nodes to query Streamflow
- Monitor gas costs and set reasonable limits
- Implement retry logic with exponential backoff

### For Integrators
- Validate program ID before interaction
- Verify PDAs are correctly derived
- Check account ownership in clients
- Implement proper error handling

## Dependency Security

### Anchor Framework
- Version: 0.29.0
- Known vulnerabilities: None at time of writing
- Update policy: Follow Anchor security advisories

### SPL Token
- Use official SPL token program
- Validate token program ID in all instructions
- Check for deprecated CPI patterns

## Ongoing Security Measures

1. **Regular Updates**
   - Monitor Anchor framework updates
   - Track Solana runtime changes
   - Update dependencies promptly

2. **Event Monitoring**
   - Log all critical operations
   - Monitor emitted events
   - Alert on unexpected patterns

3. **Rate Limiting**
   - 24h distribution gate prevents spam
   - Pagination limits prevent DoS
   - Daily caps protect against exploits

## Incident Response

In case of security incident:

1. **Immediate**: Pause distributions if possible
2. **Assess**: Determine scope and impact
3. **Contain**: Prevent further damage
4. **Notify**: Alert affected parties
5. **Fix**: Deploy patched version
6. **Review**: Post-mortem analysis

## Compliance

This module aims to comply with:
- Solana program best practices
- Anchor framework guidelines
- General smart contract security standards

## Disclaimer

This software is provided "as is" without warranty of any kind. Users assume all risks associated with its use. Conduct your own security review before production deployment.

---

**Last Updated:** 2025-10-07
