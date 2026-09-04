use m2c_pipeline::{parse_and_compile_copybook, RecordDecoder};
fn main() {
    let layout = parse_and_compile_copybook("       01 ROOT.\n       05 COUNT-FIELD PIC 9(2).\n").unwrap();
    let decoder = RecordDecoder::try_new(&layout).unwrap();
    println!("OK");
}
