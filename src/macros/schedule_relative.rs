/// Schedules an action after a delay relative to the context's current time.
///
/// The scheduled action is called with the executing `context: &mut Context` as
/// its first argument, followed by each remaining macro argument in the same
/// order.
///
/// For example:
///
/// ```ignore
/// schedule_relative!(context, my_delay, my_handler, arg1, arg2, arg3);
/// ```
///
/// expands to code like:
///
/// ```ignore
/// {
///     let time = context.get_current_time() + my_delay;
///     context.add_plan(time, move |context| {
///         my_handler(context, arg1, arg2, arg3)
///     })
/// }
/// ```
#[macro_export]
macro_rules! schedule_relative {
    ($context:expr, $delay:expr, $action:expr $(, $arg:expr)* $(,)?) => {
        {
            let time = ($context).get_current_time() + $delay;
            ($context).add_plan(time, move |context| {
                ($action)(context $(, $arg)*)
            })
        }
    };
}
