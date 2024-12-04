use clap::Args;
use ixa::runner::run_with_custom_args;

#[derive(Args, Debug)]
struct Extra {
    #[arg(short, long)]
    field: u32,
}

fn main() {
    run_with_custom_args(|_context, _args, extra: Option<Extra>| {
        if let Some(extra) = extra {
            println!("{}", extra.field);
        }
        Ok(())
    })
    .unwrap();
}
