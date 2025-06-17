# Learning Rust for ixa

## Why Rust?

Rust is different from many programming languages. The best resource for learning Rust remains
[The Rust Book](https://rust-book.cs.brown.edu/) with quizzes.

At a high-level, here's why Rust's unique features are useful for ABM development:

1. Rust has a strict model of how it uses your computer memory, called ownership. In practice, this
means that you can only manipulate objects in specific circumstances. This helps code actually do
what you think it is doing and has been shown to reduce [long-term bugs](https://thehackernews.com/2024/09/googles-shift-to-rust-programming-cuts.html#:~:text=Google%20has%20revealed%20that%20its,a%20period%20of%20six%20years.).
Rust's ownership rules are enforced at _compile-time_, which helps you find errors in your code
earlier. When developing ABMs with ixa, these capabilities help us more easily reason about
complicated model logic and ensure that plugins interact modularly.

2. Rust has a cohesive ecosystem of all the tools you will need for development: it has a built-in
package/project manager (`cargo`), a built-in linter (`clippy`), a built-in environment manager for
updating Rust versions so that you can manage multiple simultaneously (`rustup`), and a global
repository for all [packages](https://crates.io). Rust provides a complete ecosystem for all your
model development needs, and we find this centralization useful for ensuring robust version control
and reproducibility of code across different users and computers. This also means that chances are
someone has built a crate for a problem you may be troubleshooting.

3. Despite being a strongly-typed language, compared to other systems programming languages, Rust
has automatic type inference and other ergonomic features to help make writing code more seamless
and efficient. As a result, you only need to specify the type of a variable in cases where it is
ambiguous while still taking advantage of the rigidity of static typing to ensure your code is doing
what you think it is doing. We find the combination of static typing with other Rust features for
quick prototyping valuable for writing models and tests simultaneously.

## Learning Rust patterns

[The Rust Book](https://rust-book.cs.brown.edu/) remains the best resource to learn Rust, and we
recommend reading the book in its entirety. However, the book emphasizes concepts that are less
prevalent in day-to-day ixa use (for instance, ownership), and certain patterns that pop up a lot
are less emphasized. Below, we include sections of the book that are particularly important.

1. [Chapter 5: Structs](https://rust-book.cs.brown.edu/ch05-00-structs.html): If you are new to
object-oriented programming, this chapter introduces the "object-oriented"-ness of Rust. Rust has
elements of both an object-oriented and functional programming language, but structs come up often
when writing an ixa model, and thinking about ixa as being an object-oriented framework is useful
for evaluating the kinds of data you need in your model and how to use the data.

2. [Chapter 6: Enums](https://rust-book.cs.brown.edu/ch06-00-enums.html): Enums, match statements,
and storing data in enum variants are all distinctly Rust patterns. If you have previously worked
with Haskell or OCaml, these objects may be familiar to you. Nevertheless, enums come up frequently
in ixa, in particular with person properties (e.g., a person's infection status is a value of an
enum with variants `Susceptible`, `Infected`, `Recovered`), and seeing how you can store information
in each of these variants helps modelers understand how to best store the data they need for their
particular use case.

3. [Chapter 10, Section 2: Traits](https://rust-book.cs.brown.edu/ch10-02-traits.html): _If you read_
_only one section, let this be it._ Traits are the cornerstone of ixa's modular nature. They are
used in at least two ways throughout the ixa codebase: (i) methods are implemented on top of
`Context` as part of a trait -- such as methods relating to people and their properties being
implemented through the `ContextPeopleExt` trait extension -- and (ii) ixa defines traits that the
user implements in their own modeling code. For example, `PersonProperty` is a trait that the user
implements through calls to the `define_person_property!` macro, and `ixa-epi` defines a
`TransmissionModifier` trait that a user implements when writing their own intervention. Rust traits
are a supercharged version of interfaces in other programming languages, and thinking in traits will
help you write modular ixa code.

4. [Chapter 13, Section 1: Closures](https://rust-book.cs.brown.edu/ch13-01-closures.html): Ixa often
requires the user to specify a function that gets executed in the future. This chapter goes over the
mechanics for how anonymous functions are specified in Rust. Although the syntax is not challenging,
this chapter shows how closures interact with other aspects of Rust, such as the ownership model,
and therefore how you can write the most useful code that depends on closures (which ixa calls a
callback in the documentation).

In addition, [Chapter 19: Patterns](https://rust-book.cs.brown.edu/ch19-00-patterns.html) reiterates
the power of enum variants and the `match` statement in Rust. If you find yourself writing more
advanced Rust code, Chapters [9: Error Handling](https://rust-book.cs.brown.edu/ch09-00-error-handling.html),
[18: Object-oriented programming](https://rust-book.cs.brown.edu/ch18-00-oop.html), and
[20: Advanced Features](https://rust-book.cs.brown.edu/ch20-00-advanced-features.html) include
helpful tips as you dive into more complicated ixa development.

If you find yourself writing more analysis-focused code in Rust, Chapters [9: Error Handling](https://rust-book.cs.brown.edu/ch09-00-error-handling.html) and [13.2: Iterators](https://rust-book.cs.brown.edu/ch13-02-iterators.html)
include helpful tools for writing code reminiscent of Python.

### On Ownership

Rust's signature feature is its ownership rules. Many of the details of ownership are taken care of
by the ixa framework, meaning ixa users are often abstracted from Rust ownership challenges.
Nevertheless, understanding ownership rules is valuable for debugging more complicated ixa code and
understanding what you can and cannot do in Rust. There are excellent resources for understanding
Rust's ownership rules available [online](https://educatedguesswork.org/posts/memory-management-4/),
but at a high-level, here are the key ideas to understand:

1. When a function takes an object as input, it is taking ownership of that object -- meaning that
the object is "consumed" by the function and does not exist after the calling of that function.
    - This is true for all types except those that implement the `Copy` trait. All primitive types --
    such as floats, integers, booleans, etc. -- implement the `Copy` trait, meaning that this
    rule is really only felt with vectors and hashmaps.
2. References (`&Object`) allow functions to have access to the object without taking ownership of it.
Therefore, it's often useful to return a reference to an object without giving up ownership of the
object, such as when there's data that's owned by `Context`.
3. There are two kinds of references -- mutable references `&mut Object` and immutable references
`&Object`. There is no limit to the number of immutable references to an object that can be active
(i.e., returned from a function), or you can take a mutable reference to an object, and you can only
ever have one mutable reference that is active within a function at a time.

In practice, #3 often requires the most troubleshooting to get around. Sometimes, a reorg of your
code can help circumvent ownership challenges.

## Ixa tutorials

Within the ixa repo, we have created some examples. Each example has a readme that walks the user
through a toy model and what it illustrates about ixa's core functionality. To run the examples,
from the repo root, just specify the name of the example:

```bash
cargo run --example {name-of-example}
```

In general, you will be using `cargo` to run and interact with your ixa models from the command line.
We recommend learning some [basic `cargo` commands](https://doc.rust-lang.org/cargo/guide/index.html)
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
