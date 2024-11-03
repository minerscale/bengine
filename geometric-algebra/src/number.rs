use std::ops::{Add, Div, Mul, Sub};

use crate::{bivector::BiVector, rotor::Rotor, vector::Vector};

// 3D Geometric Algebra in its full glory
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Number<T> {
    pub e: T,
    pub e1: T,
    pub e2: T,
    pub e3: T,
    pub e12: T,
    pub e31: T,
    pub e23: T,
    pub e123: T,
}

impl<T: Copy + num_traits::ConstZero + num_traits::ConstOne> Number<T> {
    pub const ZERO: Self = Self {
        e: T::ZERO,
        e1: T::ZERO,
        e2: T::ZERO,
        e3: T::ZERO,
        e12: T::ZERO,
        e31: T::ZERO,
        e23: T::ZERO,
        e123: T::ZERO,
    };

    pub const E: Self = Self {
        e: T::ONE,
        ..Self::ZERO
    };

    pub const E1: Self = Self {
        e1: T::ONE,
        ..Self::ZERO
    };

    pub const E2: Self = Self {
        e2: T::ONE,
        ..Self::ZERO
    };

    pub const E3: Self = Self {
        e3: T::ONE,
        ..Self::ZERO
    };

    pub const E12: Self = Self {
        e12: T::ONE,
        ..Self::ZERO
    };

    pub const E31: Self = Self {
        e31: T::ONE,
        ..Self::ZERO
    };

    pub const E23: Self = Self {
        e23: T::ONE,
        ..Self::ZERO
    };

    pub const E123: Self = Self {
        e123: T::ONE,
        ..Self::ZERO
    };
}

impl<T: num_traits::Zero> Default for Number<T> {
    fn default() -> Self {
        let z = T::zero;
        Self {
            e: z(),
            e1: z(),
            e2: z(),
            e3: z(),
            e12: z(),
            e31: z(),
            e23: z(),
            e123: z(),
        }
    }
}

impl<T: Copy + num_traits::Zero> From<Vector<T>> for Number<T> {
    fn from(v: Vector<T>) -> Self {
        let z = num_traits::zero();
        Number {
            e: z,
            e1: v.e1,
            e2: v.e2,
            e3: v.e3,
            e12: z,
            e31: z,
            e23: z,
            e123: z,
        }
    }
}

impl<T: Copy + num_traits::Zero> From<BiVector<T>> for Number<T> {
    fn from(bv: BiVector<T>) -> Self {
        let z = num_traits::zero();
        Number {
            e: z,
            e1: z,
            e2: z,
            e3: z,
            e12: bv.e12,
            e31: bv.e31,
            e23: bv.e23,
            e123: z,
        }
    }
}

impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T> + num_traits::Zero> Number<T> {
    #[rustfmt::skip]
    pub fn dot(&self, rhs: Self) -> Self {
        let a = self;
        let b = rhs;

        Self {
        //    e          | e1          | e2          | e3          | e12         | e31          | e23          | e123
        e:    a.e*b.e    + a.e1*b.e1   + a.e2*b.e2   + a.e3*b.e3   - a.e12*b.e12 - a.e31*b.e31  - a.e23*b.e23  - a.e123*b.e123,
        e1:   a.e*b.e1   + a.e1*b.e                                                             - a.e23*b.e123 - a.e123*b.e23,
        e2:   a.e*b.e2                 + a.e2*b.e                                - a.e31*b.e123                - a.e123*b.e31,
        e3:   a.e*b.e3                               + a.e3*b.e    - a.e12*b.e123                              - a.e123*b.e12,
        e12:  a.e*b.e12                              + a.e3*b.e123 + a.e12*b.e                                 + a.e123*b.e3,
        e31:  a.e*b.e31                + a.e2*b.e123                             + a.e31*b.e                   + a.e123*b.e2,
        e23:  a.e*b.e23  + a.e1*b.e123                                                          + a.e23*b.e    + a.e123*b.e1,
        e123: a.e*b.e123 + a.e1*b.e23  + a.e2*b.e31  + a.e3*b.e12  + a.e12*b.e3  + a.e31*b.e2   + a.e23*b.e1   + a.e123*b.e,
        }
    }

    #[rustfmt::skip]
    pub fn cross(&self, rhs: Self) -> Self {
        let a = self;
        let b = rhs;
        let z = num_traits::zero();
        Self {
        //    e          | e1          | e2          | e3          | e12         | e31          | e23         | e123
        e:    z,
        e1:   z                        - a.e2*b.e12  + a.e3*b.e31  + a.e12*b.e2  - a.e31*b.e3,
        e2:   z          + a.e1*b.e12                - a.e3*b.e23  - a.e12*b.e1                 + a.e23*b.e3,
        e3:   z          - a.e1*b.e31  + a.e2*b.e23                              + a.e31*b.e1   - a.e23*b.e2,
        e12:  z          + a.e1*b.e2   - a.e2*b.e1                               + a.e31*b.e23  - a.e23*b.e31,
        e31:  z          - a.e1*b.e3                 + a.e3*b.e1   - a.e12*b.e23                + a.e23*b.e12,
        e23:  z                        + a.e2*b.e3   - a.e3*b.e2   + a.e12*b.e31 - a.e31*b.e12,
        e123: z,
        }
    }
}

impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T>> Mul for Number<T> {
    type Output = Self;

    #[rustfmt::skip]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;
        Self {
            e:    a.e*b.e    + a.e1*b.e1   + a.e2*b.e2   + a.e3*b.e3   - a.e12*b.e12  - a.e31*b.e31  - a.e23*b.e23  - a.e123*b.e123,
            e1:   a.e*b.e1   + a.e1*b.e    - a.e2*b.e12  + a.e3*b.e31  + a.e12*b.e2   - a.e31*b.e3   - a.e23*b.e123 - a.e123*b.e23,
            e2:   a.e*b.e2   + a.e1*b.e12  + a.e2*b.e    - a.e3*b.e23  - a.e12*b.e1   - a.e31*b.e123 + a.e23*b.e3   - a.e123*b.e31,
            e3:   a.e*b.e3   - a.e1*b.e31  + a.e2*b.e23  + a.e3*b.e    - a.e12*b.e123 + a.e31*b.e1   - a.e23*b.e2   - a.e123*b.e12,
            e12:  a.e*b.e12  + a.e1*b.e2   - a.e2*b.e1   + a.e3*b.e123 + a.e12*b.e    + a.e31*b.e23  - a.e23*b.e31  + a.e123*b.e3,
            e31:  a.e*b.e31  - a.e1*b.e3   + a.e2*b.e123 + a.e3*b.e1   - a.e12*b.e23  + a.e31*b.e    + a.e23*b.e12  + a.e123*b.e2,
            e23:  a.e*b.e23  + a.e1*b.e123 + a.e2*b.e3   - a.e3*b.e2   + a.e12*b.e31  - a.e31*b.e12  + a.e23*b.e    + a.e123*b.e1,
            e123: a.e*b.e123 + a.e1*b.e23  + a.e2*b.e31  + a.e3*b.e12  + a.e12*b.e3   + a.e31*b.e2   + a.e23*b.e1   + a.e123*b.e,
        }
    }
}

impl<T: Add<Output = T>> Add for Number<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            e: self.e + rhs.e,
            e1: self.e1 + rhs.e1,
            e2: self.e2 + rhs.e2,
            e3: self.e3 + rhs.e3,
            e12: self.e12 + rhs.e12,
            e31: self.e31 + rhs.e31,
            e23: self.e23 + rhs.e23,
            e123: self.e123 + rhs.e123,
        }
    }
}

impl<T: Sub<Output = T>> Sub for Number<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            e: self.e - rhs.e,
            e1: self.e1 - rhs.e1,
            e2: self.e2 - rhs.e2,
            e3: self.e3 - rhs.e3,
            e12: self.e12 - rhs.e12,
            e31: self.e31 - rhs.e31,
            e23: self.e23 - rhs.e23,
            e123: self.e123 - rhs.e123,
        }
    }
}

impl<T: Copy + Mul<Output = T>> Number<T> {
    pub fn scalar_product(self, rhs: T) -> Self {
        Self {
            e: self.e * rhs,
            e1: self.e1 * rhs,
            e2: self.e2 * rhs,
            e3: self.e3 * rhs,
            e12: self.e12 * rhs,
            e31: self.e31 * rhs,
            e23: self.e23 * rhs,
            e123: self.e123 * rhs,
        }
    }
}

impl<T: Copy + Div<Output = T>> Number<T> {
    pub fn scalar_divide(self, rhs: T) -> Self {
        Self {
            e: self.e / rhs,
            e1: self.e1 / rhs,
            e2: self.e2 / rhs,
            e3: self.e3 / rhs,
            e12: self.e12 / rhs,
            e31: self.e31 / rhs,
            e23: self.e23 / rhs,
            e123: self.e123 / rhs,
        }
    }
}

impl<T: Copy + num_traits::Zero> From<Rotor<T>> for Number<T> {
    fn from(r: Rotor<T>) -> Self {
        let z = num_traits::zero();
        Number {
            e: r.e,
            e1: z,
            e2: z,
            e3: z,
            e12: r.e12,
            e31: r.e31,
            e23: r.e23,
            e123: z,
        }
    }
}
