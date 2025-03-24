# Python vs. Rust

Many features of the Rust language will be familiar to Python programmers. Many examples are listed below.
For more code comparisons, see [py2rs](https://github.com/dinhanhx/python-to-rust/blob/master/CODE_COMPARISON.md).

<style>
  .table-container {
    display: grid;
    grid-template-columns: .2fr 1fr 1fr;
    text-align: left;
  }
  .vert-align {
    display: flex;
    flex-direction: column;
    justify-content: center;
  }
</style>

<div class="table-container">
<div class="vert-align">
Concept
</div>
<div>

Python

</div>
<div>

Rust

</div>
<div class="vert-align">
tuples
</div>
<div>

```python
my_tuple = (1, 2, 3)
# Access the element at index 1
n = my_tuple[1]
```

</div>
<div>

```rust
let my_tuple = (1, 2, 3);
// Access the element at index 1
let n = my_tuple.1;
```

</div>
<div class="vert-align">
destructuring
</div>
<div>

```python
(x, y, z) = my_tuple
```

</div>
<div>

```rust
let (x, y, z) = my_tuple;
```

</div>
<div class="vert-align">
"don't care" / wildcard
</div>
<div>

```python
(a, _, b) = my_tuple
```

</div>
<div>

```rust
let (a, _, b) = my_tuple;
```

</div>
<div class="vert-align">
lambdas / closures
</div>
<div>

```python
# Lambdas are anonymous functions
f = lambda x: x * x
f(4)
```

</div>
<div>

```rust
// Closures are anonymous functions
let f = | x | x * x;
f(4)
```

</div>
<div class="vert-align">
type annotations
</div>
<div>

```python
x: int = 42
```

</div>
<div>

```rust
let x: i32 = 42;
```

</div>
<div class="vert-align">
ranges
</div>
<div>

```python
range(0, 10)
```

</div>
<div>

```rust
0..10
```

</div>
<div class="vert-align">
match-case / match blocks
</div>
<div>

```python
match x:
    case 1:
        ...
    case _:
        ...
```

</div>
<div>

```rust
match x {
    1 => { ... },
    _ => { ... }
}
```

</div>
<div class="vert-align">
for-in loops
</div>
<div>

```python
for i in range(0, 10):
    ...
```

</div>
<div>

```rust
for i in 0..10 {
    ...
}
```

</div>
<div class="vert-align">
If-else conditionals
</div>
<div>

```python
if x > 10:
    ...
else:
    ...
```

</div>
<div>

```rust
if x > 10 {
    ...
} else {
    ...
}
```

</div>
<div class="vert-align">
While loops
</div>
<div>

```python
while x < 10:
    ...
```

</div>
<div>

```rust
while x < 10 {
    ...
}
```

</div>
<div class="vert-align">
Importing modules
</div>
<div>

```python
import math
```

</div>
<div>

```rust
use std::f64::consts::PI;
```

</div>
<div class="vert-align">
String formatting
</div>
<div>

```python
"Hello, {}".format(name)
```

</div>
<div>

```rust
format!("Hello, {}", name)
```

</div>
<div class="vert-align">
List/Vector creation
</div>
<div>

```python
lst = [1, 2, 3]
```

</div>
<div>

```rust
let lst = vec![1, 2, 3];
```

</div>
</div>
