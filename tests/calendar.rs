//! The `#[calendar]` attribute from a crate that uses ixa.

use ixa::calendar;
use ixa::calendar::{
    Calendar, CalendarContext, CalendarError, Date, Day, FieldOptions, WeekSystem,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[calendar]
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Policy {
    #[serde(default)]
    name: String,
    #[calendar]
    #[serde(default)]
    start: f64,
    #[calendar]
    #[serde(default)]
    end: Option<f64>,
    #[calendar(duration)]
    #[serde(default)]
    delay: f64,
}

#[calendar]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EndRunConditions {
    #[calendar(inclusive)]
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
        cx: CalendarContext,
        _field: FieldOptions,
    ) -> Result<Self, CalendarError> {
        Ok(Self {
            day: wire.day.resolve(cx, FieldOptions::default())?,
        })
    }

    fn to_calendar(&self, cx: CalendarContext, _field: FieldOptions) -> CalendarImportation {
        CalendarImportation {
            day: Day::from_days(self.day, cx, FieldOptions::default()),
        }
    }

    fn shift_days(&mut self, days: f64) {
        self.day += days;
    }
}

#[calendar]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Parameters {
    #[calendar(reference)]
    #[serde(default)]
    start_date: Option<Date>,
    #[calendar]
    #[serde(default)]
    policies: Vec<Policy>,
    #[calendar]
    importations: Vec<Importation>,
    #[calendar]
    end_run_conditions: EndRunConditions,
    #[calendar]
    #[serde(default)]
    regions: Option<Policy>,
    seed: u64,
}

fn start() -> CalendarContext {
    CalendarContext {
        start: Some("2026-05-14".parse().unwrap()),
        weeks: None,
    }
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
    let wire = p.to_calendar(start(), FieldOptions::default());
    assert_eq!(wire.start, Day::CalendarDate("2026-05-19".parse().unwrap()));
    assert_eq!(
        Policy::from_calendar(wire, start(), FieldOptions::default()).unwrap(),
        p
    );
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
        #[calendar]
        from: f64,
        #[calendar(inclusive)]
        to: Option<f64>,
    }

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Settings {
        #[calendar(reference)]
        start: ixa::calendar::Date,
        #[calendar]
        windows: Vec<Window>,
        #[calendar]
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
    assert_eq!(p.start_date, start().start);
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
        #[calendar(reference)]
        start: Option<Date>,
        #[calendar]
        x: f64,
    }

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Outer {
        #[calendar(reference)]
        start: Option<Date>,
        #[calendar]
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

    #[test]
    fn moving_day_zero_keeps_every_date_including_nested_roots() {
        let mut o: Outer = serde_json::from_value(json!({
            "start": "2026-01-01",
            "regions": [
                {"start": "2026-02-01", "x": "2026-03-01"},
                {"start": null, "x": "2026-01-03"},
            ]
        }))
        .unwrap();
        let before = serde_json::to_value(o.dated()).unwrap();
        o.shift_days(30.0);
        o.shift_start_dates(-30).unwrap();
        assert_eq!(o.start, Some("2025-12-02".parse().unwrap()));
        assert_eq!(o.regions[0].start, Some("2026-01-02".parse().unwrap()));
        assert_eq!(o.regions[1].start, None);
        let after = serde_json::to_value(o.dated()).unwrap();
        for i in 0..2 {
            assert_eq!(after["regions"][i]["x"], before["regions"][i]["x"]);
        }
    }

    #[test]
    fn a_start_date_moved_out_of_range_names_the_field() {
        let mut o: Outer = serde_json::from_value(json!({
            "start": "2026-01-01",
            "regions": [{"start": "0000-01-05", "x": 0}]
        }))
        .unwrap();
        let err = o.shift_start_dates(-30).unwrap_err().to_string();
        assert_eq!(
            err,
            "regions[0].start: 0000-01-05 moved -30 days is outside years 0 to 9999"
        );
    }
}

#[test]
fn a_root_round_trips_through_a_binary_format() {
    let config = bincode_next::config::standard();
    let p: Parameters = serde_json::from_value(json!({
        "startDate": "2026-05-14",
        "policies": [{"start": "2026-06-01", "end": "2026-07-02"}, {"start": 2.5}],
        "importations": [{"day": 3.0}],
        "endRunConditions": {"maxTime": "2026-08-06"},
        "regions": {"name": "north", "start": 1.0},
        "seed": 7
    }))
    .unwrap();

    let bytes = bincode_next::serde::encode_to_vec(&p, config).unwrap();
    let (back, _): (Parameters, _) =
        bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
    assert_eq!(back, p);

    let bytes = bincode_next::serde::encode_to_vec(p.dated(), config).unwrap();
    let (dated, _): (CalendarParameters, _) =
        bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
    assert_eq!(Parameters::try_from(dated).unwrap(), p);
}

#[test]
fn a_struct_without_a_start_date_reads_units_on_its_own() {
    let p: Policy =
        serde_json::from_value(json!({"start": "2w", "end": 20, "delay": "1d"})).unwrap();
    assert_eq!((p.start, p.end, p.delay), (14.0, Some(20.0), 1.0));

    let err = serde_json::from_value::<Policy>(json!({"start": "2026-06-01"}))
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("start: 2026-06-01 is a date, but there is no start date"),
        "{err}"
    );
    let err = serde_json::from_value::<Policy>(json!({"delay": -1}))
        .unwrap_err()
        .to_string();
    assert!(
        err.starts_with("delay: a duration cannot be negative: -1"),
        "{err}"
    );

    let config = bincode_next::config::standard();
    let bytes = bincode_next::serde::encode_to_vec(&p, config).unwrap();
    let (back, _): (Policy, _) = bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
    assert_eq!(back, p);
}

#[calendar]
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadOnly {
    #[calendar(reference)]
    start_date: Option<Date>,
    #[calendar]
    #[serde(default)]
    max_time: f64,
}

#[test]
fn a_root_may_derive_deserialize_without_serialize() {
    let p: ReadOnly =
        serde_json::from_value(json!({"startDate": "2026-01-01", "maxTime": "2026-01-03"}))
            .unwrap();
    assert_eq!(p.max_time, 2.0);
    assert_eq!(
        serde_json::from_value::<ReadOnly>(json!({}))
            .unwrap()
            .max_time,
        0.0
    );
}

mod weeks {
    use super::*;

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Campaign {
        #[calendar(week)]
        start: f64,
        #[calendar(week, inclusive)]
        end: Option<f64>,
        #[calendar(duration)]
        #[serde(default)]
        ramp: f64,
        #[calendar(duration, signed)]
        #[serde(default)]
        lead: f64,
        #[calendar]
        #[serde(default)]
        report_days: Vec<f64>,
    }

    #[calendar(weeks = "epi")]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Parameters {
        #[calendar(reference)]
        start_date: Option<Date>,
        #[calendar]
        campaigns: Vec<Campaign>,
        #[calendar(inclusive)]
        max_time: f64,
    }

    #[calendar(weeks = "iso")]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct IsoParameters {
        #[calendar(reference)]
        start_date: Option<Date>,
        #[calendar]
        campaigns: Vec<Campaign>,
    }

    #[calendar]
    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct NoWeeks {
        #[calendar(reference)]
        start_date: Option<Date>,
        #[calendar]
        campaigns: Vec<Campaign>,
    }

    fn document() -> serde_json::Value {
        json!({
            "start_date": "2026-05-14",
            "campaigns": [
                {"start": "2026-W27", "end": "2026-W30", "ramp": "1w 2d", "lead": "-3d",
                 "report_days": ["2026-06-01", "2026-W28", "10d", 3]},
                {"start": 2.5, "end": "2026-08-01", "ramp": "36h"},
            ],
            "max_time": "26w"
        })
    }

    #[test]
    fn days_read_as_dates_weeks_or_units() {
        let p: Parameters = serde_json::from_value(document()).unwrap();
        let c = &p.campaigns[0];
        assert_eq!((c.start, c.end), (52.0, Some(80.0)));
        assert_eq!((c.ramp, c.lead), (9.0, -3.0));
        assert_eq!(c.report_days, vec![18.0, 59.0, 10.0, 3.0]);
        assert_eq!(p.campaigns[1].end, Some(80.0));
        assert_eq!(p.campaigns[1].ramp, 1.5);
        assert_eq!(p.max_time, 182.0);
        assert_eq!(Parameters::WEEK_SYSTEM, Some(WeekSystem::Epi));

        // The root's week system is passed down.
        let iso: IsoParameters = serde_json::from_value(document()).unwrap();
        assert_eq!(
            (iso.campaigns[0].start, iso.campaigns[0].end),
            (46.0, Some(74.0))
        );
    }

    #[test]
    fn dated_writes_weeks_where_a_field_asks_for_them() {
        let p: Parameters = serde_json::from_value(document()).unwrap();
        let dated = serde_json::to_value(p.dated()).unwrap();
        let c = &dated["campaigns"][0];
        assert_eq!(c["start"], "2026-W27");
        assert_eq!(c["end"], "2026-W30");
        assert_eq!(c["ramp"], 9.0);
        assert_eq!(
            c["report_days"],
            json!(["2026-06-01", "2026-07-12", "2026-05-24", "2026-05-17"])
        );
        // An inclusive end on a week boundary is written as that week.
        assert_eq!(dated["campaigns"][1]["end"], "2026-W30");
        assert_eq!(dated["max_time"], "2026-11-11");
        let back: Parameters = serde_json::from_value(dated).unwrap();
        assert_eq!(back, p);
    }

    #[test]
    fn shift_days_moves_points_but_not_durations() {
        let mut p: Parameters = serde_json::from_value(document()).unwrap();
        p.shift_days(7.0);
        let c = &p.campaigns[0];
        assert_eq!((c.start, c.end), (59.0, Some(87.0)));
        assert_eq!((c.ramp, c.lead), (9.0, -3.0));
        assert_eq!(c.report_days, vec![25.0, 66.0, 17.0, 10.0]);
        assert_eq!(p.max_time, 189.0);
        assert_eq!(p.start_date, Some("2026-05-14".parse().unwrap()));
    }

    #[test]
    fn errors_say_what_is_wrong_where() {
        let err = |doc: serde_json::Value| {
            serde_json::from_value::<Parameters>(doc)
                .unwrap_err()
                .to_string()
        };
        let mut doc = document();
        doc["campaigns"][0]["ramp"] = json!("2026-W01");
        assert!(err(doc).starts_with("campaigns[0].ramp: a duration cannot be the week 2026-W01"));
        let mut doc = document();
        doc["campaigns"][1]["ramp"] = json!(-1);
        assert!(err(doc).starts_with("campaigns[1].ramp: a duration cannot be negative: -1"));
        let mut doc = document();
        doc["campaigns"][0]["start"] = json!("2026-W53");
        assert!(err(doc).starts_with("campaigns[0].start: 2026-W53: 2026 has 52 epi weeks"));
        let mut doc = document();
        doc["max_time"] = json!("6 months");
        assert!(err(doc).contains("months and years have no fixed length"));

        let e = serde_json::from_value::<NoWeeks>(document())
            .unwrap_err()
            .to_string();
        assert!(
            e.starts_with("campaigns[0].start: 2026-W27 is a week, but no week system is set"),
            "{e}"
        );
        assert_eq!(NoWeeks::WEEK_SYSTEM, None);
    }

    #[test]
    fn toml_reads_weeks_and_units() {
        let p: Parameters = toml::from_str(
            r#"
            start_date = 2026-05-14
            max_time = "26w"

            [[campaigns]]
            start = "2026-W27"
            ramp = "2w"
            report_days = [2026-06-01, "2026-W28"]
            "#,
        )
        .unwrap();
        assert_eq!(p.campaigns[0].start, 52.0);
        assert_eq!(p.campaigns[0].ramp, 14.0);
        assert_eq!(p.campaigns[0].report_days, vec![18.0, 59.0]);
    }

    #[test]
    fn weeks_round_trip_through_a_binary_format() {
        let config = bincode_next::config::standard();
        let p: Parameters = serde_json::from_value(document()).unwrap();
        let bytes = bincode_next::serde::encode_to_vec(p.dated(), config).unwrap();
        let (dated, _): (CalendarParameters, _) =
            bincode_next::serde::decode_from_slice(&bytes, config).unwrap();
        assert_eq!(Parameters::try_from(dated).unwrap(), p);
    }
}
