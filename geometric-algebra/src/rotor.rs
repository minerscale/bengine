use std::ops::{Add, Mul, Sub};

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Rotor<T> {
    pub e: T,
    pub e12: T,
    pub e31: T,
    pub e23: T,
}

impl<T: std::ops::Neg<Output = T>> Rotor<T> {
    pub fn conjugate(self) -> Rotor<T> {
        Rotor {
            e: self.e,
            e12: -self.e12,
            e31: -self.e31,
            e23: -self.e23,
        }
    }
}

#[rustfmt::skip]
impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T> + num_traits::Zero> Mul for Rotor<T> {
    type Output = Rotor<T>;

    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;
        Self {
            e:    a.e*b.e     - a.e12*b.e12  - a.e31*b.e31  - a.e23*b.e23,
            e12:  a.e*b.e12   + a.e12*b.e    + a.e31*b.e23  - a.e23*b.e31,
            e31:  a.e*b.e31   - a.e12*b.e23  + a.e31*b.e    + a.e23*b.e12,
            e23:  a.e*b.e23   + a.e12*b.e31  - a.e31*b.e12  + a.e23*b.e,
        }
    }
}
