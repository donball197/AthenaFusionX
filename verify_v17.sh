#!/bin/bash
echo -e "\e[1;36m>> [SCAN] VERIFYING SUBSTRATE SYMBOLS (V17 Entropy)...\e[0m"

# Check for Shannon Entropy Symbol
if nm -D libmuscle.so | grep -q "zx_vmo_calculate_entropy"; then
    echo -e "\e[1;32m[OK] Shannon Entropy Logic detected in Muscle.\e[0m"
else
    echo -e "\e[1;31m[FAIL] zx_vmo_calculate_entropy MISSING. Rebuild required.\e[0m"
fi

# Check for Thermal Scry Symbol
if nm -D libmuscle.so | grep -q "extract_thermal_pulse_raw"; then
    echo -e "\e[1;32m[OK] Thermal Scrying Logic detected in Muscle.\e[0m"
else
    echo -e "\e[1;31m[FAIL] extract_thermal_pulse_raw MISSING.\e[0m"
fi

# Check for Stress Mode Symbol
if nm -D libmuscle.so | grep -q "set_stress_mode"; then
    echo -e "\e[1;32m[OK] Hardware Stress Override detected.\e[0m"
else
    echo -e "\e[1;31m[FAIL] set_stress_mode MISSING.\e[0m"
fi

echo -e "\e[1;34m>> [SCAN] Checking Rust FFI Bridge Alignment...\e[0m"
grep -E "pub fn zx_vmo_calculate_entropy|pub fn set_stress_mode" src/kernel/ffi.rs
