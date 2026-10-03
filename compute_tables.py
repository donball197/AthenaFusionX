#!/usr/bin/env python3
"""
Compute and verify tables script.
This script computes various mathematical tables and verifies their correctness.
"""

import math
import sys


def compute_multiplication_table(n=10):
    """Compute multiplication table up to n x n."""
    table = []
    for i in range(1, n + 1):
        row = []
        for j in range(1, n + 1):
            row.append(i * j)
        table.append(row)
    return table


def compute_powers_table(base_max=10, exp_max=5):
    """Compute powers table: base^exp for base in 1..base_max, exp in 1..exp_max."""
    table = []
    for base in range(1, base_max + 1):
        row = []
        for exp in range(1, exp_max + 1):
            row.append(base ** exp)
        table.append(row)
    return table


def compute_factorial_table(n=10):
    """Compute factorial table: n! for n in 0..n."""
    table = []
    for i in range(n + 1):
        table.append(math.factorial(i))
    return table


def compute_fibonacci_table(n=15):
    """Compute Fibonacci table: F(n) for n in 0..n."""
    table = []
    a, b = 0, 1
    for _ in range(n + 1):
        table.append(a)
        a, b = b, a + b
    return table


def compute_prime_table(limit=50):
    """Compute prime numbers up to limit using Sieve of Eratosthenes."""
    sieve = [True] * (limit + 1)
    sieve[0] = sieve[1] = False
    for i in range(2, int(limit**0.5) + 1):
        if sieve[i]:
            for j in range(i*i, limit + 1, i):
                sieve[j] = False
    return [i for i, is_prime in enumerate(sieve) if is_prime]


def verify_multiplication_table(table):
    """Verify multiplication table correctness."""
    for i, row in enumerate(table, 1):
        for j, val in enumerate(row, 1):
            if val != i * j:
                return False, f"Mismatch at ({i},{j}): expected {i*j}, got {val}"
    return True, "Multiplication table verified"


def verify_powers_table(table, base_max=10, exp_max=5):
    """Verify powers table correctness."""
    for base_idx, row in enumerate(table):
        base = base_idx + 1
        for exp_idx, val in enumerate(row):
            exp = exp_idx + 1
            if val != base ** exp:
                return False, f"Mismatch at base={base}, exp={exp}: expected {base**exp}, got {val}"
    return True, "Powers table verified"


def verify_factorial_table(table):
    """Verify factorial table correctness."""
    for i, val in enumerate(table):
        if val != math.factorial(i):
            return False, f"Mismatch at {i}!: expected {math.factorial(i)}, got {val}"
    return True, "Factorial table verified"


def verify_fibonacci_table(table):
    """Verify Fibonacci table correctness."""
    if len(table) < 2:
        return True, "Fibonacci table verified (trivial)"
    if table[0] != 0 or table[1] != 1:
        return False, f"Base cases wrong: F(0)={table[0]}, F(1)={table[1]}"
    for i in range(2, len(table)):
        if table[i] != table[i-1] + table[i-2]:
            return False, f"Mismatch at F({i}): expected {table[i-1]+table[i-2]}, got {table[i]}"
    return True, "Fibonacci table verified"


def verify_prime_table(primes, limit=50):
    """Verify prime table correctness."""
    # Check all numbers up to limit
    for n in range(2, limit + 1):
        is_prime = n in primes
        # Verify by trial division
        actual_prime = all(n % d != 0 for d in range(2, int(n**0.5) + 1))
        if is_prime != actual_prime:
            return False, f"Mismatch at {n}: table says {is_prime}, actual {actual_prime}"
    return True, "Prime table verified"


def print_table(name, table, cols_per_row=10):
    """Pretty print a table."""
    print(f"\n=== {name} ===")
    if isinstance(table[0], list):
        # 2D table
        for i, row in enumerate(table):
            print(f"  {i+1:2d}: " + " ".join(f"{v:4d}" for v in row))
    else:
        # 1D table
        for i in range(0, len(table), cols_per_row):
            chunk = table[i:i+cols_per_row]
            indices = range(i, min(i+cols_per_row, len(table)))
            print("  " + " ".join(f"{idx:2d}:{val:4d}" for idx, val in zip(indices, chunk)))


def main():
    print("Computing and verifying mathematical tables...\n")
    
    all_passed = True
    
    # Multiplication table
    mult_table = compute_multiplication_table(10)
    print_table("Multiplication Table (10x10)", mult_table)
    ok, msg = verify_multiplication_table(mult_table)
    print(f"  Verification: {'PASS' if ok else 'FAIL'} - {msg}")
    all_passed = all_passed and ok
    
    # Powers table
    powers_table = compute_powers_table(10, 5)
    print_table("Powers Table (base^exp, base=1..10, exp=1..5)", powers_table)
    ok, msg = verify_powers_table(powers_table, 10, 5)
    print(f"  Verification: {'PASS' if ok else 'FAIL'} - {msg}")
    all_passed = all_passed and ok
    
    # Factorial table
    fact_table = compute_factorial_table(10)
    print_table("Factorial Table (0! to 10!)", fact_table)
    ok, msg = verify_factorial_table(fact_table)
    print(f"  Verification: {'PASS' if ok else 'FAIL'} - {msg}")
    all_passed = all_passed and ok
    
    # Fibonacci table
    fib_table = compute_fibonacci_table(15)
    print_table("Fibonacci Table (F(0) to F(15))", fib_table)
    ok, msg = verify_fibonacci_table(fib_table)
    print(f"  Verification: {'PASS' if ok else 'FAIL'} - {msg}")
    all_passed = all_passed and ok
    
    # Prime table
    prime_table = compute_prime_table(50)
    print_table("Prime Numbers up to 50", prime_table)
    ok, msg = verify_prime_table(prime_table, 50)
    print(f"  Verification: {'PASS' if ok else 'FAIL'} - {msg}")
    all_passed = all_passed and ok
    
    print("\n" + "="*50)
    if all_passed:
        print("ALL TABLES VERIFIED SUCCESSFULLY ✓")
        return 0
    else:
        print("SOME VERIFICATIONS FAILED ✗")
        return 1


if __name__ == "__main__":
    sys.exit(main())