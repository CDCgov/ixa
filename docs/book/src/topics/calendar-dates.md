# Calendar Dates

Model times are `f64` days from day 0. Once a parameters document names the
date of day 0, it can write any day field as a `YYYY-MM-DD` date instead of a
number. The `#[calendar]` attribute marks which fields are days:

```rust
use ixa::calendar;
use ixa::calendar::Date;
use serde::{Deserialize, Serialize};

#[calendar]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    #[day]
    pub start: f64,
    #[day]
    pub end: Option<f64>,
    /// A duration, not a day, so it is not marked.
    pub delay: f64,
}

#[calendar]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    #[reference]
    pub start_date: Option<Date>,
    #[days]
    pub policies: Vec<Policy>,
    #[day(inclusive)]
    pub max_time: f64,
}
```

With a start date, days and dates can be mixed freely in the same document:

```json
{
  "start_date": "2026-05-14",
  "policies": [{ "start": "2026-06-01", "end": 48, "delay": 3 }],
  "max_time": "2026-08-05"
}
```

Deserializing this gives `policies[0].start == 18.0`, `end == Some(48.0)` and
`max_time == 84.0`. The model never sees a date: every marked field is a plain
`f64` by the time the struct exists.

## Marks

- `#[reference]` goes on the start date, a `Date` or an optional one. The
  struct that carries it is the *root*. It is the only struct that reads
  directly from a document.
- `#[day]` goes on an `f64`, or an optional one, that is a point in time. A
  date resolves to the number of days from the start date.
- `#[day(inclusive)]` is for a day a date *includes*, such as the end of a
  run. `"2026-08-05"` resolves to the start of the next day, so the whole of
  August 5 is included.
- `#[days]` goes on a field whose type is another calendar struct, or a `Vec`
  or `Option` of one. The start date is passed down to it.

Durations and anything else that is not a point in time stay unmarked.

## What the attribute generates

For `Policy`, `#[calendar]` generates a twin struct `CalendarPolicy` with the
same fields, except that each `#[day]` field holds a `Day` (either days or a
date) and each `#[days]` field holds the twin of its type. It then implements
the `Calendar` trait to convert in both directions given a start date.

The root additionally deserializes through its twin, so reading `Parameters`
from JSON or TOML resolves every date, and gains a `dated()` method that
returns the twin with every whole day written as a date:

```rust
let p: Parameters = serde_json::from_str(input)?;

// Serializing the struct itself writes days.
serde_json::to_string(&p)?;   // "start":18.0

// Serializing `dated()` writes dates where it can.
serde_json::to_string(&p.dated())?;   // "start":"2026-06-01"
```

`dated()` leaves fractional days as numbers, since they have no date. A
root nested inside another root, for instance a region with its own start
date, uses its own start date and falls back to the enclosing one when it
has none.

## Errors

A date in a document with no start date is an error, as is an invalid date.
Errors name the field they occurred in:

```text
policies[0].start: 2026-06-01 is a date, but there is no start date
max_time: "2026-02-30": day 30 is not 1 to 28
```

## TOML

TOML has a native date type, so a TOML parameters file may write dates either
bare or quoted. Both read the same way:

```toml
start_date = 2026-05-14
max_time = "2026-08-05"

[[policies]]
start = 2026-06-01
end = 48
```

A date-time such as `2026-05-14T00:00:00` is rejected. Only whole dates are
accepted.

## Dates in code

Parameters built in code, for instance in a test, can use dates too. A
`Date` is the start date, and `start_of` and `end_of` give the day of a date
string counted from it. `end_of` is the inclusive form, matching
`#[day(inclusive)]`:

```rust
let start = Date::new(2026, 5, 14)?;

let p = Parameters {
    start_date: Some(start),
    policies: vec![Policy {
        start: start.start_of("2026-06-01"),       // 18.0
        end: Some(start.start_of("2026-07-01")),   // 48.0
        delay: 3.0,
    }],
    max_time: start.end_of("2026-08-05"),        // 84.0
};
```

Both panic on a malformed string, since the string is a literal in the
source. A date from input goes through `parse` instead, which returns the
error. The twin struct is also available, with a `Day` in each marked field,
and `Parameters::try_from(wire)` converts it exactly as reading a document
does.

## Writing dates in reports

Reports write whatever the row struct serializes, and `Date` serializes as
`YYYY-MM-DD`, so a date column is just a `Date` field. The context does not
know the start date on its own: store it once, from the parameters, with
`set_start_date`, then ask for the date of the current time when building a
row:

```rust
use ixa::calendar::Date;
use ixa::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
struct IncidenceRow {
    date: Option<Date>,
    time: f64,
    count: usize,
}

define_report!(IncidenceRow);

fn init(context: &mut Context, parameters: &Parameters) -> Result<(), IxaError> {
    context.set_start_date(parameters.start_date);
    context.add_report::<IncidenceRow>("incidence")?;
    Ok(())
}

fn report_incidence(context: &mut Context, count: usize) {
    context.send_report(IncidenceRow {
        date: context.get_current_date(),
        time: context.get_current_time(),
        count,
    });
}
```

With a start date of `2026-05-14`, a row sent at time `18.5` is:

```text
date,time,count
2026-06-01,18.5,2
```

A few details:

- `set_start_date` takes a `Date` or the parameters' optional one, so it can
  be called whether or not the document named a start date.
- `get_current_date` returns `None` without a start date, and an optional
  `Date` column is then written as an empty cell.
- A time falls on the date of its whole day: `18.5` and `18.0` are the same
  date. Negative times, for instance during burn-in, give dates before the
  start date.
- `get_start_date` returns the stored date, for a header or a run summary.
- To write a different day as a date, call `date_at` on the start date:
  `start.date_at(time)`.

## Limits

The twin copies the struct's derives and serde attributes, so `#[calendar]`
must sit above `#[derive(..)]`. A calendar struct:

- derives `Clone`, `Debug`, `PartialEq`, `Serialize` and `Deserialize`, and
  may derive `Copy` and `Default`, but not `Eq`, `Hash` or `Ord`;
- has no generic parameters;
- cannot use serde's `try_from`, `from`, `into` or `remote` on the struct;
- cannot use serde's `with`, `deserialize_with`, `serialize_with` or
  `default = "..."` on a `#[day]` field.

A struct whose wire form does not fit this shape can implement `Calendar` by
hand and still be used as a `#[days]` field. Its twin holds `Day` in the day
fields and converts with `Day::resolve` and `Day::from_days`:

```rust
use ixa::calendar::{Calendar, CalendarError, Date, Day};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Importation { day: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CalendarImportation { day: Day }

impl Calendar for Importation {
    type Wire = CalendarImportation;

    fn from_calendar(wire: CalendarImportation, start: Option<Date>) -> Result<Self, CalendarError> {
        Ok(Self { day: wire.day.resolve(start, false)? })
    }

    fn to_calendar(&self, start: Option<Date>) -> CalendarImportation {
        CalendarImportation { day: Day::from_days(self.day, start, false) }
    }
}
```
