# Research-0002: Sampling while excluding a value

**Investigation date:** 2026-05-07 (best estimate from the source artifact)

> This artifact preserves the mathematical analysis of an alternative sampling
> algorithm. Later benchmarking found the existing implementation faster in
> practice, so the alternative was not adopted.

## Naming observations

I would make the naming uniform. We have:

- `sample_single_excluding`
- `sample_excluding_iteration` → I'd call it `sample_single_excluding_iteration`
- `sample_excluding_rejection` → I'd call it `sample_single_excluding_rejection`

## Questions

## Assumptions?

Can you say more about the assumptions of the data being sampled from? My interpretation of your implementations is that our assumptions are these:

- We are *not* assuming that elements are *distinct*. In particular, there may be several copies of the excluded value in the data.
- We are not assuming any ordering.
- We are not assuming that the data actually even contains a non-excluded element.

I ask because we can do a lot better if we know that the excluded value, if it exists in the set, is unique. In fact, correct me if I am wrong here, but the semantics we almost always want is, *sample uniformly from the set of values*, not, *sample uniformly from the set of non-excluded indices*. So should we instead implement a function `sample_single_excluded_from_distinct`?

## Container types?

Also, you have implemented this only for slices. Don't we need it for `EntitySet`s? In which case I think we actually have three container categories:

1. Known length random access containers (slices, `Vec&lt;T&gt;`). We can sample an index from `0..container.len()` and then index into it in constant time, `container[idx]` (equivalently, the iterator's `nth` implementation is an efficient lookup).
2. Known length linear access containers (`HashMap`, `BinaryHead`). We can still sample an index from `0..container.len()`, but we cannot index into the container (equivalently, the iterator's `nth` implementation is a linear scan).
3. Unknown length linear access containers (streams, sets computed on demand). We cannot sample an index, and accessing the `nth` element is a linear scan.

Rejection sampling has different asymptotics between category 1 and 2, and I think you need to use reservoir sampling for category 3 no matter what.

I have some ideas about your implementation that I'll put in a separate comment. I have a write-up, but I want to benchmark a real implementation first.

---

I think we can get away with not scanning the entire data at least some of the time. Recall that initially `n = container.len()` is known but the count of `v_excluded` values, let's call it `m`, is unknown. The basic idea is that we sample `idx` uniformly from `0..n` (using Rust `Range` syntax, this includes `0` and excludes `n`) . The `idx` is interpreted not as an index into the original container but rather the "index" of the *filtered iterator*: `container.iter().filter(|&amp;x| x != v_excluded).nth(idx)`. We wouldn't actually use that code. Instead, we would do the `nth` scan "manually" so we can count the non-excluded values (equivalently, the exlucded values; or do even more sophisticated bookkeeping). Here's the pseudocode:

**Algorithm `ScanSample(container, v_excluded)`**

1. `n ← container.len()`

2. Sample `idx` uniformly at random from `0..n`

3. Set `i ← 0`, `count ← 0`

4. **while** `i &lt; n` **do**

   4.1 **if** `container[i] != v_excluded` **then**

   ​    4.1.1 **if** `count == idx` **return** `container[i]`

   ​    4.1.2 `count ← count + 1`

   4.2 `i ← i + 1`

5. **if** `count == 0` **return** FAIL

6. **return** `ScanSampleKnownM(container, m, v_excluded)`

This algorithm succeeds at step 4.1.1 when the sampled `idx` is in the range `0..n-m`. At step 5, we know `m` (actually, `count = n-m`), because we've scanned the entire data. The fallback `ScanSampleKnownM` could be just this same algorithm but with `idx` sampled from `0..n-m-1` in the first place. (We can imagine a more sophisticated variation where we do some bookkeeping while scanning that is passed to the fallback `ScanSampleKnownM` to make it more efficient, but let's keep the fallback simple for now.)

## Correctness

Claim: The algorithm is uniform. When `idx` falls in `0..(n−m−1)`, step 4.1.1 returns the `idx`-th non-excluded element, and since `idx` is uniform over that range, each non-excluded element is equally likely. When `idx` falls in `(n−m)..(n−1)`, the fallback samples uniformly by the same argument. Both branches produce a uniform draw, so the overall result is uniform.

## Performance

Let $k = n − m$ be the number of non-excluded elements, and let $p_0 < p_1 < \cdots < p_{k-1}$ be their 0-indexed positions in the container. Define $S = \sum_{j=0}^{k-1} (p_j + 1)$, i.e. the sum of their 1-indexed positions, or equivalently, the sum of the number of spaces that need to be scanned to get to the non-excluded element. We care about this sum because we want to know the average number of steps needed to select $p_j$, which is $S/k \approx (n+1)/2$.

**The two cases:**

`idx` is sampled uniformly from `{0, ..., n−1}`, so:

- **Early exit** (`idx ∈ {0, ..., k−1}`): probability $k/n$. Given `idx = j`, we scan to position $p_j$ and return, taking $p_j + 1$ steps.
- **Full scan** (`idx ∈ {k, ..., n−1}`): probability $m/n$. We scan all $n$ elements, then `ScanSampleKnownM` scans again, taking an expected `S/k` additional steps (same average as the early-exit case, since it samples `idx` uniformly from `0..k`).

**Computing $E[\text{steps}]$:**

$$
E[\text{steps}] = \frac{1}{n} \sum_{j=0}^{k-1}(p_j + 1) + \frac{m}{n}\left(n + \frac{S}{k}\right) \\
= \frac{S}{n} + m + \frac{mS}{nk} = \frac{S}{n}\left(1 + \frac{m}{k}\right) + m = \frac{S}{n} \cdot \frac{n}{k} + m \\
= \frac{S}{k} + m
$$

where $S/k$ is simply the average 1-indexed position of a non-excluded element, or equivalently, the average number of spaces that need to be scanned to get to the non-excluded element. For a typical random arrangement of excluded values, $\frac{S}{k} \approx \frac{n+1}{2}$ by symmetry, giving $E[\text{steps}] \approx \frac{n+1}{2} + m$.

The extremes are:

| Arrangement                   | E[steps]                         | Notes                                               |
| ----------------------------- |----------------------------------| --------------------------------------------------- |
| Non-excluded all at **front** | $(k+1)/2 + m$                    | Best case; early exit is cheap                      |
| Non-excluded all at **back**  | $m + (k+1)/2 + m = 2m + (k+1)/2$ | Worst case; nearly always falls through and rescans |
| Random arrangement            | $\approx (n+1)/2 + m$            | Average case                                        |
| Reservoir sampling            | exactly $n$                      | Always, regardless of layout                        |

The worst case approaches $2n$ (when $m$ is large and non-excluded values cluster at the end), which is worse than reservoir sampling's fixed $n$. The best case approaches $n/2$ (when $m$ is small), which is better. So `ScanSample` is essentially a bet on $m$ being small: it pays off when exclusions are rare, and costs up to twice as much as reservoir sampling when they are common.
