use std::ops::{Add, Mul, Neg, Sub};

use crate::rotor::Rotor;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct BiVector<T> {
    pub e12: T,
    pub e31: T,
    pub e23: T,
}

impl<T: num_traits::Float + std::fmt::Debug> BiVector<T> {
    pub fn rotor(&self, angle: T) -> Rotor<T> {
        let two = T::one() + T::one();
        let angle = angle / two;

        let bv = {
            let norm = (self.e12 * self.e12 + self.e31 * self.e31 + self.e23 * self.e23)
                .sqrt()
                .recip();
            BiVector {
                e12: norm * self.e12,
                e31: norm * self.e31,
                e23: norm * self.e23,
            }
        };

        let sine = angle.sin();
        Rotor {
            e: angle.cos(),
            e12: bv.e12 * sine,
            e31: bv.e31 * sine,
            e23: bv.e23 * sine,
        }
    }
}

impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T> + Neg<Output = T>> BiVector<T> {
    pub fn dot(&self, rhs: Self) -> T {
        let a = self;
        let b = rhs;

        -a.e12 * b.e12 - a.e31 * b.e31 - a.e23 * b.e23
    }

    pub fn cross(&self, rhs: Self) -> Self {
        let a = self;
        let b = rhs;

        Self {
            e12: a.e31 * b.e23 - a.e23 * b.e31,
            e31: -a.e12 * b.e23 + a.e23 * b.e12,
            e23: a.e12 * b.e31 - a.e31 * b.e12,
        }
    }
}
