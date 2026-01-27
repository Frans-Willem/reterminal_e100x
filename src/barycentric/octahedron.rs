use crate::barycentric::line::LineProjector;
use crate::barycentric::tetrahedron::TetrahedronProjector;
use crate::barycentric::triangle::TriangleProjector;
use nalgebra::base::{Scalar, Vector2, Vector3, Vector4, Vector6};
use nalgebra::geometry::Point3;
use nalgebra::{ClosedAddAssign, ClosedDivAssign, ClosedMulAssign, ClosedSubAssign, ComplexField};
use num_traits::identities::{One, Zero};
use num_traits::zero;

pub struct OctahedronProjector<T: Scalar> {
    /*
     * Projector for each of the 4 wedges. Each wedge goes from north to south, subsequently from a
     * to b, b to c, c to d, d to a.
     */
    wedges: [TetrahedronProjector<T>; 4],
    /*
     * Projector for each of the 8 faces. Each face goes from pole -> a -> b, pole -> b -> c, etc.
     * First 4 faces go from north (first) pole, second 4 from south (second) pole.
     */
    faces: [TriangleProjector<T>; 8],
    /*
     * Projector for each of the edges from from or to a pole. First 4 go from north (first) pole to
     * each equatorial vertex, second 4 go from south (second) pole to each equatorial vertex, last
     * 4 go between the equatorial vertices (e.g. a->b, b->c, c->d, d->a).
     */
    edges: [LineProjector<T>; 12],
}

impl<
    T: Scalar
        + ClosedSubAssign
        + ClosedMulAssign
        + ClosedAddAssign
        + ClosedDivAssign
        + Zero
        + One
        + ComplexField
        + PartialOrd,
> OctahedronProjector<T>
{
    pub fn new(vertices: [Point3<T>; 6]) -> Self {
        /*
         * Vertex input ordering should be the two opposing poles first, then the other vertices in
         * cyclical order
         */
        let wedges: [TetrahedronProjector<T>; 4] = core::array::from_fn(|i| {
            TetrahedronProjector::new([
                vertices[0].clone(),
                vertices[1].clone(),
                vertices[2 + (i % 4)].clone(),
                vertices[2 + ((i + 1) % 4)].clone(),
            ])
        });
        let faces: [TriangleProjector<T>; 8] = core::array::from_fn(|i| {
            // First four faces go from north, rest from south
            let pole = i / 4;
            TriangleProjector::new([
                vertices[pole].clone(),
                vertices[2 + (i % 4)].clone(),
                vertices[2 + ((i + 1) % 4)].clone(),
            ])
        });

        let edges: [LineProjector<T>; 12] = core::array::from_fn(|i| {
            // First four edges go from north, rest from south
            let pole_index = i / 4;
            let equator_index = i % 4;
            if pole_index < 2 {
                LineProjector::new([
                    vertices[pole_index].clone(),
                    vertices[2 + equator_index].clone(),
                ])
            } else {
                LineProjector::new([
                    vertices[2 + (equator_index % 4)].clone(),
                    vertices[2 + ((equator_index + 1) % 4)].clone(),
                ])
            }
        });

        OctahedronProjector {
            wedges,
            faces,
            edges,
        }
    }

    fn wedge_barycentric_local_to_global(index: usize, local: Vector4<T>) -> Vector6<T> {
        // Wedge from north (0), south (1), a (2 + index), b (2 + ((index+1)%4)
        let [north, south, a, b] = local.into();
        let mut ret = Vector6::new(north, south, zero(), zero(), zero(), zero());
        let a_index = 2 + (index % 4);
        let b_index = 2 + ((index + 1) % 4);
        ret[a_index] = a;
        ret[b_index] = b;
        ret
    }

    fn face_barycentric_local_to_global(index: usize, local: Vector3<T>) -> Vector6<T> {
        //Black, White, Blue, Green, Yellow, Red (last 2 swapped)
        let [pole, a, b] = local.into();
        let mut ret: Vector6<T> = zero();
        ret[index / 4] = pole;
        ret[2 + (index % 4)] = a;
        ret[2 + ((index + 1) % 4)] = b;
        ret
    }

    fn edge_barycentric_local_to_global(index: usize, local: Vector2<T>) -> Vector6<T> {
        //Black, White, Blue, Green, Yellow, Red (last 2 swapped)
        //
        let [a, b] = local.into();
        let mut ret: Vector6<T> = zero();
        let pole_index = index / 4;
        let equator_index = index % 4;
        if pole_index < 2 {
            // There's an error here
            ret[pole_index] = a;
            ret[2 + equator_index] = b;
        } else {
            ret[2 + equator_index] = a;
            ret[2 + ((equator_index + 1) % 4)] = b;
        }
        ret
    }

    pub fn project(&self, pt: &Point3<T>) -> Vector6<T> {
        let mut edges_to_check: [bool; 12] = [false; 12];
        let mut best: Option<(Vector6<T>, T)> = None;
        for (wedge_index, wedge) in self.wedges.iter().enumerate() {
            // Wedge from north (0), south (1), a (2 + wedge_index), b (2 + ((wedge_index+1)%4)
            let barycentric_local: Vector4<T> = wedge.project(pt);
            let barycentric_local_min = barycentric_local.min();
            if barycentric_local_min >= zero() {
                // Point lies in this wedge, so convert to global barycentric coordinates and we're
                // off to the races!
                return Self::wedge_barycentric_local_to_global(wedge_index, barycentric_local);
            }
            // In case a point lies in the rounding errors between the wedges, keep track of the one
            // with the highest minimum barycentric coordinate (e.g. the one closest to 0)
            if best
                .as_ref()
                .map(|best| best.1 < barycentric_local_min)
                .unwrap_or(true)
            {
                best = Some((
                    Self::wedge_barycentric_local_to_global(wedge_index, barycentric_local.clone()),
                    barycentric_local_min,
                ));
            }
            // The faces north->south->a and north->south->b aren't relevant, as they meet with one
            // of the other wedges there. The faces north->a->b and south->a->b are, as these are
            // outside faces. If a barycentric coordinate is negative, this means the point lies on
            // the other side of the opposite face. Thus if the barycentric coordinate for north is
            // negative, the point lies outside south->a->b
            for pole in 0..2 {
                if barycentric_local[pole] <= zero() {
                    let other_pole = 1 - pole;
                    let face_index = (other_pole * 4) + wedge_index;
                    let face = &self.faces[face_index];
                    let barycentric_local = face.project(pt);
                    if barycentric_local.min() >= zero() {
                        // Point lies outside the octahedron, but projects cleanly onto this face,
                        // meaning the point on this face is the closest to it! (as long as all faces are
                        // convex)
                        return Self::face_barycentric_local_to_global(
                            face_index,
                            barycentric_local,
                        );
                    }
                    if barycentric_local[0] <= zero() {
                        // Barycentric coordinate for pole <0 means the projected point lies on or beyond
                        // the equatorial edge.
                        edges_to_check[8 + (face_index % 4)] = true;
                    }
                    for equator_vertex_index in 0..2 {
                        if barycentric_local[1 + equator_vertex_index] <= zero() {
                            // Barycentric coordinate for one of the equatorial edges <0 means the projected
                            // point lies on or beyond the opposite pole_edge.
                            let pole_index = face_index / 4;
                            let other_equator_vertex_index =
                                ((face_index % 4) + (1 - equator_vertex_index)) % 4;
                            edges_to_check[pole_index * 4 + other_equator_vertex_index] = true;
                        }
                    }
                }
            }
        }
        // At this point, neither of the wedges indicate the point is in there, or on any of it's
        // outside faces, and we kept a list
        // of the edges that should be checked. If this list is empty, then the point probably
        // lies somewhere in the rounding errors between the wedges.
        if edges_to_check.iter().all(|to_check| !*to_check) {
            let (mut best, _) = best.unwrap();
            // Set all coordinates <0 to 0
            for coord in best.iter_mut() {
                if *coord < zero() {
                    *coord = zero()
                }
            }
            // Normalize such that the sum will be 1 again
            let sum = best.sum();
            if sum > zero() {
                best /= sum;
            }
            return best;
        }
        let mut best: Option<(Vector6<T>, T::RealField)> = None;
        for (edge_index, edge) in self.edges.iter().enumerate() {
            if !edges_to_check[edge_index] {
                continue;
            }
            let (barycentric_local, _) = edge.clipping_project(pt);
            // Keep track of best so far
            let distance = (edge.bary_to_point(&barycentric_local) - pt).norm_squared();
            if best.as_ref().map(|best| best.1 > distance).unwrap_or(true) {
                best = Some((
                    Self::edge_barycentric_local_to_global(edge_index, barycentric_local),
                    distance,
                ));
            }
        }
        // Point feel between all of the line projections with rounding errors, what to do?!
        best.unwrap().0
    }
}
