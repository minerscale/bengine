use std::ops::{Add, AddAssign, Div, Mul, Sub, SubAssign};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Vec2<T> {
    pub e1: T,
    pub e2: T,
}

impl<T: num_traits::ConstZero> Default for Vec2<T> {
    fn default() -> Self {
        Self {
            e1: T::ZERO,
            e2: T::ZERO,
        }
    }
}

impl<T: Copy + num_traits::ConstZero + num_traits::ConstOne> Vec2<T> {
    pub const ZERO: Self = Self {
        e1: T::ZERO,
        e2: T::ZERO,
    };

    pub const X_HAT: Self = Self {
        e1: T::ONE,
        e2: T::ZERO,
    };

    pub const Y_HAT: Self = Self {
        e1: T::ZERO,
        e2: T::ONE,
    };
}

pub struct Number2<T> {
    pub e: T,
    pub e1: T,
    pub e2: T,
    pub e12: T,
}

impl<T> Vec2<T> {
    pub fn new(e1: T, e2: T) -> Self {
        Self { e1, e2 }
    }
}

impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T> + num_traits::ConstZero> Mul
    for Vec2<T>
{
    type Output = Number2<T>;

    #[rustfmt::skip]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;
        Self::Output {
            e:    a.e1*b.e1 + a.e2*b.e2,
            e1:   T::ZERO,
            e2:   T::ZERO,
            e12:  a.e1*b.e2 - a.e2*b.e1,
        }
    }
}

impl<T: Add<Output = T>> Add for Vec2<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            e1: self.e1 + rhs.e1,
            e2: self.e2 + rhs.e2,
        }
    }
}

impl<T: AddAssign> AddAssign for Vec2<T> {
    fn add_assign(&mut self, rhs: Self) {
        self.e1 += rhs.e1;
        self.e2 += rhs.e2;
    }
}

impl<T: SubAssign> SubAssign for Vec2<T> {
    fn sub_assign(&mut self, rhs: Self) {
        self.e1 -= rhs.e1;
        self.e2 -= rhs.e2;
    }
}

impl<T: Sub<Output = T>> Sub for Vec2<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            e1: self.e1 - rhs.e1,
            e2: self.e2 - rhs.e2,
        }
    }
}

impl<T: Copy + Mul<Output = T>> Vec2<T> {
    pub fn scalar_product(self, rhs: T) -> Self {
        Self {
            e1: self.e1 * rhs,
            e2: self.e2 * rhs,
        }
    }
}

impl<T: Copy + Div<Output = T>> Vec2<T> {
    pub fn scalar_divide(self, rhs: T) -> Self {
        Self {
            e1: self.e1 / rhs,
            e2: self.e2 / rhs,
        }
    }
}

impl<T: num_traits::Zero> Default for Number2<T> {
    fn default() -> Self {
        let z = T::zero;

        Self {
            e: z(),
            e1: z(),
            e2: z(),
            e12: z(),
        }
    }
}

impl<T: Copy + num_traits::ConstZero + num_traits::ConstOne> Number2<T> {
    pub const ZERO: Self = Self {
        e: T::ZERO,
        e1: T::ZERO,
        e2: T::ZERO,
        e12: T::ZERO,
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

    pub const E12: Self = Self {
        e12: T::ONE,
        ..Self::ZERO
    };
}

impl<T: Copy + Mul<Output = T> + Add<Output = T> + Sub<Output = T>> Mul for Number2<T> {
    type Output = Self;

    #[rustfmt::skip]
    fn mul(self, rhs: Self) -> Self::Output {
        let a = self;
        let b = rhs;
        Self {
            e:    a.e*b.e   + a.e1*b.e1  + a.e2*b.e2  - a.e12*b.e12,
            e1:   a.e*b.e1  + a.e1*b.e   - a.e2*b.e12 + a.e12*b.e2,
            e2:   a.e*b.e2  + a.e1*b.e12 + a.e2*b.e   - a.e12*b.e1,
            e12:  a.e*b.e12 + a.e1*b.e2  - a.e2*b.e1  + a.e12*b.e,
        }
    }
}

impl<T: Add<Output = T>> Add for Number2<T> {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            e: self.e + rhs.e,
            e1: self.e1 + rhs.e1,
            e2: self.e2 + rhs.e2,
            e12: self.e12 + rhs.e12,
        }
    }
}

impl<T: Sub<Output = T>> Sub for Number2<T> {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            e: self.e - rhs.e,
            e1: self.e1 - rhs.e1,
            e2: self.e2 - rhs.e2,
            e12: self.e12 - rhs.e12,
        }
    }
}

impl<T: Copy + Mul<Output = T>> Number2<T> {
    pub fn scalar_product(self, rhs: T) -> Self {
        Self {
            e: self.e * rhs,
            e1: self.e1 * rhs,
            e2: self.e2 * rhs,
            e12: self.e12 * rhs,
        }
    }
}

impl<T: Copy + Div<Output = T>> Number2<T> {
    pub fn scalar_divide(self, rhs: T) -> Self {
        Self {
            e: self.e / rhs,
            e1: self.e1 / rhs,
            e2: self.e2 / rhs,
            e12: self.e12 / rhs,
        }
    }
}
