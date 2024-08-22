pub trait Report: 'static {
    fn type_id(&self) -> TypeId;
    fn serialize(&self, writer: &mut Writer<File>);
}

macro_rules! create_report_trait {
    ($name:ident) => {
        impl Report for $name {
            fn type_id(&self) -> TypeId {
                // returns the TypeId of the report (used for identification)
                TypeId::of::<$name>()
            }

            fn serialize(&self, writer: &mut Writer<File>) {
                writer.serialize(self).unwrap();
            }
        }
    };
}

struct ReportData {
    file_writers: RefCell<HashMap<TypeId, Writer<File>>>,
}

crate::context::define_data_plugin!(
    ReportPlugin,
    ReportData,
    ReportData {
        file_writers: RefCell::new(HashMap::new()),
    }
);

pub trait ContextReport {
    fn add_report<T: Report + 'static>(&self, short_name: &str);
    fn send_report<T: Report>(&self, report: T);
}

impl ContextReport for Context {
    fn add_report<T: Report + 'static>(&self, short_name: &str) {
        let data_container = self.get_data_container_mut::<ReportPlugin>();

        let filename = format!("{}_{}.csv", self.name, short_name);
        let path = Path::new(&filename);
        let file = File::create(path).expect("Couldn't create file");
        let writer = Writer::from_writer(file);
        let mut file_writer = data_container.file_writers.try_borrow_mut().unwrap();
        file_writer.insert(TypeId::of::<T>(), writer);
    }

    fn send_report<T: Report>(&self, report: T) {
        let data_container = self.get_data_container_mut::<ReportPlugin>();

        if let Some(writer) = data_container
            .file_writers
            .borrow_mut()
            .unwrap()
            .get_mut(&report.type_id())
        {
            report.serialize(writer);
        } else {
            panic!("No writer found for the report type");
        }
    }
}
