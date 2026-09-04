use std::fs::File;
use std::sync::Arc;
use arrow_schema::{Schema, Field, DataType};
use parquet::arrow::ArrowWriter;
use parquet::file::properties::WriterProperties;

fn main() {
    let file = File::create("test.parquet").unwrap();
    let schema = Arc::new(Schema::new(vec![
        Field::new("col", DataType::Int64, false)
    ]));
    let properties = WriterProperties::builder()
        .set_max_row_group_size(usize::MAX)
        .build();
    let _writer = ArrowWriter::try_new(file, schema, Some(properties)).unwrap();
    println!("OK");
}
