use m2c_pipeline::{RecordDecoder, parse_and_compile_copybook};
use std::{
    error::Error,
    hint::black_box,
    time::{Duration, Instant},
};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args()
        .skip(1)
        .filter(|a| a != "--bench")
        .collect();
    // Cargo runs custom bench targets under --all-targets; no opt-in means no measurement.
    if args.is_empty() || args.iter().any(|a| a == "--test") {
        return Ok(());
    }
    if args.len() != 2 || args[0] != "--profile" || !["smoke", "full"].contains(&args[1].as_str()) {
        return Err("use --profile smoke|full".into());
    }
    let full = args[1] == "full";
    let samples = if full { 7 } else { 3 };
    let window = Duration::from_millis(if full { 250 } else { 25 });
    for (workload, source, input, rows) in [
        (
            "mixed",
            include_str!("../tests/fixtures/sample_fixed.cpy"),
            include_bytes!("../tests/fixtures/sample_fixed.bin").as_slice(),
            768,
        ),
        (
            "text",
            "       01 ROOT.\n       05 TEXT-FIELD PIC X(4).\n",
            &[0xC1, 0x51, 0x40, 0x00][..],
            256,
        ),
        (
            "numeric",
            "       01 ROOT.\n       05 D PIC 9(2).\n       05 B PIC S9(4) COMP.\n       05 P PIC S9(3)V9(2) COMP-3.\n",
            &[0xF4, 0xF2, 0xFF, 0x85, 0x12, 0x34, 0x5D][..],
            256,
        ),
    ] {
        let layout = parse_and_compile_copybook(source)?;
        let decoder = RecordDecoder::try_new(&layout)?;
        let records = input.repeat(256);
        for operation in ["compile", "decode"] {
            for sample in 0..=samples {
                let start = Instant::now();
                let mut iterations = 0_u64;
                loop {
                    if operation == "compile" {
                        black_box(parse_and_compile_copybook(black_box(source))?);
                    } else {
                        black_box(decoder.decode_batch(black_box(&records))?);
                    }
                    iterations += 1;
                    if start.elapsed() >= window {
                        break;
                    }
                }
                let elapsed = start.elapsed();
                let check = decoder.decode_batch(&records)?;
                if check.num_rows() != rows || check.schema().as_ref() != &layout.arrow_schema {
                    return Err("verification failed".into());
                }
                verify_values(&check, workload)?;
                println!(
                    "{}",
                    serde_json::json!({"operation":operation,"workload":workload,"sample":sample,"warmup":sample==0,
                "iterations":iterations,"elapsed_ns":elapsed.as_nanos(),"ns_per_iteration":elapsed.as_nanos()/u128::from(iterations),
                "records_per_iteration":if operation=="decode"{Some(rows)}else{None},"input_bytes_per_iteration":if operation=="decode"{records.len()}else{source.len()},"profile":args[1]})
                );
            }
        }
    }
    Ok(())
}

fn verify_values(batch: &arrow_array::RecordBatch, workload: &str) -> Result<(), Box<dyn Error>> {
    use arrow_array::{Array, Decimal128Array, Int64Array, StringArray};
    use arrow_schema::DataType::{Decimal128, Int64, Utf8};
    let expected_types = match workload {
        "text" => vec![Utf8],
        "numeric" => vec![Int64, Int64, Decimal128(5, 2)],
        _ => vec![
            Utf8,
            Int64,
            Decimal128(7, 2),
            Int64,
            Decimal128(7, 2),
            Decimal128(9, 2),
        ],
    };
    if batch
        .schema()
        .fields()
        .iter()
        .map(|f| f.data_type())
        .ne(expected_types.iter())
    {
        return Err("independent logical types differ".into());
    }
    for column in batch.columns() {
        column.to_data().validate_full()?;
        if column.null_count() != 0 {
            return Err("unexpected null".into());
        }
    }
    let integer = |col: usize, row: usize| {
        batch
            .column(col)
            .as_any()
            .downcast_ref::<Int64Array>()
            .map(|a| a.value(row))
            .ok_or("integer type")
    };
    let decimal = |col: usize, row: usize| {
        batch
            .column(col)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .map(|a| a.value(row))
            .ok_or("decimal type")
    };
    let text = |col: usize, row: usize| {
        batch
            .column(col)
            .as_any()
            .downcast_ref::<StringArray>()
            .map(|a| a.value(row))
            .ok_or("text type")
    };
    for row in 0..batch.num_rows() {
        let i = row % 3;
        let valid = match workload {
            "text" => text(0, row)? == "Aé \0",
            "numeric" => {
                integer(0, row)? == 42 && integer(1, row)? == -123 && decimal(2, row)? == -12345
            }
            _ => {
                text(0, row)? == ["ALICE     ", "José      ", "\0\u{85}\n¤[]    "][i]
                    && integer(1, row)? == [42, 9999, 0][i]
                    && decimal(2, row)? == [12345, 9999999, 0][i]
                    && integer(3, row)? == [-123, 9999, 0][i]
                    && decimal(4, row)? == [123456, 9999999, 0][i]
                    && decimal(5, row)? == [123456789, -123, 0][i]
            }
        };
        if !valid {
            return Err("independent value oracle failed".into());
        }
    }
    Ok(())
}
