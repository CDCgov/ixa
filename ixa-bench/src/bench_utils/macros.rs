#[macro_export]
macro_rules! hyperfine_group {
    (
        $group_name:ident {
            $(
                $bench_name:ident => $block:block
            ),* $(,)?
        }
    ) => {
        paste::paste! {
            pub mod $group_name {
                use super::*;
                use $crate::bench_utils::registry;

                pub fn run_bench_group(bench_str: &str) -> anyhow::Result<()> {
                    match bench_str {
                        $( stringify!($bench_name) => $block ),*,
                        _ => anyhow::bail!("Unknown benchmark: {}", bench_str)
                    }
                    Ok(())
                }

                #[ctor::ctor]
                fn [<_register_group_$group_name:snake>]() {
                    registry::register_group(stringify!($group_name), registry::BenchGroupEntry {
                        name: stringify!($group_name),
                        bench_names: vec![$(stringify!($bench_name)),*],
                        runner: |bench_str| -> registry::Result<()> {
                            run_bench_group(bench_str)
                        }
                    });
                }

            }

        }
    };
}
