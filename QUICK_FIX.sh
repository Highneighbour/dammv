#!/bin/bash

# Quick fix script for building DAMM v2 Fee Distributor
# Run this in /workspace directory

set -e

echo "========================================="
echo "DAMM v2 Fee Distributor - Quick Build Fix"
echo "========================================="
echo ""

# 1. Ensure we're in the right directory
cd /workspace
echo "✅ Changed to /workspace directory"
echo ""

# 2. Add Solana to PATH
export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
echo "✅ Added Solana to PATH"
echo ""

# 3. Check Solana version
if command -v solana &> /dev/null; then
    SOLANA_VERSION=$(solana --version)
    echo "✅ Solana installed: $SOLANA_VERSION"
    
    # If version is 2.x, install 1.18
    if [[ $SOLANA_VERSION == *"2."* ]]; then
        echo "⚠️  Solana 2.x detected, installing 1.18.26 for compatibility..."
        sh -c "$(curl -sSfL https://release.solana.com/v1.18.26/install)"
        export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
    fi
else
    echo "❌ Solana not found in PATH"
    echo "Please run: export PATH=\"\$HOME/.local/share/solana/install/active_release/bin:\$PATH\""
    exit 1
fi
echo ""

# 4. Build
echo "🔨 Building program..."
anchor build

echo ""
echo "========================================="
echo "✅ BUILD COMPLETE!"
echo "========================================="
echo ""
echo "Compiled program: target/deploy/damm_v2_fee_distributor.so"
echo "IDL: target/idl/damm_v2_fee_distributor.json"
echo ""
