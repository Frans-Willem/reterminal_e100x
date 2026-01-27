use nalgebra::base::{Matrix4, Scalar, Vector4};
use nalgebra::geometry::Point3;
use nalgebra::{ClosedAddAssign, ClosedDivAssign, ClosedMulAssign, ComplexField};
use num_traits::identities::{One, Zero};

pub struct TetrahedronProjector<T: Scalar> {
    to_barycentric: Matrix4<T>,
    from_barycentric: Matrix4<T>,
}

impl<T: Scalar + ComplexField + ClosedMulAssign + ClosedAddAssign + ClosedDivAssign + Zero + One>
    TetrahedronProjector<T>
{
    pub fn new(vertices: [Point3<T>; 4]) -> Self {
        // Method used:
        // Create a matrix from barycentric coordinates to [x,y,z,1]
        // of the following form:
        // [ x1 x2 x3 x4 ]
        // [ y1 y2 y3 y4 ]
        // [ z1 z2 z3 z4 ]
        // [ 1  1  1  1  ]
        let from_barycentric: Matrix4<T> =
            Matrix4::from_columns(&vertices.map(|x| x.to_homogeneous()));
        let to_barycentric: Matrix4<T> = from_barycentric.clone().try_inverse().unwrap();
        TetrahedronProjector {
            to_barycentric,
            from_barycentric,
        }
    }

    pub fn project(&self, pt: &Point3<T>) -> Vector4<T> {
        &self.to_barycentric * pt.to_homogeneous()
    }

    pub fn bary_to_point(&self, barycentric_coords: &Vector4<T>) -> Point3<T> {
        Point3::from_homogeneous(&self.from_barycentric * barycentric_coords).unwrap()
    }
}
