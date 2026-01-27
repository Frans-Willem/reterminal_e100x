use nalgebra::base::{Matrix2x3, Matrix3, Scalar, Vector2, Vector3};
use nalgebra::geometry::Point3;
use nalgebra::{ClosedAddAssign, ClosedDivAssign, ClosedMulAssign, ClosedSubAssign, ComplexField};
use num_traits::identities::{One, Zero};
use num_traits::{one, zero};

use crate::barycentric::line::LineProjector;

pub struct TriangleProjector<T: Scalar> {
    v1: Point3<T>,
    project_matrix: Matrix2x3<T>,
}

impl<
    T: Scalar
        + ComplexField
        + ClosedSubAssign
        + ClosedMulAssign
        + ClosedAddAssign
        + ClosedDivAssign
        + Zero
        + One
        + PartialOrd,
> TriangleProjector<T>
{
    pub fn new(vertices: [Point3<T>; 3]) -> Self {
        // Method used:
        // Moeller-Trumbore intersection algorithm
        // https://en.wikipedia.org/wiki/M%C3%B6ller%E2%80%93Trumbore_intersection_algorithm

        // Triangle plane defined as
        // P = w*v1 + u*v2 + v * v3
        let [v1, v2, v3] = vertices;

        let v1_to_v2: Vector3<T> = v2 - &v1;
        let v1_to_v3: Vector3<T> = v3 - &v1;
        let normal: Vector3<T> = v1_to_v2.cross(&v1_to_v3);
        let neg_normal: Vector3<T> = zero::<Vector3<T>>() - normal;

        // Matrix such that premul * [t,u,v] = P - v1
        let mut premul: Matrix3<T> = zero();
        premul.set_column(0, &neg_normal);
        premul.set_column(1, &v1_to_v2);
        premul.set_column(2, &v1_to_v3);

        // Matrix such that [t,u,v] = (P - v1) * project_matrix
        let project_matrix_tuv: Matrix3<T> = premul.try_inverse().unwrap();

        // Drop the row that would calculate t, as we're very rarely interested in it
        let project_matrix = project_matrix_tuv.fixed_view::<2, 3>(1, 0);
        let project_matrix: Matrix2x3<T> = project_matrix.clone_owned();
        TriangleProjector { v1, project_matrix }
    }

    pub fn project(&self, pt: &Point3<T>) -> Vector3<T> {
        let v1_to_pt: Vector3<T> = pt - &self.v1;
        let uv: Vector2<T> = &self.project_matrix * v1_to_pt;
        let [u, v] = uv.into();
        let w: T = one::<T>() - u.clone() - v.clone();
        // Triangle plane defined as
        // P = w*v1 + u*v2 + v * v3
        Vector3::new(w, u, v)
    }
}

pub struct ClippingTriangleProjector<T: Scalar> {
    vertices: Matrix3<T>, // Each column is a vertex, such that vertices * barycentric == point
    lines: [LineProjector<T>; 3], // Line x is the line from vertex[(x+1)%3] to vertices[(x+2)%3]
    normal_project: TriangleProjector<T>,
}
impl<
    T: Scalar
        + ComplexField
        + ClosedSubAssign
        + ClosedMulAssign
        + ClosedAddAssign
        + ClosedDivAssign
        + Zero
        + One
        + PartialOrd,
> ClippingTriangleProjector<T>
{
    pub fn new(vertices: [Point3<T>; 3]) -> Self {
        let lines = [0, 1, 2].map(|i| {
            LineProjector::new([vertices[(i + 1) % 3].clone(), vertices[(i + 2) % 3].clone()])
        });
        let normal_project = TriangleProjector::new(vertices.clone());
        let vertices: Matrix3<T> = Matrix3::from_columns(&vertices.map(|x| x.coords));
        ClippingTriangleProjector {
            vertices,
            lines,
            normal_project,
        }
    }

    pub fn project(&self, pt: &Point3<T>) -> Vector3<T> {
        self.normal_project.project(pt)
    }

    pub fn bary_to_point(&self, barycentric_coords: &Vector3<T>) -> Point3<T> {
        Point3::from(&self.vertices * barycentric_coords)
    }

    // Returns barycentric coordinates, if it was clipped, and (if already calculated) distance^2
    pub fn clipping_project(&self, pt: &Point3<T>) -> (Vector3<T>, bool, Option<T::RealField>) {
        let barycentric: Vector3<T> = self.project(pt);
        if barycentric.min() >= zero() {
            // Inside the triangle, no need to clip, hurrah!
            return (barycentric, false, None);
        }
        let best_barycentric: Vector3<T> = barycentric
            .clone()
            .map(|x| if x < zero() { zero() } else { x });
        let best_barycentric_sum = best_barycentric.sum();
        // TODO: Potential division by zero here, what do to?
        let mut best_barycentric = best_barycentric / best_barycentric_sum;
        let mut best_distance_sq = (self.bary_to_point(&best_barycentric) - pt).norm_squared();
        for index in 0..3 {
            if barycentric[index] < zero() {
                // If the barycentric coordinate for a point is negative,
                // this means the point is behind the opposing line
                // Find the closest point on that line.
                let (line_barycentric, line_clipped) = self.lines[index].clipping_project(pt);
                let mut candidate_barycentric: Vector3<T> = zero();
                candidate_barycentric[(index + 1) % 3] = line_barycentric[0].clone();
                candidate_barycentric[(index + 2) % 3] = line_barycentric[1].clone();
                if !line_clipped {
                    // The point projected on the line fell cleanly between the endpoints,
                    // which indicates that this is indeed the closest point on the triangle.
                    // We can return it right away, no need to keep looking.
                    return (candidate_barycentric, true, None);
                } else {
                    // The point projected on the line fell outside the line, so was clipped to one
                    // of the endpoints. This doesn't garantuee that this is the best point, so
                    // save it and keep looking
                    let candidate_distance_sq =
                        (self.bary_to_point(&candidate_barycentric) - pt).norm_squared();
                    if candidate_distance_sq < best_distance_sq {
                        best_distance_sq = candidate_distance_sq;
                        best_barycentric = candidate_barycentric;
                    }
                }
            }
        }
        (best_barycentric, true, Some(best_distance_sq))
    }
}
