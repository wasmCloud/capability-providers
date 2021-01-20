use crate::client_type_error;
use crate::errors::GraphResult;

pub const CAPID_GRAPHDB: &str = "wasmcloud:graphdb";

use actor_graphdb::generated::*;

/// Returns the number of columns in the result set.
pub fn num_columns(rs: &ResultSet) -> usize {
    rs.columns.len()
}

/// Returns the number of rows in the result set.
pub fn num_rows(rs: &ResultSet) -> usize {
    if let Some(col) = rs.columns.get(0) {
        col.scalars.len() + col.relations.len() + col.nodes.len()
    } else {
        0
    }
}

/// Returns the scalar at the given position.
///
/// Returns an error if the value at the given position is not a scalar
/// or if the position is out of bounds.
pub fn get_scalar(rs: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<&Scalar> {
    match rs.columns.get(column_idx) {
            Some(column) => match column.scalars.get(row_idx) {
                Some(scalar) => Ok(scalar),
                None => client_type_error!(
                    "failed to get scalar: row index out of bounds: the len is {:?} but the index is {:?}", column.scalars.len(), row_idx,
                ),
            },
            None => client_type_error!(
                "failed to get scalar: column index out of bounds: the len is {:?} but the index is {:?}", rs.columns.len(), column_idx,
            ),
        }
}

/// Returns the node at the given position.
///
/// Returns an error if the value at the given position is not a node
/// or if the position is out of bounds.
pub fn get_node(rs: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<&Node> {
    match rs.columns.get(column_idx) {
            Some(column) => match column.nodes.get(row_idx) {
                Some(node) => Ok(node),
                None => client_type_error!(
                    "failed to get node: row index out of bounds: the len is {:?} but the index is {:?}", column.nodes.len(), row_idx,
                ),
            },
            None => client_type_error!(
                "failed to get node: column index out of bounds: the len is {:?} but the index is {:?}", rs.columns.len(), column_idx,
            ),
        }
}

/// Returns the relation at the given position.
///
/// Returns an error if the value at the given position is not a relation
/// or if the position is out of bounds.
pub fn get_relation(rs: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<&Relation> {
    match rs.columns.get(column_idx) {
            Some(column) => match column.relations.get(row_idx) {
                Some(relation) => Ok(relation),
                None => client_type_error!(
                    "failed to get relation: row index out of bounds: the len is {:?} but the index is {:?}", column.relations.len(), row_idx,
                ),
            },
            None => client_type_error!(
                "failed to get relation: column index out of bounds: the len is {:?} but the index is {:?}", rs.columns.len(), column_idx,
            ),
        }
}

impl FromTable for ResultSet {
    fn from_table(result_set: &ResultSet) -> GraphResult<Self> {
        Ok(result_set.clone())
    }
}

impl<T: FromRow> FromTable for Vec<T> {
    fn from_table(result_set: &ResultSet) -> GraphResult<Self> {
        let num_rows = num_rows(result_set);
        let mut ret = Self::with_capacity(num_rows);

        for i in 0..num_rows {
            ret.push(T::from_row(result_set, i)?);
        }

        Ok(ret)
    }
}

pub trait FromTable: Sized {
    fn from_table(result_set: &ResultSet) -> GraphResult<Self>;
}

/// Implemented by types that can be constructed from a row in a [`ResultSet`](../result_set/struct.ResultSet.html).
pub trait FromRow: Sized {
    fn from_row(result_set: &ResultSet, row_idx: usize) -> GraphResult<Self>;
}

/// Implemented by types that can be constructed from a cell in a [`ResultSet`](../result_set/struct.ResultSet.html).
pub trait FromCell: Sized {
    fn from_cell(result_set: &ResultSet, row_idx: usize, column_idx: usize) -> GraphResult<Self>;
}

// Macro generates generic "From" implementations to allow
// tuples/vecs-of-tuples to be extracted from various types
//
// Altered version of https://github.com/mitsuhiko/redis-rs/blob/master/src/types.rs#L1080
macro_rules! impl_row_for_tuple {
    () => ();
    ($($name:ident,)+) => (
        #[doc(hidden)]
        impl<$($name: FromCell),*> FromRow for ($($name,)*) {
            // we have local variables named T1 as dummies and those
            // variables are unused.
            #[allow(non_snake_case, unused_variables, clippy::eval_order_dependence)]
            fn from_row(result_set: &ResultSet, row_idx: usize) -> GraphResult<($($name,)*)> {
                // hacky way to count the tuple size
                let mut n = 0;
                $(let $name = (); n += 1;)*
                if num_columns(result_set) != n {
                    return client_type_error!(
                        "failed to construct tuple: tuple has {:?} entries but result table has {:?} columns",
                        n,
                        num_columns(result_set)
                    );
                }

                // this is pretty ugly too. The { i += 1; i - 1 } is rust's
                // postfix increment :)
                let mut i = 0;
                Ok(($({let $name = (); $name::from_cell(result_set, row_idx, { i += 1; i - 1 })?},)*))
            }
        }
        impl_row_for_tuple_peel!($($name,)*);
    )
}

// Support for the recursive macro calls
macro_rules! impl_row_for_tuple_peel {
    ($name:ident, $($other:ident,)*) => (impl_row_for_tuple!($($other,)*);)
}

// The library supports tuples of up to 12 items
impl_row_for_tuple! { T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, }

// Row and column indices default to zero for lower-level values
impl<T: FromCell> FromRow for T {
    fn from_row(result_set: &ResultSet, row_idx: usize) -> GraphResult<Self> {
        T::from_cell(result_set, row_idx, 0)
    }
}

impl<T: FromRow> FromTable for T {
    fn from_table(result_set: &ResultSet) -> GraphResult<Self> {
        T::from_row(result_set, 0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    // Verifies that we can extract the tuples we expect from the raw ResultSet
    // structure and that the various return types are automatically converted
    #[test]
    fn tuple_extraction_test() {
        let (name, birth_year): (String, u32) = fake_query("fake query").unwrap();
        assert_eq!("tester", name);
        assert_eq!(1985, birth_year);
    }

    #[test]
    fn vec_tuple_extraction_test() {
        let res: Vec<(String, u32)> = fake_vec_query("foo").unwrap();
        assert_eq!(("tester".to_string(), 1985), res[0]);
        assert_eq!(("test2".to_string(), 1986), res[1]);
    }

    fn fake_vec_query<T: FromTable>(_query: &str) -> GraphResult<T> {
        query_with_statistics2().map(|(value, _)| value)
    }

    fn fake_query<T: FromTable>(_query: &str) -> GraphResult<T> {
        query_with_statistics().map(|(value, _)| value)
    }

    fn query_with_statistics<T: FromTable>() -> GraphResult<(T, Vec<String>)> {
        let result_set = get_result_set()?;
        let value = T::from_table(&result_set)?;
        Ok((value, result_set.statistics))
    }

    fn query_with_statistics2<T: FromTable>() -> GraphResult<(T, Vec<String>)> {
        let result_set = get_result_set2()?;
        let value = T::from_table(&result_set)?;
        Ok((value, result_set.statistics))
    }

    fn get_result_set() -> GraphResult<ResultSet> {
        Ok(ResultSet {
            statistics: vec![],
            columns: vec![
                Column {
                    scalars: vec![Scalar {
                        bool_value: None,
                        double_value: None,
                        int_value: None,
                        string_value: Some("tester".to_string()),
                    }],
                    nodes: vec![],
                    relations: vec![],
                },
                Column {
                    scalars: vec![Scalar {
                        bool_value: None,
                        double_value: None,
                        int_value: Some(1985),
                        string_value: None,
                    }],
                    nodes: vec![],
                    relations: vec![],
                },
            ],
        })
    }

    fn get_result_set2() -> GraphResult<ResultSet> {
        Ok(ResultSet {
            statistics: vec![],
            columns: vec![
                Column {
                    scalars: vec![
                        Scalar {
                            bool_value: None,
                            double_value: None,
                            int_value: None,
                            string_value: Some("tester".to_string()),
                        },
                        Scalar {
                            bool_value: None,
                            double_value: None,
                            int_value: None,
                            string_value: Some("test2".to_string()),
                        },
                    ],
                    nodes: vec![],
                    relations: vec![],
                },
                Column {
                    scalars: vec![
                        Scalar {
                            bool_value: None,
                            double_value: None,
                            int_value: Some(1985),
                            string_value: None,
                        },
                        Scalar {
                            bool_value: None,
                            double_value: None,
                            int_value: Some(1986),
                            string_value: None,
                        },
                    ],
                    nodes: vec![],
                    relations: vec![],
                },
            ],
        })
    }
}
