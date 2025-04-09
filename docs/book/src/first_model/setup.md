# Setting Up Your First Model

## Create a new project with Cargo

Let's setup the bare bones skeleton of our first model. First decide where your Ixa-related code is going to live on
your computer. On my computer, that's the `Code` directory in my `home` folder (or `~` for short). I will use my
directory structure for illustration purposes in this section. Just modify the commands for wherever you chose to store
your models.

Navigate to the directory you have chosen for your models and then use `Cargo`to initialize a new Rust project called
`disease_model`.

```bash
cd ~/Code
cargo new --bin disease_model
```

Cargo creates a directory named `disease_model` with a project skeleton for us. Open the newly created `disease_model`
directory in your favorite IDE, like [VSCode](https://code.visualstudio.com/) (free)
or [RustRover](https://www.jetbrains.com/rust/).

```bash
🏠 home/
└── 🗂️ Code/
    └── 🗂️ disease_model/
        ├── 🗂️ src/
        │   └── 📄 main.rs
        ├── .gitignore
        └── 📄 Cargo.toml
```

> [!INFO] Source Control
> The `.gitignore` file lists all the files and directories you don't want to include in source control. For a Rust
> project you should at least have `target` and `Cargo.lock` listed in the `.gitignore`. I also make a habit of listing
`.vscode` and `.idea`, the directories VS Code and JetBrains respectively store IDE project settings.
> [!INFO] Cargo
> Cargo is Rust's package manager and build system. It is a single tool that plays the role of the multiple different
> tools you would use in other languages, such as `pip` and `poetry` in the Python ecosystem. We use Cargo to
>
> - install tools like ripgrep (`cargo install`)
> - initialize new projects (`cargo new` and `cargo init`)
> - add new project dependencies (`cargo add serde`)
> - update dependency versions (`cargo update`)
> - check the project's code for errors (`cargo check`)
> - download and build the correct dependencies with the correct feature flags (`cargo build`)
> - build the project's targets, including examples and tests (`cargo build`)
> - generate documentation (`cargo doc`)
> - run tests and benchmarks (`cargo test`, `cargo bench`)

## Setup Dependencies and `Cargo.toml`

Ixa comes with a convenience script for setting up new Ixa projects. Change directory to `disease_model/`, the project
root, and run this command.

```bash
curl -s https://raw.githubusercontent.com/CDCgov/ixa/release/scripts/setup_new_ixa_project.sh | sh -s
```

The script adds the Ixa library as a project dependency and provides you with a minimal Ixa program in `src/main.rs`.

### Dependencies

We will depend on a few external libraries in addition to Ixa. The `cargo add` command makes this easy.

```bash
cargo add rand_distr@0.4.3
cargo add serde --features derive
cargo add csv
```

Notice that:

- a particular version can be specified with the `packagename@1.2.3` syntax;
- we can compile a library with specific features turn on or off.

### `Cargo.toml`

Cargo stores information about these dependencies in the `Cargo.toml` file. This file also stores metadata about your
project used when publishing your project to Crates.io. Even though we won't be publishing the crate to Crates.io, it's
a good idea to get into the habit of adding at least the author(s) and a brief description of the project.

```toml
# Cargo.toml
{{ #include ../../models/disease_model/Cargo.toml}}
```

## Executing the Ixa model

We are almost ready to execute our first model. Edit `src/main.rs` to look like this:

```rust
// main.rs
{{ #include ../../models/basic/main.rs}}
```

Don't let this code intimidate you—it's really quite simple. The first line says we want to use symbols from the `ixa`
library in the code that follows. In `main()`, the first thing we do is call  `run_with_args()`. The `run_with_args()`
function takes as an argument a closure inside which we can do additional setup before the simulation is kicked off if
necessary. The only setup we do is schedule a plan at time 1.0. The plan is itself another closure that prints the
current simulation time.

> [!INFO] Closures
> A *closure* is a small, self-contained block of code that can be passed around and executed later. It can capture and
> use variables from its surrounding environment, which makes it useful for things like callbacks, event handlers, or
> any
> situation where you want to define some logic on the fly and run it at a later time. In simple terms, a closure is
> like
> a mini anonymous function.

The `run_with_args()` function does the following:

1. It sets up a `Context` object for us, parsing and applying any command line arguments and initializing subsystems
   accordingly. A `Context` keeps track of the state of the world for our model and is the primary way code interacts
   with anything in the running model.
2. It executes our closure, passing it a *mutable reference* to `context` so we can do any additional setup.

3. Finally, it kicks off the simulation by executing `context.execute()`. Of course, our model doesn't actually do
   anything or even contain any data, so `context.execute()` checks that there is no work to do and immediately returns.

If there is an error at any stage, `run_with_args()` will return an error result. The Rust compiler will complain if we
do not handle the returned result, either by checking for the error or explicitly opting out of the check, which
encourages us to do the responsible thing: `match result` checks for the error.

We can build and run our model from the command line using Cargo:

```bash
cargo run
```

## Enabling Logging

The model doesn't do anything yet—it doesn't even emit the log messages we included. We can turn those on to see what is
happening inside our model during development with the following command line argument:

```bash
cargo run -- --log-level trace
```

This turns on messages emitted by Ixa itself, too. If you only want to see messages emitted by `disease_model`, you can
specify the module in addition to the log level:

```bash
cargo run -- --log-level disease_model=trace
```

> [!INFO] Logging
> The `trace!`, `info!`, and `error!` logging macros allow us to print messages to the console, but they are much more
> powerful than a simple print statement. With log messages, you can:
>
> - Turn log messages on and off as needed.
> - Enable only messages with a specified priority (for example, only warnings or higher).
> - Filter messages to show only those emitted from a specific module, like the `people` module we write in the next
    section.
>
> See the logging documentation for more details.
> [!INFO] Command Line Arguments
> The `run_with_args()` function takes care of handling any command line arguments for us, which is why we don't just
> create a `Context` object and call `context.execute()` ourselves. There are many arguments we can pass to our model
> that
> affects what is output and where, debugging options, configuration input, and so forth.
> See the command line documentation for more details.

In the next section we will add people to our model.
