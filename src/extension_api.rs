use crate::Context;

trait Extension {
    type Args;
    type Result;

    fn run(context: &mut Context, args: &Self::Args) -> Self::Result;
}
