//! The `#[calendar]` attribute from a crate that uses ixa.

use ixa::calendar;
use ixa::calendar::{Calendar, CalendarError, Date, Day};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[calendar]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Policy {
    #[serde(default)]
    name: String,
    #[day]
    #[serde(default)]
    start: f64,
    #[day]
    #[serde(default)]
    end: Option<f64>,
    /// A duration: not marked.
    #[serde(default)]
    delay: f64,
}

#[calendar]
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EndRunConditions {
    #[day(inclusive)]
    max_time: f64,
    #[serde(default)]
    max_infections: Option<u64>,
}

/// Converts by hand, as a type with a custom wire shape does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Importation {
    day: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct CalendarImportation {
    day: Day,
}

impl Calendar for Importation {
    type Wire = CalendarImportation;

    fn from_calendar(
        wire: CalendarImportation,
        start: Option<Date>,
    ) -> Result<Self, CalendarError> {
        Ok(Self {
            day: wire.day.resolve(start, false)?,
        })
    }

    fn to_calendar(&self, start: Option<Date>) -> CalendarImportation {
        CalendarImportation {
            day: Day::from_days(self.day, start, false),
        }
    }
}

#[calendar]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Parameters {
    #[reference]
    #[serde(default)]
    start_date: Option<Date>,
    #[days]
    #[serde(default)]
    policies: Vec<Policy>,
    #[days]
    importations: Vec<Importation>,
    #[days]
    end_run_conditions: EndRunConditions,
    #[days]
    #[serde(default)]
    regions: Option<Policy>,
    seed: u64,
}

fn start() -> Option<Date> {
    Some("2026-05-14".parse().unwrap())
}

#[test]
fn parameters_can_be_written_in_code_with_dates() {
    let start = Date::new(2026, 5, 14).unwrap();
    let p = Parameters {
        start_date: Some(start),
        policies: vec![Policy {
            name: "Dated".into(),
            start: start.start_of("2026-06-01"),
            end: Some(start.start_of("2026-07-01")),
            delay: 3.0,
        }],
        importations: vec![],
        end_run_conditions: EndRunConditions {
            max_time: start.end_of("2026-08-05"),
            max_infections: None,
        },
        regions: None,
        seed: 7,
    };
    let from_json: Parameters = serde_json::from_value(json!({
        "startDate": "2026-05-14",
        "policies": [{"name": "Dated", "start": "2026-06-01", "end": "2026-07-01", "delay": 3}],
        "importations": [],
        "endRunConditions": {"maxTime": "2026-08-05"},
        "seed": 7
    }))
    .unwrap();
    assert_eq!(p, from_json);
}

#[test]
fn dates_become_days_from_the_start_date_and_back() {
    let p: Parameters = serde_json::from_value(json!({
        "startDate": "2026-05-14",
        "policies": [
            {"name": "Dated", "start": "2026-06-01", "end": "2026-07-01", "delay": 3},
            {"name": "Days", "start": 3, "end": null},
        ],
        "importations": [{"day": "2026-05-01"}, {"day": 4}],
        "endRunConditions": {"maxTime": "2026-08-05"},
        "regions": {"start": {"$__toml_private_datetime": "2026-05-24"}},
        "seed": 7
    }))
    .unwrap();
    assert_eq!((p.policies[0].start, p.policies[0].end), (18.0, Some(48.0)));
    assert_eq!(p.policies[0].delay, 3.0);
    assert_eq!((p.policies[1].start, p.policies[1].end), (3.0, None));
    assert_eq!(p.importations[0].day, -13.0);
    assert_eq!(p.importations[1].day, 4.0);
    assert_eq!(p.end_run_conditions.max_time, 84.0);
    assert_eq!(p.regions.as_ref().unwrap().start, 10.0);

    // Written as days by default.
    let days = serde_json::to_value(&p).unwrap();
    assert_eq!(days["policies"][0]["start"], 18.0);
    assert_eq!(days["importations"][0]["day"], -13.0);
    assert_eq!(days["endRunConditions"]["maxTime"], 84.0);
    assert_eq!(days["startDate"], "2026-05-14");
    // And as dates with `dated()`: whole days only, inclusive ends.
    let mut p = p;
    p.policies[1].start = 2.5;
    let dated = serde_json::to_value(p.dated()).unwrap();
    assert_eq!(dated["policies"][0]["start"], "2026-06-01");
    assert_eq!(dated["policies"][0]["end"], "2026-07-01");
    assert_eq!(dated["policies"][1]["start"], 2.5);
    assert_eq!(dated["policies"][1]["end"], serde_json::Value::Null);
    assert_eq!(dated["importations"][0]["day"], "2026-05-01");
    assert_eq!(dated["endRunConditions"]["maxTime"], "2026-08-05");
    assert_eq!(dated["regions"]["start"], "2026-05-24");
    // The dated form reads back to the same parameters.
    let back: Parameters = serde_json::from_value(dated).unwrap();
    assert_eq!(back, p);
}

#[test]
fn a_date_needs_a_start_date_and_a_bad_date_says_why() {
    let err = serde_json::from_value::<Parameters>(json!({
        "importations": [{"day": "2026-05-01"}],
        "endRunConditions": {"maxTime": 10},
        "seed": 1
    }))
    .unwrap_err()
    .to_string();
    assert!(
        err.contains("importations[0]: 2026-05-01 is a date, but there is no start date"),
        "{err}"
    );
    let err = serde_json::from_value::<Parameters>(json!({
        "startDate": "2026-05-14",
        "importations": [],
        "endRunConditions": {"maxTime": "2026-02-30"},
        "seed": 1
    }))
    .unwrap_err()
    .to_string();
    assert!(err.contains("day 30 is not 1 to 28"), "{err}");
    // A plain struct reads and writes days on its own.
    let p: Policy = serde_json::from_value(json!({"start": 5, "end": 9})).unwrap();
    assert_eq!(p.end, Some(9.0));
    assert_eq!(
        serde_json::to_value(&p).unwrap(),
        json!({"name": "", "start": 5.0, "end": 9.0, "delay": 0.0})
    );
    // A struct converts on its own, given the start date.
    let wire = p.to_calendar(start());
    assert_eq!(wire.start, Day::CalendarDate("2026-05-19".parse().unwrap()));
    assert_eq!(Policy::from_calendar(wire, start()).unwrap(), p);
    assert_eq!(CalendarPolicy::default().start, Day::DaysFromStart(0.0));
    let end: CalendarEndRunConditions = serde_json::from_value(json!({"maxTime": 3})).unwrap();
    assert_eq!(end.max_time, Day::DaysFromStart(3.0));
}

/// Only the attribute is imported: the expansion uses full paths.
mod bare {
    use ixa::calendar;
    use serde::{Deserialize, Serialize};

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Window {
        #[day]
        from: f64,
        #[day(inclusive)]
        to: Option<f64>,
    }

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Settings {
        #[reference]
        start: ixa::calendar::Date,
        #[days]
        windows: Vec<Window>,
        #[days]
        current: Option<Window>,
    }

    #[test]
    fn a_bare_date_reference_works_too() {
        let s: Settings = serde_json::from_str(
            r#"{"start": "2026-05-14", "windows": [{"from": "2026-05-15", "to": "2026-05-16"}], "current": null}"#,
        )
        .unwrap();
        assert_eq!((s.windows[0].from, s.windows[0].to), (1.0, Some(3.0)));
        assert_eq!(s.current, None);
        let dated = serde_json::to_value(s.dated()).unwrap();
        assert_eq!(dated["windows"][0]["to"], "2026-05-16");
    }
}

#[test]
fn native_toml_dates_and_quoted_dates_both_read() {
    let p: Parameters = toml::from_str(
        r#"
        startDate = 2026-05-14
        seed = 7
        importations = [{ day = 2026-05-01 }, { day = 4 }]
        endRunConditions = { maxTime = "2026-08-05" }

        [[policies]]
        name = "Dated"
        start = 2026-06-01
        end = 48
        "#,
    )
    .unwrap();
    assert_eq!(p.start_date, start());
    assert_eq!((p.policies[0].start, p.policies[0].end), (18.0, Some(48.0)));
    assert_eq!(p.importations[0].day, -13.0);
    assert_eq!(p.end_run_conditions.max_time, 84.0);

    let err = toml::from_str::<Parameters>(
        "startDate = 2026-05-14T00:00:00\nseed = 7\nimportations = []\nendRunConditions = { maxTime = 1 }",
    )
    .unwrap_err();
    assert!(err.message().contains("is not a date"), "{err}");
}

mod nested {
    use super::*;

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Region {
        #[reference]
        start: Option<Date>,
        #[day]
        x: f64,
    }

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Outer {
        #[reference]
        start: Option<Date>,
        #[days]
        regions: Vec<Region>,
    }

    #[test]
    fn a_nested_root_uses_its_own_start_date_or_the_outer_one() {
        let o: Outer = serde_json::from_value(json!({
            "start": "2026-01-01",
            "regions": [
                {"start": "2026-06-01", "x": "2026-06-02"},
                {"start": null, "x": "2026-01-03"},
            ]
        }))
        .unwrap();
        assert_eq!(o.regions[0].x, 1.0);
        assert_eq!(o.regions[1].x, 2.0);
        let dated = serde_json::to_value(o.dated()).unwrap();
        assert_eq!(dated["regions"][0]["x"], "2026-06-02");
        assert_eq!(dated["regions"][1]["x"], "2026-01-03");
    }

    #[test]
    fn errors_name_the_field() {
        let err = serde_json::from_value::<Outer>(json!({
            "start": null,
            "regions": [{"start": null, "x": 3}, {"start": null, "x": "2026-01-03"}]
        }))
        .unwrap_err()
        .to_string();
        assert!(
            err.starts_with("regions[1].x: 2026-01-03 is a date, but there is no start date"),
            "{err}"
        );
    }
}
