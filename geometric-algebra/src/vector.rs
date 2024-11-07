use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::{bivector::BiVector, rotor::Rotor};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Vector<T> {
    pub e1: T,
    pub e2: T,
    pub e3: T,
}

impl<T: Copy + num_traits::ConstZero + num_traits::ConstOne> Vector<T> {
    pub const ZERO: Self = Self {
        e1: T::ZERO,
        e2: T::ZERO,
        e3: T::ZERO,
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
}

impl<T> Vector<T> {
    pub fn new(e1: T, e2: T, e3: T) -> Self {
        Self { e1, e2, e3 }
    }

    pub fn from_slice(slice: [T; 3]) -> Self {
        match slice {
            [e1, e2, e3] => Self::new(e1, e2, e3),
        }
    }
}

impl<T: Add<Output = T>> Add for Vector<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            e1: self.e1 + rhs.e1,
            e2: self.e2 + rhs.e2,
            e3: self.e3 + rhs.e3,
        }
    }
}

impl<T: Sub<Output = T>> Sub for Vector<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            e1: self.e1 - rhs.e1,
            e2: self.e2 - rhs.e2,
            e3: self.e3 - rhs.e3,
        }
    }
}

impl<T: Copy + Mul<Output = T>> Vector<T> {
    pub fn scalar_product(self, rhs: T) -> Self {
        Self {
            e1: self.e1 * rhs,
            e2: self.e2 * rhs,
            e3: self.e3 * rhs,
        }
    }
}

impl<T: Copy + Div<Output = T>> Vector<T> {
    pub fn scalar_divide(self, rhs: T) -> Self {
        Self {
            e1: self.e1 / rhs,
            e2: self.e2 / rhs,
            e3: self.e3 / rhs,
        }
    }
}

#[rustfmt::skip]
impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T> + num_traits::Zero> Mul for Vector<T> {
    type Output = Rotor<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;

        let e = a.dot(b);
        let bv = a.wedge(b);

        Self::Output {
            e,
            e12: bv.e12,
            e31: bv.e31,
            e23: bv.e23, 
        }
    }
}

impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T> + num_traits::Zero> Vector<T> {
    pub fn dot(self, rhs: Self) -> T {
        let a = self;
        let b = rhs;

        a.e1 * b.e1 + a.e2 * b.e2 + a.e3 * b.e3
    }

    pub fn wedge(self, rhs: Self) -> BiVector<T> {
        let a = self;
        let b = rhs;

        BiVector {
            e12: a.e1 * b.e2 - a.e2 * b.e1,
            e31: a.e3 * b.e1 - a.e1 * b.e3,
            e23: a.e2 * b.e3 - a.e3 * b.e2,
        }
    }
}

impl<T: Copy + num_traits::Float> Vector<T> {
    pub fn norm(self) -> Self {
        self.scalar_divide(self.dot(self).sqrt())
    }
}

impl<T: Copy + Add<Output = T> + Mul<Output = T> + Sub<Output = T> + Neg<Output = T>> Vector<T> {
    #[inline(never)]
    pub fn rotate(self, rotor: Rotor<T>) -> Self {
        let k = rotor.e;
        let a = rotor.e12;
        let b = rotor.e31;
        let c = rotor.e23;

        let x = self.e1;
        let y = self.e2;
        let z = self.e3;

        let r = k * x + a * y - b * z;
        let s = -a * x + k * y + c * z;
        let t = b * x + k * z - c * y;
        let u = c * x + b * y + a * z;

        Vector {
            e1: r * k + s * a - t * b + u * c,
            e2: -r * a + s * k + t * c + u * b,
            e3: r * b - s * c + t * k + u * a,
        }
    }
}
