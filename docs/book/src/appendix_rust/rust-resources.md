# Learning Rust for Ixa

## Why Rust?

We designed Ixa to be efficient for computers and for people. Rust provides the speed and memory
characteristics necessary for large-scale, computationally-intensive simulations while still being
relatively friendly to work with. That being said, if you are used to working in a higher-level
language like R or Python, Rust can take some time to learn. We recommend taking a look at the
[The Rust Book](https://rust-book.cs.brown.edu/) for a comprehensive overview. Here are a few features
that might be new to you:

1. Rust has a strict model of how it uses your computer memory, called ownership. In practice, this
means that you can only manipulate objects in specific ways. This helps code actually do what you
think it is doing and has been shown to reduce [long-term bugs](https://thehackernews.com/2024/09/googles-shift-to-rust-programming-cuts.html#:~:text=Google%20has%20revealed%20that%20its,a%20period%20of%20six%20years.).
Rust's ownership rules are enforced at _compile-time_, which helps you find errors in your code
earlier. When developing ABMs with Ixa, these capabilities help us more easily reason about
complicated model logic and ensure that plugins interact modularly.

2. Rust has a cohesive ecosystem of all the tools you will need for development: it has a built-in
package/project manager (`cargo`), a built-in linter (`clippy`), a built-in environment manager for
updating Rust versions so that you can manage multiple simultaneously (`rustup`), and a global
repository for all [packages](https://crates.io). Rust provides a complete ecosystem for all your
model development needs, and we find this centralization useful for ensuring robust version control
and reproducibility of code across different users and computers. This also means that chances are
someone has built a crate for a problem you may be troubleshooting.

3. Rust is a strongly-typed language, meaning that the type of each variable must be known at
compile-time and cannot be changed after variable definition. However, Rust also has automatic type
inference and other ergonomic features to help make writing code more seamless and efficient. As a
result, you only need to specify the type of a variable in cases where it is ambiguous while still
taking advantage of the rigidity of static typing to ensure your code is doing what you think it is
doing. We find the combination of static typing with other Rust features for quick prototyping
valuable for writing models and tests simultaneously.

## Common Rust patterns in Ixa modeling

[The Rust Book](https://rust-book.cs.brown.edu/) remains the best resource to learn Rust, and we
recommend reading the book in its entirety. However, the book emphasizes concepts that are less
prevalent in day-to-day Ixa use (for instance, ownership), and certain patterns that pop up a lot
are less emphasized. Below, we include sections of the book that are particularly important.

1. [Chapter 5: Structs](https://rust-book.cs.brown.edu/ch05-00-structs.html): If you are new to
object-oriented programming, this chapter introduces the "object-oriented"-ness of Rust. Rust has
elements of both an object-oriented and functional programming language, but structs come up often
when writing an Ixa model, and thinking about Ixa as being an object-oriented framework is useful
for evaluating the kinds of data you need in your model and how to use the data.

2. [Chapter 6: Enums](https://rust-book.cs.brown.edu/ch06-00-enums.html): Enums, match statements,
and storing data in enum variants are all distinctly Rust patterns. If you have previously worked
with Haskell or OCaml, these objects may be familiar to you. Nevertheless, enums come up frequently
in Ixa, in particular with person properties (e.g., a person's infection status is a value of an
enum with variants `Susceptible`, `Infected`, `Recovered`), and seeing how you can store information
in each of these variants helps modelers understand how to best store the data they need for their
particular use case.

3. [Chapter 10, Section 2: Traits](https://rust-book.cs.brown.edu/ch10-02-traits.html): _If you read_
_only one section, let this be it._ Traits are the cornerstone of Ixa's modular nature. They are
used in at least two ways throughout the Ixa codebase. First, the methods that make up higher-level
abstractions -- like `Property` or `Event` -- are organized into a trait, giving them shared features
so that calling code can use these features without knowing the exact underlying type. For instance,
your model might define a trait to specify interventions that impact the probability of transmission
-- like facemasks or hazmat suits. Regardless of the exact nature of the intervention, your model
wants to call some methods on the transmisison modifier, like returning its efficacy. If these
methods are grouped into the `TransmissionModifier` trait, they can be implemented on any type and
used wherever the code needs a transmission modifier without depending on the underlying type. Secondly,
traits implemented on `Context` (called "Context extensions" in Ixa) are the primary way of new
modules adding a public interface so that their methods can be called from other modules. For instance,
the `ContextPeopleExt` trait extension provides methods relating to people and their properties. Rust
traits are a supercharged version of "interfaces" in other programming languages, and thinking in terms
of traits will help you write modular Ixa code.

4. [Chapter 13, Section 1: Closures](https://rust-book.cs.brown.edu/ch13-01-closures.html): Ixa often
requires the user to specify a function that gets executed in the future. This chapter goes over the
mechanics for how anonymous functions are specified in Rust. Although the syntax is not challenging,
this chapter discusses capturing and moving values into a closure, type inference for closure arguments,
and other concepts specific to Rust closures. You may see the word "callback" referred to in the Ixa
documentation -- callbacks are what Ixa calls the user-specified closures that are evaluated when
executing plans or handling events.

In addition, [Chapter 19: Patterns](https://rust-book.cs.brown.edu/ch19-00-patterns.html) reiterates
the power of enum variants and the `match` statement in Rust. If you find yourself writing more
advanced Rust code, Chapters [9: Error Handling](https://rust-book.cs.brown.edu/ch09-00-error-handling.html),
[18: Object-oriented programming](https://rust-book.cs.brown.edu/ch18-00-oop.html), and
[20: Advanced Features](https://rust-book.cs.brown.edu/ch20-00-advanced-features.html) include
helpful tips as you dive into more complicated Ixa development.

If you find yourself writing more analysis-focused code in Rust, Chapters
[9: Error Handling](https://rust-book.cs.brown.edu/ch09-00-error-handling.html) and
[13.2: Iterators](https://rust-book.cs.brown.edu/ch13-02-iterators.html) include helpful tools for
writing code reminiscent of Python.

### On Ownership

Rust's signature feature is its ownership rules. We tried to design Ixa to handle ownership internally,
so that users rarely have to think about the specific Rust rules when interfacing with Ixa.
Nevertheless, understanding ownership rules is valuable for debugging more complicated Ixa code and
understanding what you can and cannot do in Rust. There are excellent resources for learning Rust's
ownership rules available [online](https://educatedguesswork.org/posts/memory-management-4/),
but at a high-level, here are the key ideas to understand:

1. When a function takes an object as input, it takes ownership of that object -- meaning that the
object is "consumed" by the function and does not exist after the calling of that function.
    - This is true for all types except those that are automatically copyable, or in Rust lingo
    implement the `Copy` trait. All primitive types -- floats, integers, booleans, etc. -- implement
    `Copy`, meaning that this rule is most commonly felt with vectors and hashmaps. These are examples
    of types that do not implement `Copy` -- in this case, that is because their size dynamically
    changes as data is added and removed, so Rust stores them in a part of the memory optimized for
    changing sizes rather than fast access and copying.
2. References (denoted by an `&` in front of an object like `&Object`) allow functions to have
access to the object without taking ownership of it. Therefore, we often return references to an
object so that we do not have to give up explicit ownership of that object, such as when we want to
get the value of data owned centrally by the simulation (ex., model parameters) in a particular
module.
3. There are two kinds of references -- mutable references `&mut Object` and immutable references
`&Object`. Depending on what kind of reference you have, you can do one of two kinds of things.
If you have an active immutable reference to an object, you can take any number of additional immutable
references to the object. But, if you have a mutable reference to an object, you can only ever have
that one mutable reference to the object be active. This is because an active immutable reference to
the object would also have changed as the mutable reference is mutated, and Rust can no longer make
guarantees about the memory to which the immutable reference points.

In practice, #3 often requires the most troubleshooting to get around. Sometimes, a refactor of your
code can help circumvent ownership challenges.

## Ixa tutorials

Within the Ixa repo, we have created some examples. Each example has a readme that walks the user
through a toy model and what it illustrates about Ixa's core functionality. To run the examples,
from the repo root, just specify the name of the example:

```bash
cargo run --example {name-of-example}
```

In general, you will be using `cargo` to run and interact with your Ixa models from the command line.
We recommend learning some [basic `cargo` commands](https://doc.rust-lang.org/cargo/guide/index.html),
and there are valuable [cheatsheets](https://kapeli.com/cheat_sheets/Cargo.docset/Contents/Resources/Documents/index)
to keep handy as you get more involved in active development.

Here are a few other useful commands to know:

- `cargo test` will run all tests in the project.
- `cargo build --release` compiles the project into a shell executable that you can ship to your users.
- `cargo add {crate-name}` adds a Rust crate/project dependency to your project for you to use.

## Additional Rust resources

- **[Rust By Example](https://doc.rust-lang.org/rust-by-example/index.html):** "RBE shows off a bunch of code,
   and keeps the talking to a minimum. It also includes exercises!"
- **[Tour of Rust](https://tourofrust.com/TOC_en.html):** Live code and explanations, side by side.
- **[Rust in Easy English](https://dhghomon.github.io/easy_rust/Chapter_3.html):** 60+ concepts, simple English, example-driven.
- **[Rust for the Polyglot Programmer](https://www.chiark.greenend.org.uk/~ianmdlvl/rust-polyglot/index.html):**
   A guide for the experienced programmer.
- **[The Rust Standard Library Documentation](https://doc.rust-lang.org/std/index.html)**
- **[The Cargo Book](https://doc.rust-lang.org/cargo/index.html):** An online manual for Rust's package manager Cargo.
- **[The Rust Playground](https://play.rust-lang.org/):** Execute sample code in your browser.
