# Setting Up Your First Model

Let's setup the bare bones skeleton of our first model. First decide where your Ixa-related code is going to live on your computer. It's probably easiest to just have your models in the same folder as the Ixa source repository if you are using the Ixa library from source. On my computer, that's the `Code` directory in my `home` folder (or `~` for short). I will use my directory structure for illustration purposes in this section. Just modify the commands for wherever you chose to store your models.

```
🏠 home/
└── 🗂️ Code/
    ├── 🗂️ ixa/           (Ixa source repository)
    └── 🗂️ disease_model/ (Our first model)
```

## Create a new project with Cargo
Navigate to the directory you have chosen for your models and then use `Cargo` to initialize a new Rust project called `disease_model`.

```bash
cd ~/Code
cargo new --bin disease_model
```

Cargo creates a directory named `disease_model` with a project skeleton for us. Open the newly created `disease_model` directory in your favorite IDE, like [VSCode](https://code.visualstudio.com/) (free) or [RustRover](https://www.jetbrains.com/rust/).

```
🗂️ disease_model/
├── .git/
├── 🗂️ src/
│   └── 📄 main.rs
├── .gitignore
└── 📄 Cargo.toml
```

> [!INFO] Source Control
> The `.gitignore` file lists all the files and directories you don't want to include in source control. For a Rust project you should at least have `target` and `Cargo.lock` listed in the `.gitignore`. I also make a habit of listing `.vscode` and `.idea`, the directories VS Code and JetBrains respectively store IDE project settings. The `.git` directory is where Git stores its database. It is automatically ignored. You shouldn't touch this directory.
>
> For simplicity I will suppress these hidden files and directories from now on.

## Setup `Cargo.toml`
The `Cargo.toml` file stores information about your project, including metadata used when publishing your project to Crates.io and which libraries your project depends on. Even though we won't be publishing the crate to Crates.io, it's a good idea to get into the habit of adding at least the author(s) and a brief description of the project.
```toml
# Cargo.toml
{{#include ../../models/disease_model/Cargo.toml}}
```
We also specify our dependencies. In this case, I am telling Cargo to use the version of the Ixa library located on my computer at a particular path. If you are using a version of Ixa from the Crates.io registry, which is recommended, you need only specify the version number:
```toml
[dependencies]
ixa = "0.1.0"
```
We also depend on the `serde` library and it's "derive" feature and the `rand_distr` library. Your IDE might have a feature that adds dependencies for you automatically when you use an item from the library.
## Executing the Ixa model
We are almost ready to execute our first model. Edit `src/main.rs` to look like this:
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
Don't let this code intimidate you—it's really quite simple. The first line says we want to use symbols from the `ixa` library in the code that follows.  In `main()`, the first thing we do is call  `run_with_args()`. The `run_with_args()` function takes as an argument a closure inside which we can do additional setup before the simulation is kicked off if necessary. The only "setup" we do is log a `trace!` message that we are initializing the model.

> [!INFO] Closures
>  A *closure* is a small, self-contained block of code that can be passed around and executed later. It can capture and use variables from its surrounding environment, which makes it useful for things like callbacks, event handlers, or any situation where you want to define some logic on the fly and run it at a later time. In simple terms, a closure is like a mini anonymous function.

The `run_with_args()` function does the following:
1. It sets up a `Context` object for us, parsing and applying any command line arguments and initializing subsystems accordingly. A `Context` keeps track of the state of the world for our model and is the primary way code interacts with anything in the running model. 
2. It executes our closure, passing it a *mutable reference* to `context` so we can do any additional setup.
3. Finally, it kicks off the simulation by executing `context.execute()`. Of course, our model doesn't actually do anything or even contain any data, so `context.execute()` checks that there is no work to do and immediately returns. 

If there is an error at any stage, `run_with_args()` will return an error result. The Rust compiler will complain if we do not handle the returned result, either by checking for the error or explicitly opting out of the check, which encourages us to do the responsible thing: `match result` checks for the error.

We can build and run our model from the command line using Cargo:
```bash
cargo run
```
## Command line arguments
The model doesn't do anything yet—it doesn't even emit the log messages we included. We can turn those on to see what is happening inside our model during development with the following command line argument:
```bash
cargo run -- --log-level=trace
```
This turns on messages emitted by Ixa itself, too. If you only want to see messages emitted by `disease_model`, you can specify the module in addition to the log level:
```bash
cargo run -- --log-level=disease_model:trace
```

> [!INFO] Logging
> The `trace!`, `info!`, and `error!` logging macros allow us to print messages to the console, but they are much more powerful than a simple print statement. With log messages, you can:
> - Turn log messages on and off as needed.
> - Enable only messages with a specified priority (for example, only warnings or higher).
> - Filter messages to show only those emitted from a specific module, like the `people` module we write in the next section.
>
> See the logging documentation for more details.

> [!INFO] Command Line Arguments
> The `run_with_args()` function takes care of handling any command line arguments for us, which is why we don't just create a `Context` object and call `context.execute()` ourselves. There are many arguments we can pass to our model that affects what is output and where, debugging options, configuration input, and so forth.
> 
> See the command line documentation for more details.


In the next section we will add people to our model.
