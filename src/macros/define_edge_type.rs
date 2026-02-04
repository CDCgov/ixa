/// Define a new edge type for use with [`network`](crate::network).
///
/// Defines a new edge type of type `$edge_type`, with inner type `$value`.
/// Use `()` for `$value` to have no inner type.
#[allow(unused_macros)]
#[macro_export]
macro_rules! define_edge_type {
    ($edge_type:ident, $value:ty) => {
        #[derive(Debug, Copy, Clone)]
        pub struct $edge_type;

        impl $crate::network::EdgeType for $edge_type {
            type Value = $value;
        }
    };
}
