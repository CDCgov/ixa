use criterion::{criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion};
use ixa::prelude::*;

define_entity!(BulkBenchPerson);
define_property!(struct Age(u8), BulkBenchPerson);
define_property!(struct HomeId(u32), BulkBenchPerson);
define_property!(struct SchoolId(u32), BulkBenchPerson);
define_property!(struct WorkplaceId(u32), BulkBenchPerson);

const ROW_COUNTS: [usize; 3] = [100, 10_000, 1_000_000];

#[derive(Clone, Copy)]
struct PersonRow {
    age: u8,
    home_id: u32,
    school_id: u32,
    workplace_id: u32,
}

fn generate_row(i: usize) -> PersonRow {
    PersonRow {
        age: (i % 100) as u8,
        home_id: (i % 10_000) as u32,
        school_id: (i % 1_000) as u32,
        workplace_id: (i % 2_000) as u32,
    }
}

fn make_rows(n: usize) -> Vec<PersonRow> {
    (0..n).map(generate_row).collect()
}

struct StreamingRows {
    current: usize,
    end: usize,
}

impl StreamingRows {
    fn new(n: usize) -> Self {
        Self { current: 0, end: n }
    }
}

impl Iterator for StreamingRows {
    type Item = PersonRow;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current >= self.end {
            None
        } else {
            let i = self.current;
            self.current += 1;
            Some(generate_row(i))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

fn insert_one_by_one<I>(context: &mut Context, rows: I)
where
    I: IntoIterator<Item = PersonRow>,
{
    for row in rows {
        context
            .add_entity((
                Age(row.age),
                HomeId(row.home_id),
                SchoolId(row.school_id),
                WorkplaceId(row.workplace_id),
            ))
            .unwrap();
    }
}

fn bench_bulk_entity_add_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("bulk_entity_add");

    for &n in &ROW_COUNTS {
        let pre_collected_rows = make_rows(n);
        group.bench_with_input(
            BenchmarkId::new("add_entity_loop_pre_collected", n),
            &n,
            |b, _| {
                b.iter_batched_ref(
                    Context::new,
                    |context| {
                        insert_one_by_one(context, pre_collected_rows.iter().copied());
                    },
                    BatchSize::LargeInput,
                )
            },
        );

        group.bench_with_input(
            BenchmarkId::new("add_entity_loop_streaming", n),
            &n,
            |b, &size| {
                b.iter_batched_ref(
                    Context::new,
                    |context| {
                        insert_one_by_one(context, StreamingRows::new(size));
                    },
                    BatchSize::LargeInput,
                )
            },
        );
    }

    group.finish();
}

criterion_group!(bulk_entity_add_benches, bench_bulk_entity_add_baseline);
criterion_main!(bulk_entity_add_benches);
