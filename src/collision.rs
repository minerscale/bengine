/// collision.rs
/// This is an implementation of the GJK and EPA algorithms
/// Much of this implementation is directly and shamefully copied from https://winter.dev/
/// Also of great help is Reducible's excellent video on GJK: https://youtu.be/ajv46BSqcK4
/// and kevinmoran's implementattion of Winter's implementation in C#, https://github.com/kevinmoran/GJK/blob/master/GJK.h
/// which helped me catch the bug which I was trying to fix for multiple hours
use std::{
    io::BufRead,
    ops::{Index, IndexMut},
    rc::Rc,
};

use itertools::Itertools;
use obj::raw::RawObj;
use ultraviolet::{Isometry3, Vec2, Vec3};

pub trait Collider<T> {
    fn support(&self, d: T) -> T;
}

#[derive(Clone, Debug)]
pub struct Polyhedron<T> {
    vertices: Rc<[T]>,
}

#[derive(Clone, Debug)]
pub struct TransformedPolyhedron<T> {
    vertices: Box<[T]>,
}

fn transform_vecs(isometry: Isometry3, src: &[Vec3]) -> Box<[Vec3]> {
    let rotor = isometry.rotation;
    let translation = isometry.translation;

    let s2 = rotor.s * rotor.s;
    let bxy2 = rotor.bv.xy * rotor.bv.xy;
    let bxz2 = rotor.bv.xz * rotor.bv.xz;
    let byz2 = rotor.bv.yz * rotor.bv.yz;
    let s_bxy = rotor.s * rotor.bv.xy;
    let s_bxz = rotor.s * rotor.bv.xz;
    let s_byz = rotor.s * rotor.bv.yz;
    let bxz_byz = rotor.bv.xz * rotor.bv.yz;
    let bxy_byz = rotor.bv.xy * rotor.bv.yz;
    let bxy_bxz = rotor.bv.xy * rotor.bv.xz;

    let xa = s2 - bxy2 - bxz2 + byz2;
    let xb = s_bxy - bxz_byz;
    let xc = s_bxz + bxy_byz;

    let ya = -(bxz_byz + s_bxy);
    let yb = s2 - bxy2 + bxz2 - byz2;
    let yc = s_byz - bxy_bxz;

    let za = bxy_byz - s_bxz;
    let zb = bxy_bxz + s_byz;
    let zc = -(s2 + bxy2 - bxz2 - byz2);

    src.iter()
        .map(|vec| {
            let two_vx = vec.x + vec.x;
            let two_vy = vec.y + vec.y;
            let two_vz = vec.z + vec.z;

            Vec3::new(
                vec.x * xa + two_vy * xb + two_vz * xc,
                two_vx * ya + vec.y * yb + two_vz * yc,
                two_vx * za - two_vy * zb - vec.z * zc,
            ) + translation
        })
        .collect()
}

impl Polyhedron<Vec3> {
    pub fn new<T: BufRead>(file: T, scale: Option<Vec3>, transform: Option<Isometry3>) -> Self {
        let mesh: RawObj = obj::raw::parse_obj(file).unwrap();

        let vertices: Rc<[Vec3]> = mesh
            .positions
            .iter()
            .map(|v| {
                let v = scale.unwrap_or(Vec3::one()) * Vec3::from([v.0, v.1, v.2]);

                if let Some(t) = transform {
                    t.transform_vec(v)
                } else {
                    v
                }
            })
            .collect();

        Polyhedron { vertices }
    }

    pub fn transform(&mut self, isometry: ultraviolet::Isometry3) -> TransformedPolyhedron<Vec3> {
        TransformedPolyhedron {
            vertices: transform_vecs(isometry, &self.vertices),
        }
    }
}

impl Collider<Vec3> for TransformedPolyhedron<Vec3> {
    fn support(&self, d: Vec3) -> Vec3 {
        self.vertices
            .iter()
            .map(|&v| (v, d.dot(v)))
            .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap())
            .unwrap()
            .0
    }
}

struct Simplex<T, const N: usize> {
    points: [T; N],
    size: usize,
}

impl<T: Default + Copy, const N: usize> Simplex<T, N> {
    fn new() -> Self {
        Simplex {
            points: [T::default(); N],
            size: 0,
        }
    }

    fn push_front(&mut self, point: T) {
        self.points.copy_within(0..self.size, 1);
        self.points[0] = point;

        self.size = (self.size + 1).min(N);
    }

    fn set(&mut self, value: &[T]) {
        self.size = value.len();
        assert!(self.size <= N);

        self.points[0..self.size].copy_from_slice(value);
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl<T, const N: usize> Index<usize> for Simplex<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.size);

        &self.points[index]
    }
}

impl<T, const N: usize> IndexMut<usize> for Simplex<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.size);

        &mut self.points[index]
    }
}

fn line(simplex: &mut Simplex<(Vec3, Vec3, Vec3), 4>, direction: &mut Vec3) -> bool {
    let a = simplex[0];
    let b = simplex[1];

    let ab = b.0 - a.0;
    let ao = -a.0;

    if ab.dot(ao) > 0.0 {
        *direction = ab.cross(ao).cross(ab);
    } else {
        simplex.set(&[a]);
        *direction = ao;
    }

    false
}

fn triangle(simplex: &mut Simplex<(Vec3, Vec3, Vec3), 4>, direction: &mut Vec3) -> bool {
    let a = simplex[0];
    let b = simplex[1];
    let c = simplex[2];

    let ab = b.0 - a.0;
    let ac = c.0 - a.0;
    let ao = -a.0;

    let abc = ab.cross(ac);

    if abc.cross(ac).dot(ao) > 0.0 {
        if ac.dot(ao) > 0.0 {
            simplex.set(&[a, c]);
            *direction = ac.cross(ao).cross(ac);
        } else {
            simplex.set(&[a, b]);
            return line(simplex, direction);
        }
    } else if ab.cross(abc).dot(ao) > 0.0 {
        simplex.set(&[a, b]);
        return line(simplex, direction);
    } else if abc.dot(ao) > 0.0 {
        *direction = abc;
    } else {
        simplex.set(&[a, c, b]);
        *direction = -abc;
    }

    false
}

fn tetrahedron(simplex: &mut Simplex<(Vec3, Vec3, Vec3), 4>, direction: &mut Vec3) -> bool {
    let a = simplex[0];
    let b = simplex[1];
    let c = simplex[2];
    let d = simplex[3];

    let ab = b.0 - a.0;
    let ac = c.0 - a.0;
    let ad = d.0 - a.0;
    let ao = -a.0;

    let abc = ab.cross(ac);
    let acd = ac.cross(ad);
    let adb = ad.cross(ab);

    if abc.dot(ao) > 0.0 {
        simplex.set(&[a, b, c]);
        return triangle(simplex, direction);
    }

    if acd.dot(ao) > 0.0 {
        simplex.set(&[a, c, d]);
        return triangle(simplex, direction);
    }

    if adb.dot(ao) > 0.0 {
        simplex.set(&[a, d, b]);
        return triangle(simplex, direction);
    }

    true
}

pub fn collide<P: Collider<Vec3>, Q: Collider<Vec3>>(p: &P, q: &Q) -> Option<(Vec3, Vec3, f32)> {
    gjk_intersection(p, q, Vec3::unit_x()).map(|simplex| epa(&simplex, p, q).unwrap())
}

fn get_support_point<P: Collider<Vec3>, Q: Collider<Vec3>>(
    direction: Vec3,
    p: &P,
    q: &Q,
) -> (Vec3, Vec3, Vec3) {
    let p_support = p.support(direction);
    let q_support = q.support(-direction);
    (p_support - q_support, p_support, q_support)
}

const GJK_MAX_ITERATIONS: usize = 32;

fn gjk_intersection<P: Collider<Vec3>, Q: Collider<Vec3>>(
    p: &P,
    q: &Q,
    mut direction: Vec3,
) -> Option<Simplex<(Vec3, Vec3, Vec3), 4>> {
    let mut simplex = Simplex::<_, 4>::new();

    simplex.push_front(get_support_point(direction, p, q));

    direction = -simplex[0].0;

    let mut i = 0;
    loop {
        simplex.push_front(get_support_point(direction, p, q));

        if i == GJK_MAX_ITERATIONS || direction.dot(simplex[0].0) <= 0.0 {
            break false;
        }

        if match simplex.size() {
            2 => line(&mut simplex, &mut direction),
            3 => triangle(&mut simplex, &mut direction),
            4 => tetrahedron(&mut simplex, &mut direction),
            _ => panic!(),
        } {
            break true;
        }

        i += 1;
    }
    .then_some(simplex)
}

const EPA_EPSILON: f32 = 0.0001;
const EPA_MAX_ITERATIONS: usize = 64;

fn epa<P: Collider<Vec3>, Q: Collider<Vec3>>(
    simplex: &Simplex<(Vec3, Vec3, Vec3), 4>,
    p: &P,
    q: &Q,
) -> Option<(Vec3, Vec3, f32)> {
    let mut polytope = simplex.points.to_vec();

    let mut faces: Vec<[usize; 3]> = vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];

    let (mut normals, mut min_face) = get_face_normals(&polytope, &faces);

    let mut min_normal = Vec3::default();
    let mut min_distance = f32::MAX;

    let mut success: bool = false;

    for _ in 0..EPA_MAX_ITERATIONS {
        min_normal = normals[min_face].0;
        min_distance = normals[min_face].1;

        let support = get_support_point(min_normal, p, q);
        let s_distance = min_normal.dot(support.0);

        if (s_distance - min_distance).abs() <= EPA_EPSILON {
            success = true;
            break;
        }

        min_distance = f32::MAX;

        let mut unique_edges: Vec<(usize, usize)> = Vec::new();

        let mut i = 0;
        while i < normals.len() {
            if (normals[i].0.dot(support.0) - normals[i].1) > 0.0 {
                add_if_unique_edge(&mut unique_edges, &faces, i, 0, 1);
                add_if_unique_edge(&mut unique_edges, &faces, i, 1, 2);
                add_if_unique_edge(&mut unique_edges, &faces, i, 2, 0);

                faces.swap_remove(i);
                normals.swap_remove(i);
            } else {
                i += 1;
            }
        }

        let new_faces: Vec<[usize; 3]> = unique_edges
            .iter()
            .map(|(edge_index_1, edge_index_2)| [*edge_index_1, *edge_index_2, polytope.len()])
            .collect();

        polytope.push(support);

        let (new_normals, new_min_face) = get_face_normals(&polytope, &new_faces);
        let mut old_min_distance = f32::MAX;
        for (idx, normal) in normals.iter().enumerate() {
            if normal.1 < old_min_distance {
                old_min_distance = normal.1;
                min_face = idx;
            }
        }

        if new_normals[new_min_face].1 < old_min_distance {
            min_face = new_min_face + normals.len();
        }

        faces.extend(new_faces);
        normals.extend(new_normals);
    }

    if success {
        // get contact point
        let face = faces[min_face]
            .iter()
            .map(|&i| &polytope[i])
            .collect_array::<3>()
            .unwrap();

        let p = -min_normal * face[0].0.dot(min_normal);

        fn to_barycentric(p: Vec3, polytope_face: [Vec3; 3]) -> Vec3 {
            let f = polytope_face;
            let (v0, v1, v2) = (f[1] - f[0], f[2] - f[0], p - f[0]);

            let d00 = v0.dot(v0);
            let d01 = v0.dot(v1);
            let d11 = v1.dot(v1);
            let d20 = v2.dot(v0);
            let d21 = v2.dot(v1);

            let denom = d00 * d11 - d01 * d01;

            if denom.abs() <= EPA_EPSILON {
                // the triangle is degenerate
                if d00 <= EPA_EPSILON && d11 <= EPA_EPSILON {
                    Vec3::new(1.0, 0.0, 0.0)
                } else {
                    if d00 > EPA_EPSILON {
                        let t: f32 = d20 / d00;

                        Vec3::new(1.0 - t, t, 0.0)
                    } else if d11 > EPA_EPSILON {
                        let t: f32 = d21 / d11;

                        Vec3::new(1.0 - t, 0.0, t)
                    } else {
                        Vec3::new(1.0, 0.0, 0.0)
                    }
                }
            } else {
                let k = Vec2::new(d11 * d20 - d01 * d21, d00 * d21 - d01 * d20) / denom;

                Vec3::new(1.0 - k.x - k.y, k.x, k.y)
            }
        }

        fn barycentric_to_global(weights: Vec3, face: [Vec3; 3]) -> Vec3 {
            weights.x * face[0] + weights.y * face[1] + weights.z * face[2]
        }

        let polytope_face = [face[0].0, face[1].0, face[2].0];
        let real_face_1 = [face[0].1, face[1].1, face[2].1];
        let real_face_2 = [face[0].2, face[1].2, face[2].2];

        let weights = to_barycentric(p, polytope_face);

        let a = barycentric_to_global(weights, real_face_1);
        let b = barycentric_to_global(weights, real_face_2);

        Some(((a + b) / 2.0, min_normal, min_distance + EPA_EPSILON))
    } else {
        None
    }
}

fn add_if_unique_edge(
    edges: &mut Vec<(usize, usize)>,
    faces: &[[usize; 3]],
    face: usize,
    a: usize,
    b: usize,
) {
    match edges
        .iter()
        .find_position(|&&e| e == (faces[face][b], faces[face][a]))
    {
        Some((idx, _)) => {
            edges.remove(idx);
        }
        None => edges.push((faces[face][a], faces[face][b])),
    }
}

fn get_face_normals(
    polytope: &[(Vec3, Vec3, Vec3)],
    faces: &[[usize; 3]],
) -> (Vec<(Vec3, f32)>, usize) {
    let mut normals = Vec::new();

    let mut min_triangle = 0;
    let mut min_distance = f32::MAX;

    for (face_idx, face) in faces.iter().enumerate() {
        let a = polytope[face[0]].0;
        let b = polytope[face[1]].0;
        let c = polytope[face[2]].0;

        let unnormalised = (b - a).cross(c - a);
        let l = unnormalised.mag();

        let (mut normal, mut distance) = if l < EPA_EPSILON {
            (Vec3::zero(), f32::MAX)
        } else {
            let normal = unnormalised / l;
            (normal, normal.dot(a))
        };

        if distance < 0.0 {
            normal *= -1.0;
            distance *= -1.0;
        }

        normals.push((normal, distance));

        if distance < min_distance {
            min_triangle = face_idx;
            min_distance = distance;
        }
    }

    (normals, min_triangle)
}
