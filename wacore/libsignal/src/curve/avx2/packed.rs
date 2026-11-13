// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
// Copyright (c) 2016-2021 isis lovecruft
// Copyright (c) 2016-2019 Henry de Valence
//! Private vector operations, inlined into the checked AVX2 entry point.
//! Signed intrinsic arguments preserve unsigned wrapping arithmetic bits.
use core::ops::{Add, AddAssign, BitAnd, BitAndAssign, Sub};
macro_rules! impl_shared {
    (
        $ty:ident,
        $add_intrinsic:ident,
        $sub_intrinsic:ident,
        $shl_intrinsic:ident
    ) => {
        #[allow(non_camel_case_types)]
        #[derive(Copy, Clone)]
        #[repr(transparent)]
        pub(super) struct $ty(core::arch::x86_64::__m256i);
        impl From<$ty> for core::arch::x86_64::__m256i {
            #[inline(always)]
            fn from(value: $ty) -> core::arch::x86_64::__m256i {
                value.0
            }
        }
        impl From<core::arch::x86_64::__m256i> for $ty {
            #[inline(always)]
            fn from(value: core::arch::x86_64::__m256i) -> $ty {
                $ty(value)
            }
        }
        impl Add for $ty {
            type Output = Self;
            #[inline(always)]
            fn add(self, rhs: $ty) -> Self {
                unsafe { core::arch::x86_64::$add_intrinsic(self.0, rhs.0).into() }
            }
        }
        #[allow(clippy::assign_op_pattern)]
        impl AddAssign for $ty {
            #[inline(always)]
            fn add_assign(&mut self, rhs: $ty) {
                *self = *self + rhs
            }
        }
        impl Sub for $ty {
            type Output = Self;
            #[inline(always)]
            fn sub(self, rhs: $ty) -> Self {
                unsafe { core::arch::x86_64::$sub_intrinsic(self.0, rhs.0).into() }
            }
        }
        impl BitAnd for $ty {
            type Output = Self;
            #[inline(always)]
            fn bitand(self, rhs: $ty) -> Self {
                unsafe { core::arch::x86_64::_mm256_and_si256(self.0, rhs.0).into() }
            }
        }

        impl $ty {
            #[inline(always)]
            pub(super) fn shl<const N: i32>(self) -> Self {
                unsafe { core::arch::x86_64::$shl_intrinsic(self.0, N).into() }
            }
        }
    };
}

impl_shared!(u64x4, _mm256_add_epi64, _mm256_sub_epi64, _mm256_slli_epi64);
impl_shared!(u32x8, _mm256_add_epi32, _mm256_sub_epi32, _mm256_slli_epi32);
impl From<u64x4> for u32x8 {
    #[inline(always)]
    fn from(value: u64x4) -> Self {
        Self(value.0)
    }
}
impl BitAndAssign for u64x4 {
    #[inline(always)]
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 = unsafe { core::arch::x86_64::_mm256_and_si256(self.0, rhs.0) };
    }
}
impl u64x4 {
    #[inline(always)]
    pub(super) fn splat(x: u64) -> u64x4 {
        unsafe { u64x4(core::arch::x86_64::_mm256_set1_epi64x(x as i64)) }
    }
}
impl u32x8 {
    #[allow(clippy::too_many_arguments)]
    pub(super) const fn new_const(
        x0: u32,
        x1: u32,
        x2: u32,
        x3: u32,
        x4: u32,
        x5: u32,
        x6: u32,
        x7: u32,
    ) -> Self {
        // SAFETY: Both types occupy 32 bytes and accept every bit pattern.
        unsafe {
            Self(
                core::mem::transmute::<[u32; 8], core::arch::x86_64::__m256i>([
                    x0, x1, x2, x3, x4, x5, x6, x7,
                ]),
            )
        }
    }
    pub(super) const fn splat_const<const N: u32>() -> Self {
        Self::new_const(N, N, N, N, N, N, N, N)
    }
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub(super) fn new(
        x0: u32,
        x1: u32,
        x2: u32,
        x3: u32,
        x4: u32,
        x5: u32,
        x6: u32,
        x7: u32,
    ) -> u32x8 {
        unsafe {
            u32x8(core::arch::x86_64::_mm256_set_epi32(
                x7 as i32, x6 as i32, x5 as i32, x4 as i32, x3 as i32, x2 as i32, x1 as i32,
                x0 as i32,
            ))
        }
    }
    #[inline(always)]
    pub(super) fn splat(x: u32) -> u32x8 {
        unsafe { u32x8(core::arch::x86_64::_mm256_set1_epi32(x as i32)) }
    }
}
impl u32x8 {
    #[inline(always)]
    pub(super) fn mul32(self, rhs: u32x8) -> u64x4 {
        unsafe { core::arch::x86_64::_mm256_mul_epu32(self.0, rhs.0).into() }
    }
}
impl u64x4 {
    #[inline(always)]
    pub(super) fn shr<const N: i32>(self) -> Self {
        unsafe { core::arch::x86_64::_mm256_srli_epi64(self.0, N).into() }
    }
}
impl u32x8 {
    #[inline(always)]
    pub(super) fn extract<const N: i32>(self) -> u32 {
        unsafe { core::arch::x86_64::_mm256_extract_epi32(self.0, N) as u32 }
    }
}
