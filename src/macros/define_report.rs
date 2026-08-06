/// Use this macro to define a unique report type
#[macro_export]
macro_rules! define_report {
    ($name:ident) => {
        impl $crate::Report for $name {
            fn type_id(&self) -> std::any::TypeId {
                std::any::TypeId::of::<$name>()
            }

            fn serialize(&self, writer: &mut $crate::csv::Writer<std::fs::File>) {
                writer.serialize(self).unwrap();
            }
        }
    };
}
