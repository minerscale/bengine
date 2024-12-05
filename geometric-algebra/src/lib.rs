pub mod bivector;
pub mod number;
pub mod rotor;
pub mod vec2;
pub mod vector;

#[cfg(test)]
mod tests {
    use crate::{bivector::BiVector, number::Number, vector::Vector};

    const A: Number<i32> = Number {
        e: 2,
        e1: 3,
        e2: 5,
        e3: 7,
        e12: 11,
        e31: 13,
        e23: 17,
        e123: 19,
    };

    const B: Number<i32> = Number {
        e: 23,
        e1: 29,
        e2: 31,
        e3: 37,
        e12: 41,
        e31: 43,
        e23: 47,
        e123: 53,
    };

    #[test]
    fn geometric_product() {
        assert_eq!(A * B, A.dot(B) + A.cross(B));
    }

    #[test]
    fn dot_product() {
        assert_eq!((A * B + B * A).scalar_divide(2), A.dot(B));
    }

    #[test]
    fn cross_product() {
        assert_eq!((A * B - B * A).scalar_divide(2), A.cross(B));
    }

    #[test]
    fn orthogonal_vectors() {
        // u,v are orthogonal vectors
        let u = Vector {
            e1: 2,
            e2: -3,
            e3: -1,
        };

        let v = Vector {
            e1: 3,
            e2: -2,
            e3: 4,
        };

        let zero_bv = BiVector {
            e12: 0,
            e31: 0,
            e23: 0,
        };
        assert_eq!(u.dot(v), v.dot(u));
        assert_eq!(u.wedge(u), zero_bv);
        assert_eq!(v.wedge(v), zero_bv);
    }

    #[test]
    fn rotor() {
        let a = Vector::<f64> {
            e1: 1.0,
            e2: 0.0,
            e3: 0.0,
        };
        let b = Vector::<f64> {
            e1: 0.0,
            e2: 1.0,
            e3: 0.0,
        };

        let r = a.wedge(b).rotor(std::f64::consts::PI);

        assert_eq!(r.e12, 1.0);
        let v = Vector::<f64> {
            e1: 1.0,
            e2: 0.0,
            e3: 0.0,
        };

        assert_eq!(v.rotate(r).e1, -1.0);
        assert_eq!(
            (Number::from(r) * v.into() * r.conjugate().into()).e1,
            v.rotate(r).e1
        );
    }
}
