# Handling Errors

Ixa generates errors using its `IxaError` type. In Ixa v2, this error type is intended only for errors Ixa itself
generates. For errors your own code produces, it is up to you to create your own error type or use a "universal" error
type crate like [`anyhow`](https://docs.rs/anyhow/latest/anyhow/).

## Summary

Use [`anyhow`](https://docs.rs/anyhow/latest/anyhow/) and its universal error type if:

- you want an easy, no-nonsense way to deal with errors with the least amount of effort
- you don't need to define your own error types or variants

Use [`thiserror`](https://docs.rs/thiserror/latest/thiserror/) to help you easily define your own error type if:

- you want to have your own error types / error enum variants
- you want control over how you manage errors in your model

## Creating your own error types

You might want your own error type if you want to generate structured errors from your own code or want your own
structured error handling code. For example, you might want to have functions that return `Result<U, V>` to indicate
that they might fail:

```rust
fn get_itinerary(person_id: PersonId, context: &Context) -> Result<Itinerary, ModelError> {
    // If we can't retrieve an itinerary for the given person, we return an error
    // that gives information about what went wrong:
    return Err(ModelError::NoItineraryForPerson);
}
```

When you call this function, you can take more specific action based on what it returns:

```rust
match get_itinerary(person_id, context) {
    Ok(itinerary) => {
        /* Do something with the itinerary */
    }
    Err(ModelError::NoItineraryForPerson) => {
        /* Handle the `NoItineraryForPerson` error */
    }
    Err(err) => {
        /* A different error occurred; handle it in a different way */
    }
}
```

The [`thiserror`](https://docs.rs/thiserror/latest/thiserror/) crate reduces the boilerplate you have to write to
implement your own error types (an enum implementing the `std::error::Error` trait). In practice, model code often
needs to report different types of errors:

- errors defined by the model itself
- errors returned by Ixa APIs such as `context.add_report()`
- errors from other crates or from the standard library

That usually means your error enum contains a mix of your own variants and variants that wrap foreign error types. For
example:

```rust
use ixa::error::IxaError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("model error: {0}")]
    ModelError(String),

    #[error("Ixa error")]
    IxaError(#[from] IxaError),

    #[error("string error")]
    StringError(#[from] std::string::FromUtf8Error),

    #[error("parse int error")]
    ParseIntError(#[from] std::num::ParseIntError),

    #[error("Ixa csv error")]
    CsvError(#[from] ixa::csv::Error),
}
```

[`thiserror`](https://docs.rs/thiserror/latest/thiserror/) automatically generates:

- `impl std::error::Error for ModelError`
- `impl Display for ModelError`
- `From<IxaError>`, `From<std::string::FromUtf8Error>`, `From<std::num::ParseIntError>`, and `From<ixa::csv::Error>`
  (because of `#[from]`)
- `source()` wiring for error chaining

That last item is what lets one error wrap another: The `std::error::Error`
trait has a `source(&self) -> Option<&(dyn Error + 'static)>` method. When an
error returns another error from `source()`, it is saying "this error happened
because of that other error." Error reporters can then walk the chain and show
both the top-level message and the underlying cause.

You can implement all of this without the [`thiserror`](https://docs.rs/thiserror/latest/thiserror/) crate, but
[`thiserror`](https://docs.rs/thiserror/latest/thiserror/) saves you a lot of boilerplate. With
[`thiserror`](https://docs.rs/thiserror/latest/thiserror/), you usually do not write `source()` yourself. A field
marked with `#[from]` or `#[source]` is treated as the underlying cause and returned from `source()` automatically.
`#[from]` also generates the corresponding `From<...>` impl, while `#[source]` only marks the wrapped error as the
cause.

This shows several useful patterns:

- `ModelError::ModelError` is a model-specific error variant that you define yourself.
- `ModelError::IxaError` wraps any `IxaError` variant, which is useful when Ixa code returns an `IxaError` and you want
  to propagate it as part of your model's error type.
- `ModelError::CsvError` wraps errors returned from the vendored CSV crate.
- `ModelError::StringError` and `ModelError::ParseIntError` wrap standard library error types.

All of these wrapped variants participate in the standard error chain through `source()`. If one of your model
functions calls into Ixa or another library and that call fails, your model error can preserve the original cause while
still returning a single model-specific error type.

For example, if you want to add model-specific context around an `IxaError` instead of directly converting it with
`#[from]`, you can write:

```rust
use ixa::error::IxaError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ModelError {
    #[error("failed to load itinerary for person {person_id}")]
    LoadItinerary {
        person_id: PersonId,
        #[source]
        source: IxaError,
    },
}
```

Now `Display` prints the outer message, while `source()` returns the inner
`IxaError`, such as `IxaError::NoGlobalProperty { .. }`. This is useful when you want the error to say what your model
was trying to do, but you also want to preserve the lower-level Ixa failure as the underlying cause. Callers can then
pattern-match on `ModelError` and still inspect or report the original cause through the standard error chain.

## Using the [`anyhow`](https://docs.rs/anyhow/latest/anyhow/) crate to easily propagate errors

Where [`thiserror`](https://docs.rs/thiserror/latest/thiserror/) is for defining your own structured error types,
[`anyhow`](https://docs.rs/anyhow/latest/anyhow/) provides a single concrete error type for you: `anyhow::Error`. Its
major selling point is propagating errors ergonomically in applications.

### Easy error propagation

Instead of:

```rust
fn do_work() -> Result<T, MyError>
```

you can write:

```rust
fn do_work() -> anyhow::Result<T>
```

and use `?` with almost anything that implements `std::error::Error`. No custom enum required, and no `From`
boilerplate-it automatically converts into `anyhow::Error`.

### Attaching additional context to errors

This is one of the strongest features:

```rust
use anyhow::Context;

read_file(path).with_context( | | format!("Failed reading {}", path))?;
```

This produces an error chain like:

```text
Failed reading config.json
Caused by:
    No such file or directory
```

That is extremely ergonomic.

### Automatic Backtraces

If enabled:

- `anyhow` captures backtraces automatically
- No custom wiring required

### Downsides to `anyhow`

Unlike creating your own error types with `thiserror`, `anyhow` erases concrete type information. That means you cannot
pattern-match on the original error type unless you [downcast](https://docs.rs/anyhow/latest/anyhow/).
