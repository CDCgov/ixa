/// Tracks periodic value change counts with concise entity, property, and strata syntax.
///
/// The strata list is optional. Omitting it, or passing `[]`, uses the empty
/// property list `()`. The period may be any value that converts into `f64`.
///
/// ```ignore
/// track_periodic_value_change_counts!(
///     context,
///     Person,
///     InfectionStatus,
///     1.0,
///     handle_incidence_tracking
/// );
///
/// track_periodic_value_change_counts!(
///     context,
///     Person,
///     InfectionStatus,
///     [Age],
///     1.0,
///     move |_context, counter| {
///         let _ = counter;
///     }
/// );
///
/// track_periodic_value_change_counts!(
///     context,
///     Person,
///     InfectionStatus,
///     [Age, HighRisk],
///     1.0,
///     handle_incidence_tracking
/// );
/// ```
#[macro_export]
macro_rules! track_periodic_value_change_counts {
    ($context:expr, $entity:ty, $property:ty, [$($stratum:ty),* $(,)?], $period:expr, $handler:expr $(,)?) => {
        ($context).track_periodic_value_change_counts::<$entity, ($($stratum,)*), $property, _>(
            $period,
            $handler,
        )
    };

    ($context:expr, $entity:ty, $property:ty, $period:expr, $handler:expr $(,)?) => {
        track_periodic_value_change_counts!($context, $entity, $property, [], $period, $handler)
    };
}
