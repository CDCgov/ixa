#[cfg(test)]
mod tests {
    use ixa::prelude::*;
    use serde::{Deserialize, Serialize};

    define_entity!(Person);

    // Test entity property / derived / multi-property macros
    define_property!(struct TestPropU32(u32), Person);
    define_property!(struct TestPropU32b(u32), Person);
    define_property!(
        struct TestPropDefault(u32),
        Person,
        default_const = TestPropDefault(7u32)
    );
    define_property!(
        struct TestPropOpt(Option<u8>),
        Person,
        default_const = TestPropOpt(None)
    );

    define_derived_property!(struct DerivedProp(pub u32), Person, [TestPropU32], |v| {
        let v: TestPropU32 = v;
        DerivedProp(v.0 + 1)
    });

    type MultiProp = (TestPropU32, TestPropU32b);
    define_multi_property!((TestPropU32, TestPropU32b), Person);

    // Test global property macro
    define_global_property!(TestGlobal, u32);

    // Test edge type macro
    define_edge_type!(struct TestEdge, Person);

    // Test report macro
    #[derive(Serialize, Deserialize)]
    struct SampleR {
        x: u32,
    }
    define_report!(SampleR);

    // Test data plugin macro (simple container)
    define_data_plugin!(TestDataPlugin, Vec<u8>, vec![1u8, 2u8]);

    // Test rng macro
    define_rng!(TestRngId);

    #[test]
    fn compile_and_run_macros() {
        let mut ctx = Context::new();

        // Check global property registration works (add and set)
        ctx.set_global_property_value(TestGlobal, 42u32).unwrap();
        let v = *ctx.get_global_property_value(TestGlobal).unwrap();
        assert_eq!(v, 42u32);

        // Entity properties: add a Person with TestPropU32
        let pid: EntityId<Person> = ctx
            .add_entity((
                TestPropU32(10u32),
                TestPropU32b(20u32),
                TestPropOpt(Some(3u8)),
            ))
            .unwrap();
        let val: TestPropU32 = ctx.get_property(pid);
        assert_eq!(val.0, 10u32);
        // Verify default property value is set for TestPropDefault
        let default_val: TestPropDefault = ctx.get_property(pid);
        assert_eq!(default_val.0, 7u32);

        // Derived property should compute from TestPropU32
        let d: DerivedProp = ctx.get_property(pid);
        assert_eq!(d.0, 11u32);

        // Multi property - basic sanity (canonical value types)
        let multi = <MultiProp as Property<Person>>::make_canonical((
            TestPropU32(10u32),
            TestPropU32b(20u32),
        ));
        let disp = <MultiProp as Property<Person>>::get_display(&multi);
        assert!(disp.contains("10"));

        // Edge type (entity-based network): create two people and add an edge of type TestEdge
        let p1 = ctx
            .add_entity((TestPropU32(1u32), TestPropU32b(1u32)))
            .unwrap();
        let p2 = ctx
            .add_entity((TestPropU32(2u32), TestPropU32b(2u32)))
            .unwrap();
        ctx.add_edge::<Person, TestEdge>(p1, p2, 1.0, TestEdge)
            .unwrap();
        let e = ctx.get_edge::<Person, TestEdge>(p1, p2).unwrap();
        assert_eq!(e.weight, 1.0);
        // Now remove the edge and ensure it's gone
        ctx.remove_edge::<Person, TestEdge>(p1, p2);
        assert!(ctx.get_edge::<Person, TestEdge>(p1, p2).is_none());

        // Data plugin access
        let data: &Vec<u8> = ctx.get_data(TestDataPlugin);
        assert_eq!(data.len(), 2);

        // RNG macro: initialize random subsystem and sample to ensure macro-generated types work
        ctx.init_random(42);
        let _ = ctx.sample(TestRngId, |_rng| 1u32);

        // Report: write into a temp directory so existing files don't interfere
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().to_path_buf();
        let opts = ctx.report_options();
        opts.directory(path.clone());
        opts.overwrite(true);

        ctx.add_report::<SampleR>("sample_report").unwrap();
        ctx.send_report(SampleR { x: 5 });

        // Flush the writer so data is on disk, then verify CSV file exists and contains the sample row
        let mut writer = ctx.get_writer(std::any::TypeId::of::<SampleR>());
        writer.flush().unwrap();

        let file_path = path.join("sample_report.csv");
        let contents = std::fs::read_to_string(&file_path).unwrap();
        assert!(!contents.is_empty());
        assert!(contents.contains("5"));

        // assert_almost_eq macro usage
        let a = 1.0f64;
        let b = 1.0f64 + 1e-12;
        ixa::assert_almost_eq!(a, b, 1e-10);
    }
}
