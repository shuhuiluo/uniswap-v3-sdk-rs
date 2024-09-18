use alloy_primitives::{Signed, Uint};
use bigdecimal::BigDecimal;
use num_bigint::{BigInt, BigUint, Sign};

pub trait ToBig {
    fn to_big_uint(&self) -> BigUint;

    fn to_big_int(&self) -> BigInt;

    #[inline]
    fn to_big_decimal(&self) -> BigDecimal {
        BigDecimal::from(self.to_big_int())
    }
}

impl<const BITS: usize, const LIMBS: usize> ToBig for Uint<BITS, LIMBS> {
    #[inline]
    fn to_big_uint(&self) -> BigUint {
        BigUint::from_bytes_le(&self.as_le_bytes())
    }

    #[inline]
    fn to_big_int(&self) -> BigInt {
        BigInt::from_biguint(Sign::Plus, self.to_big_uint())
    }
}

impl<const BITS: usize, const LIMBS: usize> ToBig for Signed<BITS, LIMBS> {
    #[inline]
    fn to_big_uint(&self) -> BigUint {
        self.into_raw().to_big_uint()
    }

    #[inline]
    fn to_big_int(&self) -> BigInt {
        BigInt::from_signed_bytes_le(&self.into_raw().as_le_bytes())
    }
}

pub trait FromBig {
    fn from_big_uint(x: BigUint) -> Self;
    fn from_big_int(x: BigInt) -> Self;
}

impl<const BITS: usize, const LIMBS: usize> FromBig for Uint<BITS, LIMBS> {
    #[inline]
    fn from_big_uint(x: BigUint) -> Self {
        Self::from_limbs_slice(&x.to_u64_digits())
    }

    #[inline]
    fn from_big_int(x: BigInt) -> Self {
        let (sign, data) = x.to_u64_digits();
        match sign {
            Sign::Plus => Self::from_limbs_slice(&data),
            Sign::NoSign => Self::ZERO,
            Sign::Minus => -Self::from_limbs_slice(&data),
        }
    }
}

impl<const BITS: usize, const LIMBS: usize> FromBig for Signed<BITS, LIMBS> {
    #[inline]
    fn from_big_uint(x: BigUint) -> Self {
        Self::from_raw(Uint::from_big_uint(x))
    }

    #[inline]
    fn from_big_int(x: BigInt) -> Self {
        Self::from_raw(Uint::from_big_int(x))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{I256, U256};

    #[test]
    fn test_uint_to_big() {
        let x = U256::from_limbs([1, 2, 3, 4]);
        let y = BigUint::from(1_u64)
            + (BigUint::from(2_u64) << 64)
            + (BigUint::from(3_u64) << 128)
            + (BigUint::from(4_u64) << 192);
        assert_eq!(x.to_big_uint(), y);
        assert_eq!(x.to_big_int(), BigInt::from_biguint(Sign::Plus, y.clone()));
        assert_eq!(
            x.to_big_decimal(),
            BigDecimal::from(BigInt::from_biguint(Sign::Plus, y))
        );
    }

    #[test]
    fn test_signed_to_big() {
        let x = I256::from_raw(U256::from_limbs([1, 2, 3, 4]));
        let y: BigInt = BigInt::from(1)
            + (BigInt::from(2) << 64)
            + (BigInt::from(3) << 128)
            + (BigInt::from(4) << 192);
        assert_eq!(x.to_big_uint(), y.to_biguint().unwrap());
        assert_eq!(x.to_big_int(), y);
        assert_eq!(x.to_big_decimal(), BigDecimal::from(y.clone()));

        let x = -x;
        let z: BigInt = (BigInt::from(1) << 256) - y.clone();
        assert_eq!(x.to_big_uint(), z.to_biguint().unwrap());
        assert_eq!(x.to_big_int(), -y.clone());
        assert_eq!(x.to_big_decimal(), BigDecimal::from(-y));
    }

    #[test]
    fn test_uint_from_big() {
        let x = U256::from_limbs([1, 2, 3, 4]);
        assert_eq!(U256::from_big_uint(x.to_big_uint()), x);
        assert_eq!(U256::from_big_int(x.to_big_int()), x);

        let x = -x;
        assert_eq!(U256::from_big_uint(x.to_big_uint()), x);
        assert_eq!(U256::from_big_int(x.to_big_int()), x);
    }

    #[test]
    fn test_signed_from_big() {
        let x = I256::from_raw(U256::from_limbs([1, 2, 3, 4]));
        assert_eq!(I256::from_big_uint(x.to_big_uint()), x);
        assert_eq!(I256::from_big_int(x.to_big_int()), x);

        let x = -x;
        assert_eq!(I256::from_big_uint(x.to_big_uint()), x);
        assert_eq!(I256::from_big_int(x.to_big_int()), x);
        assert_eq!(
            x.to_big_uint() + (-x.to_big_int()).to_biguint().unwrap(),
            BigUint::from(1_u64) << 256
        );
    }
}
