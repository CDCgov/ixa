# Calendar Dates

Model times are `f64` days from day 0. Once a parameters document names the
date of day 0, it can write any day field as a `YYYY-MM-DD` date, a
`YYYY-Www` week (e.g., `2026-W27`), days with units such as `2w`, or a number.
The `#[calendar]` attribute marks which fields are days:

```rust
use ixa::calendar;
use ixa::calendar::Date;
use serde::{Deserialize, Serialize};

#[calendar]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Policy {
    #[calendar]
    pub start: f64,
    #[calendar]
    pub end: Option<f64>,
    #[calendar(duration)]
    pub delay: f64,
}

#[calendar(weeks = "epi")]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameters {
    #[calendar(reference)]
    pub start_date: Option<Date>,
    #[calendar]
    pub policies: Vec<Policy>,
    #[calendar(inclusive)]
    pub max_time: f64,
}
```

The forms mix freely in one document:

```json
{
  "start_date": "2026-05-14",
  "policies": [{ "start": "2026-06-01", "end": "2026-W27", "delay": "2w" }],
  "max_time": "2026-08-05"
}
```

This reads as `start == 18.0`, `end == Some(52.0)` (the Sunday that epi
week 27 starts on), `delay == 14.0` and `max_time == 84.0`. The model only
ever sees `f64`s.

## Marks

`#[calendar]` marks a field of days: an `f64`, another calendar struct, or
an `Option` or `Vec` of either. Options go in parentheses:

- `#[calendar(reference)]` marks the start date, a `Date` or `Option<Date>`.
  Its struct is the *root*, and passes the start date down to the structs
  inside it. A nested root uses its own start date, or the enclosing one if
  it has none.
- `#[calendar]` alone is a point in time: a date resolves to days from the
  start date, a week to the day it starts on.
- `#[calendar(inclusive)]` is a day that a date or week *includes*, such as
  the end of a run: `"2026-08-05"` resolves to the start of August 6, and a
  week to the start of the next week.
- `#[calendar(duration)]` is a length of time: numbers and units only, not
  negative, and not moved by `shift_days`. Add `signed` to allow negatives.
- `#[calendar(week)]` makes `dated()` write a day that starts a week as the
  week. It combines with `inclusive`.

`inclusive`, `duration`, `signed` and `week` are a compile error on a field
of calendar structs. Fields that are not times stay unmarked.

## Units

Units are `w`, `d` and `h`, or `week(s)`, `day(s)` and `hour(s)`. Terms add
up and may be fractional: `"2w"` is 14, `"1w 2d"` is 9, `"36h"` is 1.5 and
`"0.5w"` is 3.5. A leading `-` negates the whole value: `"-1w"` is -7. Units
count from day 0, so they need no start date: `"max_time": "26w"` is day 182.
Months and years have no fixed length and are rejected; write a date instead.

## Weeks

There are two week systems:

- `weeks = "iso"`: ISO 8601 weeks, starting on Monday.
- `weeks = "epi"`: CDC epidemiological (MMWR) weeks, starting on Sunday.

In both, week 1 is the first week with at least four days in the new year,
so a week near New Year may belong to the adjacent year: 2026-01-01 is in
epi week 53 of 2025 but ISO week 1 of 2026.

`#[calendar(weeks = "..")]` on a struct sets the system for it and the
structs inside it. There is no default, since reading a week in the wrong
system shifts it by a day or a week, so a week without a system is an error.

## Generated `Calendar` structs

For `Policy`, `#[calendar]` generates a twin struct `CalendarPolicy` in which
each marked field holds its wire form: a `Day` (days, a date or a week) for
an `f64`, or the nested struct's twin. The `Calendar` trait converts between
the two, given a `CalendarContext` (start date and week system) and the
`FieldOptions` of the field being converted.

Every calendar struct deserializes through its twin, so dates resolve as the
document is read. A struct read without a root around it has no start date:
it takes numbers and units, and a date or week in it is an error.

The root also has `dated()`, which returns the twin with whole days written
as dates. Fractional days and durations stay numbers:

```rust
let p: Parameters = serde_json::from_str(input)?;

serde_json::to_string(&p)?;           // "start":18.0
serde_json::to_string(&p.dated())?;   // "start":"2026-06-01"
```

## Shifting times

Two methods on `Calendar` move times:

- `shift_days(n)` moves every point in time `n` days later. Durations and
  start dates stay put.
- `shift_start_dates(n)` moves every start date, nested roots' included,
  `n` days later. It fails if a date would leave years 0 to 9999.

Together they move day 0 while every date stays the same. For example, to
add a 30-day burn-in before the first date:

```rust
use ixa::calendar::Calendar;

parameters.shift_days(30.0);
parameters.shift_start_dates(-30)?;
```

In a hand-written `Calendar` impl, `shift_start_dates` does nothing by
default. Override it if the struct holds a start date or a nested root.

## Errors

Errors name the field they occurred in:

```text
policies[0].start: 2026-06-01 is a date, but there is no start date
max_time: "2026-02-30": day 30 is not 1 to 28
policies[0].end: 2026-W53: 2026 has 52 epi weeks
policies[0].delay: a duration cannot be the week 2026-W01
```

## TOML

TOML dates may be bare or quoted; both read the same. Weeks and units must
be quoted, and date-times such as `2026-05-14T00:00:00` are rejected:

```toml
start_date = 2026-05-14
max_time = "2026-08-05"

[[policies]]
start = 2026-06-01
end = 48
```

## Binary formats

Dates, weeks and units are for human-readable formats. A format that is not
human-readable by serde's `is_human_readable`, such as bincode, MessagePack
or postcard, reads and writes a calendar struct as written, in days only.
The struct round trips through any serde format, and other tools see plain
numbers.

## Dates in code

For parameters built in code, such as in a test, the start date converts
date and week strings to days: `start_of` and `start_of_week`, plus the
inclusive forms `end_of` and `end_of_week`:

```rust
let start = Date::new(2026, 5, 14)?;

let p = Parameters {
    start_date: Some(start),
    policies: vec![Policy {
        start: start.start_of("2026-06-01"),                         // 18.0
        end: Some(start.start_of_week("2026-W27", WeekSystem::Epi)), // 52.0
        delay: 3.0,
    }],
    max_time: start.end_of("2026-08-05"),        // 84.0
};
```

These panic on a malformed string, since it is a literal in the source; use
`parse` for strings from input. To convert a twin exactly as reading a
document does, use `Parameters::try_from(wire)`.

## Writing dates in reports

`Date`, `Week` and `Weekday` serialize as `YYYY-MM-DD`, `YYYY-Www` and `Mon`
to `Sun`, and read back the same way, so each can be a report column. Store
the start date and week system on the context once, then ask for the
current date or week when building a row:

```rust
use ixa::calendar::{Date, Week, Weekday};
use ixa::prelude::*;
use serde::Serialize;

#[derive(Serialize)]
struct IncidenceRow {
    date: Option<Date>,
    weekday: Option<Weekday>,
    epi_week: Option<Week>,
    time: f64,
    count: usize,
}

define_report!(IncidenceRow);

fn init(context: &mut Context, parameters: &Parameters) -> Result<(), IxaError> {
    context.set_start_date(parameters.start_date);
    context.set_week_system(Parameters::WEEK_SYSTEM);
    context.add_report::<IncidenceRow>("incidence")?;
    Ok(())
}

fn report_incidence(context: &mut Context, count: usize) {
    let date = context.get_current_date();
    context.send_report(IncidenceRow {
        date,
        weekday: date.map(Date::weekday),
        epi_week: context.get_current_week(),
        time: context.get_current_time(),
        count,
    });
}
```

With a start date of `2026-05-14`, a row sent at time `18.5` is:

```text
date,weekday,epi_week,time,count
2026-06-01,Mon,2026-W22,18.5,2
```

- `set_start_date` takes a `Date` or an `Option<Date>`; `set_week_system`
  takes a `WeekSystem` or a root's `WEEK_SYSTEM` constant.
- `get_current_date` returns `None` without a start date, written as an
  empty cell. `get_current_week` also returns `None` without a week system.
- A time falls on the date of its whole day: `18.5` and `18.0` are the same
  date. Negative times give dates before the start date.
- For separate year and week columns, use `week.year()` and `week.number()`;
  for another system, `date.week(WeekSystem::Iso)`.
- `get_start_date` returns the stored date, and `start.date_at(time)` gives
  the date of any time.

## Limits

The twin copies the struct's derives and serde attributes, so `#[calendar]`
must sit above `#[derive(..)]`. A calendar struct:

- derives `Clone`, `Debug`, `PartialEq`, `Serialize` and `Deserialize`, and
  anything else its fields allow;
- has no generic parameters;
- cannot use serde's `try_from`, `from`, `into` or `remote` on the struct;
- cannot use serde's `with`, `deserialize_with`, `serialize_with` or
  `default = "..."` on a `#[calendar]` field, since the twin holds a `Day`
  or a twin there.

Unmarked fields take any serde attribute, and container attributes such as
`rename_all`, `deny_unknown_fields` and `default` apply as usual.

### Custom deserialization

To validate or post-process a document, wrap the calendar struct in a type
whose hand-written `Deserialize` reads it, then checks or transforms the
result. Sorting or filtering a `#[calendar]` list also goes here, since
`deserialize_with` is rejected on it.

```rust
use serde::de::{Deserializer, Error};

#[derive(Debug)]
struct ValidatedParameters { parameters: Parameters, first_start: f64 }

impl<'de> Deserialize<'de> for ValidatedParameters {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let parameters = Parameters::deserialize(d)?;
        if parameters.policies.iter().any(|p| p.start < 0.0) {
            return Err(Error::custom("a policy starts before the start date"));
        }
        let first_start = parameters.policies.iter().map(|p| p.start).fold(f64::INFINITY, f64::min);
        Ok(Self { parameters, first_start })
    }
}
```

A calendar struct that does not derive `Deserialize` keeps its own
implementation, but then no twin is read and dates are not resolved.

### Implementing `Calendar` by hand

A struct whose wire form does not fit the generated shape can implement
`Calendar` itself and still be a `#[calendar]` field. Convert each day field
with `Day::resolve` and `Day::from_days`, passing the context along with
that field's options: `FieldOptions::default()` for a point in time, or
e.g. `FieldOptions { inclusive: true, ..FieldOptions::default() }`. The
`field` argument is always the default for a struct. Derive `Serialize` and
`Deserialize` on both the struct and its twin, since binary formats read the
struct and text formats the twin:

```rust
use ixa::calendar::{Calendar, CalendarContext, CalendarError, Day, FieldOptions};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Importation { day: f64 }

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CalendarImportation { day: Day }

impl Calendar for Importation {
    type Wire = CalendarImportation;

    fn from_calendar(
        wire: CalendarImportation,
        cx: CalendarContext,
        _field: FieldOptions,
    ) -> Result<Self, CalendarError> {
        Ok(Self { day: wire.day.resolve(cx, FieldOptions::default())? })
    }

    fn to_calendar(&self, cx: CalendarContext, _field: FieldOptions) -> CalendarImportation {
        CalendarImportation { day: Day::from_days(self.day, cx, FieldOptions::default()) }
    }

    fn shift_days(&mut self, days: f64) {
        self.day += days;
    }
}
```
