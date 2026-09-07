//! External fixture oracle. No private M4 types, decoder calls, or writes.
use arrow_array::{Array, Decimal128Array, Int64Array, StringArray};
use arrow_schema::{DataType, Field, Schema};
use parquet::{arrow::arrow_reader::ParquetRecordBatchReaderBuilder, basic::Compression};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    error::Error,
    fs::{self, File},
    io::Read,
    path::Path,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn require(ok: bool, message: &'static str) -> Result<()> {
    if ok { Ok(()) } else { Err(message.into()) }
}
pub fn digest(path: &Path) -> Result<(u64, String)> {
    regular(path, false)?;
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buf = [0; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        size = size.checked_add(n as u64).ok_or("size overflow")?;
    }
    Ok((size, format!("{:x}", hash.finalize())))
}
fn regular(path: &Path, directory: bool) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        require(meta.file_attributes() & 0x400 == 0, "reparse point")?;
    }
    require(
        !meta.file_type().is_symlink()
            && if directory {
                meta.is_dir()
            } else {
                meta.is_file()
            },
        "unexpected file type",
    )
}
fn hash_json(value: &impl Serialize) -> Result<String> {
    let canonical = serde_json::to_value(value)?; // serde_json default Map sorts recursively.
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&canonical)?)
    ))
}
fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn document<T: DeserializeOwned>(path: &Path) -> Result<T> {
    regular(path, false)?;
    let mut bytes = Vec::new();
    File::open(path)?.take(4097).read_to_end(&mut bytes)?;
    require(bytes.len() <= 4096, "oversize document")?;
    Ok(serde_json::from_slice(&bytes)?)
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    version: u32,
    input_bytes: u64,
    input_sha256: String,
    layout_sha256: String,
    record_length: u64,
    batch_records: u64,
    profile: String,
    job_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    version: u32,
    job_id: String,
    part_index: u64,
    start_record: u64,
    record_count: u64,
    parquet_bytes: u64,
    parquet_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Completion {
    version: u32,
    job_id: String,
    part_count: u64,
    total_records: u64,
}

fn schema() -> Schema {
    Schema::new(
        [
            ("CUSTOMER-NAME", DataType::Utf8),
            ("ACCOUNT-NUMBER", DataType::Int64),
            ("INTEREST-RATE", DataType::Decimal128(7, 2)),
            ("BALANCE-BIN", DataType::Int64),
            ("RATE-BIN", DataType::Decimal128(7, 2)),
            ("AMOUNT-PACKED", DataType::Decimal128(9, 2)),
        ]
        .into_iter()
        .map(|(n, t)| Field::new(format!("SAMPLE-RECORD.HEADER-GROUP.{n}"), t, false))
        .collect::<Vec<_>>(),
    )
}
pub fn parquet(path: &Path, start: u64, rows: u64, batch: u64) -> Result<()> {
    regular(path, false)?;
    require(batch > 0, "zero batch")?;
    let reader = ParquetRecordBatchReaderBuilder::try_new(File::open(path)?)?;
    require(
        reader.schema().as_ref() == &schema(),
        "schema differs from fixture",
    )?;
    require(
        u64::try_from(reader.metadata().file_metadata().num_rows())? == rows,
        "wrong row count",
    )?;
    require(
        u64::try_from(reader.metadata().num_row_groups())? == rows.div_ceil(batch),
        "wrong row groups",
    )?;
    let mut remaining = rows;
    for group in reader.metadata().row_groups() {
        let n = remaining.min(batch);
        require(
            u64::try_from(group.num_rows())? == n,
            "wrong row-group size",
        )?;
        remaining -= n;
        for col in group.columns() {
            require(
                col.compression() == Compression::UNCOMPRESSED,
                "unexpected compression",
            )?;
        }
    }
    let mut position = start;
    for batch in reader.with_batch_size(4096).build()? {
        let batch = batch?;
        for col in batch.columns() {
            col.to_data().validate_full()?;
            require(col.null_count() == 0, "unexpected null")?;
        }
        let texts = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or("text type")?;
        let numbers = batch
            .column(1)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or("int type")?;
        let interests = batch
            .column(2)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .ok_or("decimal type")?;
        let balances = batch
            .column(3)
            .as_any()
            .downcast_ref::<Int64Array>()
            .ok_or("int type")?;
        let rates = batch
            .column(4)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .ok_or("decimal type")?;
        let packed = batch
            .column(5)
            .as_any()
            .downcast_ref::<Decimal128Array>()
            .ok_or("decimal type")?;
        for row in 0..batch.num_rows() {
            let i = (position % 3) as usize;
            require(
                texts.value(row) == ["ALICE     ", "José      ", "\0\u{85}\n¤[]    "][i]
                    && numbers.value(row) == [42, 9999, 0][i]
                    && interests.value(row) == [12345, 9999999, 0][i]
                    && balances.value(row) == [-123, 9999, 0][i]
                    && rates.value(row) == [123456, 9999999, 0][i]
                    && packed.value(row) == [123456789, -123, 0][i],
                "fixture value or order differs",
            )?;
            position = position.checked_add(1).ok_or("position overflow")?;
        }
    }
    require(position.checked_sub(start) == Some(rows), "incomplete rows")
}
fn type_json(t: m2c_pipeline::LogicalType) -> Value {
    use m2c_pipeline::LogicalType::*;
    match t {
        Utf8 => json!({"type":"utf8"}),
        Int64 => json!({"type":"int64"}),
        Decimal128 { precision, scale } => {
            json!({"type":"decimal128","precision":precision,"scale":scale})
        }
    }
}
fn fixture_layout_hash() -> Result<String> {
    // Public compiler is used only for physical identity. Semantic oracle above is independent.
    use m2c_pipeline::PhysicalEncoding::*;
    let layout = m2c_pipeline::parse_and_compile_copybook(include_str!(
        "../../tests/fixtures/sample_fixed.cpy"
    ))?;
    let fields: Vec<_> = layout.fields.iter().map(|f| json!({"path":f.path,"source_name":f.source_name,
        "offset":f.offset,"byte_length":f.byte_length,"physical_encoding":match f.physical_encoding {
            EbcdicText=>"ebcdic_text",EbcdicDisplayNumeric=>"ebcdic_display_numeric",BigEndianBinary=>"big_endian_binary",PackedDecimal=>"packed_decimal"},
        "signed":f.signed,"precision":f.precision,"scale":f.scale,"logical_type":type_json(f.logical_type)})).collect();
    let arrow: Vec<_> = layout
        .fields
        .iter()
        .filter_map(|f| {
            f.path.as_ref().map(|name|
        json!({"name":name,"data_type":type_json(f.logical_type),"nullable":false,"metadata":{}}))
        })
        .collect();
    hash_json(
        &json!({"name":layout.name,"record_length":layout.record_length,"fields":fields,"arrow_fields":arrow,"arrow_metadata":{}}),
    )
}
fn namespace(path: &Path, expected: BTreeSet<String>) -> Result<()> {
    regular(path, true)?;
    let mut found = BTreeSet::new();
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        found.insert(
            entry
                .file_name()
                .into_string()
                .map_err(|_| "nonunicode artifact")?,
        );
    }
    require(found == expected, "unknown or missing artifact")
}
pub fn m4(root: &Path, input: &Path, rows: u64, batch: u64) -> Result<()> {
    require(batch > 0, "zero batch")?;
    namespace(
        root,
        [
            ".m4.lock",
            "manifest.json",
            "complete.json",
            "parts",
            "commits",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
    )?;
    regular(&root.join(".m4.lock"), false)?;
    let job: Manifest = document(&root.join("manifest.json"))?;
    require(
        job.format == "m2c-m4"
            && job.version == 1
            && job.profile == "m2c-v0.1-cp037-parquet53-uncompressed-v1",
        "unsupported manifest",
    )?;
    let (size, hash) = digest(input)?;
    require(
        job.input_bytes == size
            && size == rows.checked_mul(35).ok_or("size overflow")?
            && job.input_sha256 == hash
            && job.record_length == 35
            && job.batch_records == batch
            && job.layout_sha256 == fixture_layout_hash()?,
        "identity mismatch",
    )?;
    require(
        valid_hash(&job.job_id) && valid_hash(&job.layout_sha256) && valid_hash(&job.input_sha256),
        "invalid hash",
    )?;
    let mut identity = serde_json::to_value(&job)?;
    identity
        .as_object_mut()
        .ok_or("descriptor object")?
        .remove("job_id");
    require(job.job_id == hash_json(&identity)?, "job hash mismatch")?;
    let count = rows.div_ceil(batch).max(1);
    let complete: Completion = document(&root.join("complete.json"))?;
    require(
        complete.version == 1
            && complete.job_id == job.job_id
            && complete.part_count == count
            && complete.total_records == rows,
        "completion mismatch",
    )?;
    // Bounded iteration: directory counts first; no allocation from untrusted part_count.
    for directory in ["parts", "commits"] {
        regular(&root.join(directory), true)?;
        require(
            u64::try_from(fs::read_dir(root.join(directory))?.count())? == count,
            "artifact count mismatch",
        )?;
    }
    for i in 0..count {
        let start = i.checked_mul(batch).ok_or("range overflow")?;
        let n = rows.saturating_sub(start).min(batch);
        let receipt: Receipt = document(&root.join(format!("commits/part-{i:020}.json")))?;
        require(
            receipt.version == 1
                && receipt.job_id == job.job_id
                && receipt.part_index == i
                && receipt.start_record == start
                && receipt.record_count == n,
            "receipt range mismatch",
        )?;
        let part = root.join(format!("parts/part-{i:020}.parquet"));
        let (size, hash) = digest(&part)?;
        require(
            valid_hash(&receipt.parquet_sha256)
                && size == receipt.parquet_bytes
                && hash == receipt.parquet_sha256,
            "part hash mismatch",
        )?;
        parquet(&part, start, n, batch)?;
    }
    Ok(())
}
pub fn roundtrip(left: &Path, right: &Path) -> Result<()> {
    regular(left, false)?;
    regular(right, false)?;
    let mut left = File::open(left)?;
    let mut right = File::open(right)?;
    let mut remaining = left.metadata()?.len();
    require(remaining == right.metadata()?.len(), "length mismatch")?;
    let (mut a, mut b) = ([0; 65536], [0; 65536]);
    while remaining > 0 {
        let n = remaining.min(65536) as usize;
        left.read_exact(&mut a[..n])?;
        right.read_exact(&mut b[..n])?;
        require(a[..n] == b[..n], "byte mismatch")?;
        remaining -= n as u64;
    }
    require(
        left.read(&mut a[..1])? == 0 && right.read(&mut b[..1])? == 0,
        "file changed",
    )
}
