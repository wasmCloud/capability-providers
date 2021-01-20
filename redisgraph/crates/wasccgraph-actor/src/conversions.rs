use crate::client_type_error;
use crate::errors::GraphResult;
use crate::results::{get_node, get_relation, get_scalar, FromCell};

use actor_graphdb::generated::{Node, Relation, ResultSet, Scalar};

impl FromCell for Scalar {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
        let scalar = get_scalar(result_set, row_idx, column_idx)?;
        Ok(scalar.clone())
    }
}

impl<T: FromCell> FromCell for Option<T> {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
        let scalar = get_scalar(result_set, row_idx, column_idx)?;

        match scalar {
            _ => T::from_cell(result_set, row_idx, column_idx).map(Some),
        }
    }
}

impl FromCell for bool {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
        let scalar = get_scalar(result_set, row_idx, column_idx)?;
        if let Some(boolean) = scalar.bool_value {
            Ok(boolean)
        } else {
            client_type_error!("failed to construct value: expected boolean, found None",)
        }
    }
}

// The following code and macros produce the requisite type "magic" to allow
// code in an actor to extract strongly-typed data from a result set in
// tuples (or vecs of tuples)

macro_rules! impl_from_scalar_for_integer {
    ($t:ty) => {
        impl FromCell for $t {
            fn from_cell(
                result_set: &ResultSet,
                row_idx: usize,
                column_idx: usize,
            ) -> GraphResult<Self> {
                let scalar = get_scalar(result_set, row_idx, column_idx)?;
                if let Some(int) = scalar.int_value {
                    Ok(int as $t)
                } else {
                    client_type_error!("failed to construct value: expected integer, found None",)
                }
            }
        }
    };
}

impl_from_scalar_for_integer!(u8);
impl_from_scalar_for_integer!(u16);
impl_from_scalar_for_integer!(u32);
impl_from_scalar_for_integer!(u64);
impl_from_scalar_for_integer!(usize);

impl_from_scalar_for_integer!(i8);
impl_from_scalar_for_integer!(i16);
impl_from_scalar_for_integer!(i32);
impl_from_scalar_for_integer!(i64);
impl_from_scalar_for_integer!(isize);

macro_rules! impl_from_scalar_for_float {
    ($t:ty) => {
        impl FromCell for $t {
            fn from_cell(
                result_set: &ResultSet,
                row_idx: usize,
                column_idx: usize,
            ) -> GraphResult<Self> {
                let scalar = get_scalar(result_set, row_idx, column_idx)?;
                if let Some(double) = scalar.double_value {
                    Ok(double as $t)
                } else {
                    client_type_error!("failed to construct value: expected double, found None",)
                }
            }
        }
    };
}

impl_from_scalar_for_float!(f32);
impl_from_scalar_for_float!(f64);

impl FromCell for String {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
        let scalar = get_scalar(result_set, row_idx, column_idx)?;
        if let Some(data) = &scalar.string_value {
            Ok(data.clone())
        } else {
            client_type_error!("failed to construct value: expected string, found None",)
        }
    }
}

// impl FromCell for String {
//     fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
//         let redis_string = GraphString::from_cell(result_set, row_idx, column_idx)?;
//         String::from_utf8(redis_string.into()).map_err(|_| GraphError::InvalidUtf8)
//     }
// }

impl FromCell for Node {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
        let node = get_node(result_set, row_idx, column_idx)?;
        Ok(node.clone())
    }
}

impl FromCell for Relation {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self> {
        let relation = get_relation(result_set, row_idx, column_idx)?;
        Ok(relation.clone())
    }
}
