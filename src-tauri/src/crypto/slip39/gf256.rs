//! Arithmetic in GF(256) and Lagrange interpolation.
//!
//! Elements are bytes interpreted over the Rijndael field (reduction polynomial
//! x⁸ + x⁴ + x³ + x + 1), exactly as in AES and as mandated by SLIP-0039.
//! Multiplication is the bitwise carry-less product; inverses are computed as
//! `a²⁵⁴` (Fermat), which keeps this module free of precomputed tables.

/// Carry-less multiplication modulo the Rijndael polynomial.
pub fn mul(mut a: u8, mut b: u8) -> u8 {
    let mut product = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            product ^= a;
        }
        let carry = a & 0x80;
        a <<= 1;
        if carry != 0 {
            a ^= 0x1b;
        }
        b >>= 1;
    }
    product
}

/// Multiplicative inverse. `a` must be non-zero.
fn inv(a: u8) -> u8 {
    debug_assert!(a != 0, "GF(256) has no inverse for zero");
    let mut result = 1u8;
    let mut base = a;
    let mut exponent = 254u32;
    while exponent > 0 {
        if exponent & 1 == 1 {
            result = mul(result, base);
        }
        base = mul(base, base);
        exponent >>= 1;
    }
    result
}

/// Division. `b` must be non-zero.
fn div(a: u8, b: u8) -> u8 {
    if a == 0 {
        0
    } else {
        mul(a, inv(b))
    }
}

/// Evaluate, at `x`, the polynomials defined byte-wise by the given points.
///
/// Each point is an `x` coordinate paired with a value vector; all value
/// vectors must have the same length `n`. The result is the length-`n` vector
/// `(f₁(x), …, fₙ(x))`, where `fₖ` is the unique lowest-degree polynomial
/// through the `k`-th coordinate of every point. The caller guarantees the `x`
/// coordinates are pairwise distinct.
pub fn interpolate(x: u8, points: &[(u8, Vec<u8>)]) -> Vec<u8> {
    // If x coincides with a known point, that point *is* the answer — and
    // taking the Lagrange route would divide by zero.
    if let Some((_, value)) = points.iter().find(|(xi, _)| *xi == x) {
        return value.clone();
    }

    let n = points.first().map_or(0, |(_, value)| value.len());
    let mut result = vec![0u8; n];

    for (xi, yi) in points {
        // Lagrange basis: ∏_{j≠i} (x − xⱼ) / (xᵢ − xⱼ). Subtraction is XOR.
        let mut numerator = 1u8;
        let mut denominator = 1u8;
        for (xj, _) in points {
            if xj == xi {
                continue;
            }
            numerator = mul(numerator, x ^ xj);
            denominator = mul(denominator, xi ^ xj);
        }
        let coefficient = div(numerator, denominator);

        for (slot, &y) in result.iter_mut().zip(yi.iter()) {
            *slot ^= mul(coefficient, y);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multiplication_is_commutative_with_identity() {
        assert_eq!(mul(0, 123), 0);
        assert_eq!(mul(1, 123), 123);
        assert_eq!(mul(123, 45), mul(45, 123));
    }

    #[test]
    fn every_nonzero_element_has_an_inverse() {
        for a in 1u8..=255 {
            assert_eq!(mul(a, inv(a)), 1, "inverse failed for {a}");
        }
    }

    #[test]
    fn interpolation_recovers_the_constant_term() {
        // Line through (1, 5) and (2, ?) for the polynomial f(t) = 5 + 7·t.
        let f = |t: u8| 5u8 ^ mul(7, t);
        let points = vec![(1u8, vec![f(1)]), (2u8, vec![f(2)])];
        assert_eq!(interpolate(0, &points), vec![5]);
        assert_eq!(interpolate(3, &points), vec![f(3)]);
    }

    #[test]
    fn interpolation_returns_a_matching_point_directly() {
        let points = vec![(10u8, vec![42]), (20u8, vec![99])];
        assert_eq!(interpolate(10, &points), vec![42]);
    }
}
