# Quick Start
Execute the following commands to create a new Rust project called `disease_model` and add the Ixa library as a dependency.
```bash
cargo new --bin disease_model
cd disease_model
cargo add ixa
```

Open `src/main.rs` in your favorite editor or IDE and modify it to look like the following:
```rust
use ixa::{error, info, run_with_args, trace, Context};  
  
fn main() {  
    let result =  
        run_with_args(|_context: &mut Context, _args, _| {  
            trace!("Initializing disease_model");  
            Ok(())  
        });  
  
    match result {  
        Ok(_) => {  
            info!("Simulation finished executing");  
        }  
        Err(e) => {  
            error!("Simulation exited with error: {}", e);  
        }  
    }  
}
```

## Using Ixa From Source
The recommended way to use Ixa is by including it as a dependency in your model project's `Cargo.toml` file like any other dependency, but there are situations when you may want the latest pre-release Ixa source code. 
### Clone the Ixa repo to your local machine.

Open a terminal and change directories to wherever you want Ixa's source code to reside. Then run this command:

```bash
git clone https://github.com/CDCgov/ixa.git
```

There should now be a new directory called `ixa`. 

### Check that Ixa can compile and run.

Change directory to the new `ixa` directory and try to run an example:

```bash
cd ixa
cargo run --example basic-infection
```

While this example doesn't display much, it should run without any errors. 
