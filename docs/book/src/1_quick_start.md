# Quick Start
Execute the following commands to create a new Rust project called `disease_model` and add the Ixa library as a dependency.
```bash
cargo new --bin disease_model
cd disease_model
cargo add ixa --git https://github.com/CDCgov/ixa --branch release
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

To run the model:
```bash
cargo run
```

To run with logging enabled globally:
```bash
cargo run -- --log-level=trace
```

To run with logging enabled for just `disease_model`:
```bash
cargo run -- --log-level=disease_model:trace
```
