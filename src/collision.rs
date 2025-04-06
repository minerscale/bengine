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
use ultraviolet::{Isometry3, Vec3};

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
    pub fn new<T: BufRead>(file: T) -> Self {
        let mesh: RawObj = obj::raw::parse_obj(file).unwrap();

        let vertices: Rc<[Vec3]> = mesh
            .positions
            .iter()
            .map(|v| Vec3::from([v.0, v.1, v.2]))
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

fn line(simplex: &mut Simplex<Vec3, 4>, direction: &mut Vec3) -> bool {
    let a = simplex[0];
    let b = simplex[1];

    let ab = b - a;
    let ao = -a;

    if ab.dot(ao) > 0.0 {
        *direction = ab.cross(ao).cross(ab);
    } else {
        simplex.set(&[a]);
        *direction = ao;
    }

    false
}

fn triangle(simplex: &mut Simplex<Vec3, 4>, direction: &mut Vec3) -> bool {
    let a = simplex[0];
    let b = simplex[1];
    let c = simplex[2];

    let ab = b - a;
    let ac = c - a;
    let ao = -a;

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

fn tetrahedron(simplex: &mut Simplex<Vec3, 4>, direction: &mut Vec3) -> bool {
    let a = simplex[0];
    let b = simplex[1];
    let c = simplex[2];
    let d = simplex[3];

    let ab = b - a;
    let ac = c - a;
    let ad = d - a;
    let ao = -a;

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

pub fn collide<P: Collider<Vec3>, Q: Collider<Vec3>>(p: &P, q: &Q) -> Option<(Vec3, f32)> {
    gjk_intersection(p, q, Vec3::unit_x()).map(|simplex| epa(&simplex, p, q))
}

fn gjk_intersection<P: Collider<Vec3>, Q: Collider<Vec3>>(
    p: &P,
    q: &Q,
    mut direction: Vec3,
) -> Option<Simplex<Vec3, 4>> {
    let mut simplex = Simplex::<_, 4>::new();

    simplex.push_front(p.support(direction) - q.support(-direction));

    direction = -simplex[0];

    loop {
        simplex.push_front(p.support(direction) - q.support(-direction));

        if direction.dot(simplex[0]) <= 0.0 {
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
    }
    .then_some(simplex)
}

const EPA_EPSILON: f32 = 0.001;

fn epa<A: Collider<Vec3>, B: Collider<Vec3>>(
    simplex: &Simplex<Vec3, 4>,
    a: &A,
    b: &B,
) -> (Vec3, f32) {
    let mut polytope = simplex.points.to_vec();

    let mut faces: Vec<[usize; 3]> = vec![[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]];

    let (mut normals, mut min_face) = get_face_normals(&polytope, &faces);

    let mut min_normal = Vec3::default();
    let mut min_distance = None;

    while min_distance.is_none() {
        min_normal = normals[min_face].0;
        min_distance = Some(normals[min_face].1);

        let support = a.support(min_normal) - b.support(-min_normal);
        let s_distance = min_normal.dot(support);

        if min_distance.is_some_and(|d| (s_distance - d).abs() > EPA_EPSILON) {
            min_distance = None;

            let mut unique_edges: Vec<(usize, usize)> = Vec::new();

            let mut i = 0;
            while i < normals.len() {
                if (normals[i].0.dot(support) - normals[i].1) > 0.0 {
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

            let mut old_min_distance = None;
            for (idx, normal) in normals.iter().enumerate() {
                if old_min_distance.is_none_or(|d| normal.1 < d) {
                    old_min_distance = Some(normal.1);
                    min_face = idx;
                }
            }

            if old_min_distance.is_none_or(|d| new_normals[new_min_face].1 < d) {
                min_face = new_min_face + normals.len();
            }

            faces.extend(new_faces);
            normals.extend(new_normals);
        }
    }

    (min_normal, min_distance.unwrap() + 0.001)
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

fn get_face_normals(polytope: &[Vec3], faces: &[[usize; 3]]) -> (Vec<(Vec3, f32)>, usize) {
    let mut normals = Vec::new();

    let mut min_triangle = 0;
    let mut min_distance = None;

    for (face_idx, face) in faces.iter().enumerate() {
        let a = polytope[face[0]];
        let b = polytope[face[1]];
        let c = polytope[face[2]];

        let mut normal = (b - a).cross(c - a).normalized();
        let mut distance = normal.dot(a);

        if distance < 0.0 {
            normal *= -1.0;
            distance *= -1.0;
        }

        normals.push((normal, distance));

        if min_distance.is_none_or(|d| distance < d) {
            min_triangle = face_idx;
            min_distance = Some(distance);
        }
    }

    (normals, min_triangle)
}
