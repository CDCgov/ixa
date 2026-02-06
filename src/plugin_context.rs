use crate::context::{Context, ContextBase};
use crate::{
    ContextEntitiesExt, ContextGlobalPropertiesExt, ContextNetworkExt, ContextRandomExt,
    ContextReportExt,
};

/// A supertrait that exposes useful methods from [`Context`]
/// for plugins implementing [`Context`] extensions.
///
/// Usage:
// This example triggers the error "#[ctor]/#[dtor] is not supported
// on the current target," which appears to be spurious, so we
// ignore it.
/// ```ignore
/// use ixa::prelude_for_plugins::*;
/// define_data_plugin!(MyData, bool, false);
/// pub trait MyPlugin: PluginContext {
///     fn set_my_data(&mut self) {
///         let my_data = self.get_data_container_mut(MyData);
///         *my_data = true;
///     }
/// }
pub trait PluginContext:
    ContextBase
    + ContextRandomExt
    + ContextReportExt
    + ContextNetworkExt
    + ContextGlobalPropertiesExt
    + ContextEntitiesExt
{
}

impl PluginContext for Context {}

// Tests for the PluginContext trait, including:
// - Making sure all the methods work identically to Context
// - Defining a trait extension that uses it compiles correctly
// - External functions can use impl PluginContext if necessary
#[cfg(test)]
mod test_plugin_context {
    use crate::prelude_for_plugins::*;
    #[derive(Copy, Clone, IxaEvent)]
    struct MyEvent {
        pub data: usize,
    }

    define_data_plugin!(MyData, i32, 0);

    fn do_stuff_with_context(context: &mut impl PluginContext) {
        context.add_plan(1.0, |context| {
            let data = context.get_data(MyData);
            assert_eq!(*data, 42);
        });
    }

    trait MyDataExt: PluginContext {
        fn all_methods(&mut self) {
            assert_eq!(self.get_current_time(), 0.0);
        }
        fn all_methods_mut(&mut self) {
            self.setup();
            self.subscribe_to_event(|_: &mut Context, event: MyEvent| {
                assert_eq!(event.data, 42);
            });
            self.emit_event(MyEvent { data: 42 });
            self.add_plan_with_phase(
                1.0,
                |context| {
                    let data = context.get_data(MyData);
                    assert_eq!(*data, 42);
                    context.set_my_data(100);
                },
                crate::ExecutionPhase::Last,
            );
            self.add_plan(1.0, |context| {
                assert_eq!(context.get_my_data(), 42);
            });
            self.add_periodic_plan_with_phase(
                1.0,
                |context| {
                    println!(
                        "Periodic plan at time {} with data {}",
                        context.get_current_time(),
                        context.get_my_data()
                    );
                },
                crate::ExecutionPhase::Normal,
            );
            self.queue_callback(|context| {
                let data = context.get_data(MyData);
                assert_eq!(*data, 42);
            });
        }
        fn setup(&mut self) {
            let data = self.get_data_mut(MyData);
            *data = 42;
            do_stuff_with_context(self);
        }
        fn get_my_data(&self) -> i32 {
            *self.get_data(MyData)
        }
        fn set_my_data(&mut self, value: i32) {
            let data = self.get_data_mut(MyData);
            *data = value;
        }
        fn test_external_function(&mut self) {
            self.setup();
            do_stuff_with_context(self);
        }
    }
    impl MyDataExt for Context {}

    #[test]
    fn test_all_methods() {
        let mut context = Context::new();
        context.all_methods_mut();
        context.all_methods();
        context.execute();
    }

    #[test]
    fn test_plugin_context() {
        let mut context = Context::new();
        context.setup();
        assert_eq!(context.get_my_data(), 42);
    }

    #[test]
    fn test_external_function() {
        let mut context = Context::new();
        context.test_external_function();
        assert_eq!(context.get_my_data(), 42);
    }
}
