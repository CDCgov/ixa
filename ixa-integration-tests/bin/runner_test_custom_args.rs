use clap::Args;
use ixa::prelude::*;

#[derive(Args, Debug)]
struct Extra {
    #[arg(short, long)]
    a: u32,
}

fn main() {
    run_with_custom_args(|_context, _args, extra: Option<Extra>| {
        if let Some(extra) = extra {
            println!("{}", extra.a);
        }
        Ok(())
    })
    .unwrap();
}
