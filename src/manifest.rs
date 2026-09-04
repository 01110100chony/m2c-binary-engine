//! Versioned M4 metadata and content identities. No filesystem lifecycle lives here.

use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use arrow_schema::DataType;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};

use crate::recovery::RecoveryError;
use crate::{CompiledCopybook, LogicalType, PhysicalEncoding};

pub(crate) const VERSION: u32 = 1;
pub(crate) const PROFILE: &str = "m2c-v0.1-cp037-parquet53-uncompressed-v1";
const FORMAT: &str = "m2c-m4";
const MAX_JSON_BYTES: usize = 4096;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Manifest {
    pub format: String,
    pub version: u32,
    pub input_bytes: u64,
    pub input_sha256: String,
    pub layout_sha256: String,
    pub record_length: u64,
    pub batch_records: u64,
    pub profile: String,
    pub job_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    pub version: u32,
    pub job_id: String,
    pub part_index: u64,
    pub start_record: u64,
    pub record_count: u64,
    pub parquet_bytes: u64,
    pub parquet_sha256: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Completion {
    pub version: u32,
    pub job_id: String,
    pub part_count: u64,
    pub total_records: u64,
}

fn invalid(reason: &'static str) -> RecoveryError {
    RecoveryError::InvalidState {
        path: PathBuf::new(),
        reason,
    }
}

fn check_version(version: u32) -> Result<(), RecoveryError> {
    if version != VERSION {
        return Err(RecoveryError::UnsupportedVersion { version });
    }
    Ok(())
}

impl Manifest {
    pub(crate) fn new(
        input_bytes: u64,
        input_sha256: String,
        layout_sha256: String,
        record_length: u64,
        batch_records: u64,
    ) -> Result<Self, RecoveryError> {
        let mut manifest = Self {
            format: FORMAT.to_owned(),
            version: VERSION,
            input_bytes,
            input_sha256,
            layout_sha256,
            record_length,
            batch_records,
            profile: PROFILE.to_owned(),
            job_id: String::new(),
        };
        manifest.validate_dimensions()?;
        manifest.job_id = manifest.expected_job_id()?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub(crate) fn validate(&self) -> Result<(), RecoveryError> {
        check_version(self.version)?;
        if self.profile != PROFILE {
            return Err(RecoveryError::UnsupportedProfile {
                profile: self.profile.clone(),
            });
        }
        if self.format != FORMAT {
            return Err(invalid("unknown manifest format"));
        }
        self.validate_dimensions()?;
        if !valid_hash(&self.input_sha256)
            || !valid_hash(&self.layout_sha256)
            || !valid_hash(&self.job_id)
        {
            return Err(invalid(
                "hash must contain 64 lowercase hexadecimal characters",
            ));
        }
        if self.job_id != self.expected_job_id()? {
            return Err(invalid("manifest job identity does not match its contents"));
        }
        Ok(())
    }

    fn validate_dimensions(&self) -> Result<(), RecoveryError> {
        if self.record_length == 0 || self.batch_records == 0 {
            return Err(invalid("record length and batch size must be positive"));
        }
        if !self.input_bytes.is_multiple_of(self.record_length) {
            return Err(invalid("input length is not a multiple of record length"));
        }
        self.record_length
            .checked_mul(self.batch_records)
            .ok_or_else(|| invalid("batch byte capacity overflow"))?;
        Ok(())
    }

    fn expected_job_id(&self) -> Result<String, RecoveryError> {
        let mut value = serde_json::to_value(self).map_err(json_error)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| invalid("manifest must be a JSON object"))?;
        object.remove("job_id");
        canonical_hash(&value)
    }

    pub(crate) fn total_records(&self) -> u64 {
        // Deserialized metadata has not necessarily been validated yet.
        self.input_bytes
            .checked_div(self.record_length)
            .unwrap_or_default()
    }

    pub(crate) fn part_count(&self) -> u64 {
        // ceil(N / B) without N + B - 1, which could overflow for valid inputs.
        self.total_records()
            .checked_sub(1)
            .and_then(|last_record| last_record.checked_div(self.batch_records))
            .and_then(|last_part| last_part.checked_add(1))
            .unwrap_or(1)
    }

    pub(crate) fn range(&self, index: u64) -> Result<(u64, u64), RecoveryError> {
        self.validate_dimensions()?;
        if index >= self.part_count() {
            return Err(invalid("part index exceeds the conversion range"));
        }
        let start = index
            .checked_mul(self.batch_records)
            .ok_or_else(|| invalid("part start record overflow"))?;
        let remaining = self
            .total_records()
            .checked_sub(start)
            .ok_or_else(|| invalid("part starts beyond the input"))?;
        start
            .checked_mul(self.record_length)
            .ok_or_else(|| invalid("part input offset overflow"))?;
        Ok((start, remaining.min(self.batch_records)))
    }
}

impl Receipt {
    pub(crate) fn validate(&self, manifest: &Manifest, index: u64) -> Result<(), RecoveryError> {
        check_version(self.version)?;
        if !valid_hash(&self.job_id) || !valid_hash(&self.parquet_sha256) {
            return Err(invalid("receipt contains an invalid hash"));
        }
        if self.job_id != manifest.job_id || self.part_index != index {
            return Err(invalid(
                "receipt identity does not match its job and filename",
            ));
        }
        let (start, count) = manifest.range(index)?;
        if self.start_record != start || self.record_count != count {
            return Err(invalid(
                "receipt does not match its deterministic record range",
            ));
        }
        if self.parquet_bytes == 0 {
            return Err(invalid("a Parquet artifact cannot be empty"));
        }
        Ok(())
    }
}

impl Completion {
    pub(crate) fn validate(&self, manifest: &Manifest) -> Result<(), RecoveryError> {
        check_version(self.version)?;
        if !valid_hash(&self.job_id) || self.job_id != manifest.job_id {
            return Err(invalid("completion identity does not match its job"));
        }
        if self.part_count != manifest.part_count()
            || self.total_records != manifest.total_records()
        {
            return Err(invalid("completion does not cover the input exactly"));
        }
        Ok(())
    }
}

pub(crate) fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(crate) fn hash_reader(reader: &mut impl Read) -> io::Result<(u64, String)> {
    let mut buffer = [0_u8; 64 * 1024];
    let mut count = 0_u64;
    let mut hasher = Sha256::new();
    loop {
        let read = match reader.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if read == 0 {
            break;
        }
        count = count
            .checked_add(u64::try_from(read).map_err(|_| io::Error::other("read size overflow"))?)
            .ok_or_else(|| io::Error::other("input byte count overflow"))?;
        hasher.update(&buffer[..read]);
    }
    Ok((count, format!("{:x}", hasher.finalize())))
}

fn json_error(source: serde_json::Error) -> RecoveryError {
    RecoveryError::Metadata {
        path: PathBuf::new(),
        source,
    }
}

pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, RecoveryError> {
    let file = File::open(path).map_err(|source| RecoveryError::Io {
        operation: "open metadata",
        path: path.to_owned(),
        source,
    })?;
    let mut bytes = Vec::with_capacity(MAX_JSON_BYTES + 1);
    file.take((MAX_JSON_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|source| RecoveryError::Io {
            operation: "read metadata",
            path: path.to_owned(),
            source,
        })?;
    if bytes.len() > MAX_JSON_BYTES {
        return Err(RecoveryError::InvalidState {
            path: path.to_owned(),
            reason: "metadata exceeds the 4096-byte limit",
        });
    }
    serde_json::from_slice(&bytes).map_err(|source| RecoveryError::Metadata {
        path: path.to_owned(),
        source,
    })
}

pub(crate) fn json_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, RecoveryError> {
    struct LimitedJson(Vec<u8>);
    impl Write for LimitedJson {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if bytes.len() > MAX_JSON_BYTES - self.0.len() {
                return Err(io::Error::other("metadata exceeds the 4096-byte limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = LimitedJson(Vec::new());
    serde_json::to_writer(&mut writer, value).map_err(json_error)?;
    Ok(writer.0)
}

// serde_json's default Map is a BTreeMap, including nested objects produced by
// to_value. Do not enable preserve_order: canonical identities require sorted keys.
fn canonical_hash(value: &impl Serialize) -> Result<String, RecoveryError> {
    let value = serde_json::to_value(value).map_err(json_error)?;
    let bytes = serde_json::to_vec(&value).map_err(json_error)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum EncodingIdentity {
    EbcdicText,
    EbcdicDisplayNumeric,
    BigEndianBinary,
    PackedDecimal,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TypeIdentity {
    Utf8,
    Int64,
    Decimal128 { precision: u8, scale: i8 },
}

impl From<LogicalType> for TypeIdentity {
    fn from(value: LogicalType) -> Self {
        match value {
            LogicalType::Utf8 => Self::Utf8,
            LogicalType::Int64 => Self::Int64,
            LogicalType::Decimal128 { precision, scale } => Self::Decimal128 { precision, scale },
        }
    }
}

impl From<PhysicalEncoding> for EncodingIdentity {
    fn from(value: PhysicalEncoding) -> Self {
        match value {
            PhysicalEncoding::EbcdicText => Self::EbcdicText,
            PhysicalEncoding::EbcdicDisplayNumeric => Self::EbcdicDisplayNumeric,
            PhysicalEncoding::BigEndianBinary => Self::BigEndianBinary,
            PhysicalEncoding::PackedDecimal => Self::PackedDecimal,
        }
    }
}

#[derive(Serialize)]
struct FieldIdentity<'a> {
    path: &'a Option<String>,
    source_name: &'a str,
    offset: usize,
    byte_length: usize,
    physical_encoding: EncodingIdentity,
    signed: bool,
    precision: Option<u8>,
    scale: Option<i8>,
    logical_type: TypeIdentity,
}

#[derive(Serialize)]
struct ArrowFieldIdentity<'a> {
    name: &'a str,
    data_type: TypeIdentity,
    nullable: bool,
    metadata: &'a std::collections::HashMap<String, String>,
}

#[derive(Serialize)]
struct LayoutIdentity<'a> {
    name: &'a str,
    record_length: usize,
    fields: Vec<FieldIdentity<'a>>,
    arrow_fields: Vec<ArrowFieldIdentity<'a>>,
    arrow_metadata: &'a std::collections::HashMap<String, String>,
}

pub(crate) fn layout_hash(layout: &CompiledCopybook) -> Result<String, RecoveryError> {
    let fields = layout
        .fields
        .iter()
        .map(|field| FieldIdentity {
            path: &field.path,
            source_name: &field.source_name,
            offset: field.offset,
            byte_length: field.byte_length,
            physical_encoding: field.physical_encoding.into(),
            signed: field.signed,
            precision: field.precision,
            scale: field.scale,
            logical_type: field.logical_type.into(),
        })
        .collect();
    let arrow_fields = layout
        .arrow_schema
        .fields()
        .iter()
        .map(|field| {
            let data_type = match field.data_type() {
                DataType::Utf8 => TypeIdentity::Utf8,
                DataType::Int64 => TypeIdentity::Int64,
                DataType::Decimal128(precision, scale) => TypeIdentity::Decimal128 {
                    precision: *precision,
                    scale: *scale,
                },
                _ => return Err(invalid("Arrow type is outside the M4 profile")),
            };
            Ok(ArrowFieldIdentity {
                name: field.name(),
                data_type,
                nullable: field.is_nullable(),
                metadata: field.metadata(),
            })
        })
        .collect::<Result<_, RecoveryError>>()?;
    canonical_hash(&LayoutIdentity {
        name: &layout.name,
        record_length: layout.record_length,
        fields,
        arrow_fields,
        arrow_metadata: layout.arrow_schema.metadata(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};

    use arrow_schema::{Field, Schema};

    const FIXTURE: &[u8] = include_bytes!("../tests/fixtures/sample_fixed.bin");
    const FIXTURE_SHA: &str = "bc5083614c9c50322a78ea30b909fabb28d63a22f0d4bda87f77dfd49e47fb73";

    fn sample_manifest(bytes: u64, batch: u64) -> Manifest {
        Manifest::new(bytes, "a".repeat(64), "b".repeat(64), 35, batch).unwrap()
    }

    fn layout() -> CompiledCopybook {
        crate::parse_and_compile_copybook(include_str!("../tests/fixtures/sample_fixed.cpy"))
            .unwrap()
    }

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(contents: &[u8]) -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "m2c-manifest-{}-{}.json",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut file = File::create_new(&path).unwrap();
            file.write_all(contents).unwrap();
            Self(path)
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    #[test]
    fn sha256_matches_independent_vectors_and_fixture() {
        for (bytes, expected) in [
            (
                b"".as_slice(),
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                b"abc".as_slice(),
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (FIXTURE, FIXTURE_SHA),
        ] {
            let (count, hash) = hash_reader(&mut Cursor::new(bytes)).unwrap();
            assert_eq!(count, bytes.len() as u64);
            assert_eq!(hash, expected);
        }
    }

    #[test]
    fn hash_stream_handles_short_and_interrupted_reads() {
        struct Fragmented<'a> {
            bytes: &'a [u8],
            interrupted: bool,
        }
        impl Read for Fragmented<'_> {
            fn read(&mut self, target: &mut [u8]) -> io::Result<usize> {
                if !self.interrupted {
                    self.interrupted = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                self.interrupted = false;
                let count = self.bytes.len().min(3).min(target.len());
                target[..count].copy_from_slice(&self.bytes[..count]);
                self.bytes = &self.bytes[count..];
                Ok(count)
            }
        }
        let mut reader = Fragmented {
            bytes: FIXTURE,
            interrupted: false,
        };
        assert_eq!(hash_reader(&mut reader).unwrap(), (105, FIXTURE_SHA.into()));
    }

    #[test]
    fn deterministic_ranges_cover_empty_full_and_partial_batches() {
        let manifest = sample_manifest(105, 2);
        assert_eq!(manifest.total_records(), 3);
        assert_eq!(manifest.part_count(), 2);
        assert_eq!(manifest.range(0).unwrap(), (0, 2));
        assert_eq!(manifest.range(1).unwrap(), (2, 1));
        assert!(manifest.range(2).is_err());
        let manifest = sample_manifest(105, 1);
        assert_eq!(manifest.part_count(), 3);
        assert_eq!(manifest.range(2).unwrap(), (2, 1));
        let manifest = sample_manifest(0, 2);
        assert_eq!(manifest.part_count(), 1);
        assert_eq!(manifest.range(0).unwrap(), (0, 0));
        assert!(manifest.range(1).is_err());
    }

    #[test]
    fn dimensions_and_arithmetic_are_checked_without_panics() {
        let make = |bytes, record, batch| {
            Manifest::new(bytes, "a".repeat(64), "b".repeat(64), record, batch)
        };
        assert!(make(0, 0, 1).is_err());
        assert!(make(0, 1, 0).is_err());
        assert!(make(106, 35, 2).is_err());
        assert!(make(0, u64::MAX, 2).is_err());
        let largest = make(u64::MAX, 1, 1).unwrap();
        assert_eq!(largest.part_count(), u64::MAX);
        assert_eq!(largest.range(u64::MAX - 1).unwrap(), (u64::MAX - 1, 1));
        assert!(largest.range(u64::MAX).is_err());
        let largest = make(u64::MAX, 1, 2).unwrap();
        assert_eq!(largest.part_count(), (u64::MAX / 2) + 1);
        assert_eq!(largest.range(u64::MAX / 2).unwrap(), (u64::MAX - 1, 1));
        let mut invalid = sample_manifest(105, 2);
        invalid.record_length = 0;
        invalid.batch_records = 0;
        assert_eq!(invalid.total_records(), 0);
        assert_eq!(invalid.part_count(), 1);
        assert!(invalid.range(0).is_err());
    }

    #[test]
    fn canonical_hash_sorts_nested_maps_and_preserves_array_order() {
        let left: HashMap<_, _> = [
            ("z", HashMap::from([("b", 2), ("a", 1)])),
            ("a", HashMap::from([("d", 4), ("c", 3)])),
        ]
        .into();
        let right: HashMap<_, _> = [
            ("a", HashMap::from([("c", 3), ("d", 4)])),
            ("z", HashMap::from([("a", 1), ("b", 2)])),
        ]
        .into();
        assert_eq!(
            canonical_hash(&left).unwrap(),
            canonical_hash(&right).unwrap()
        );
        // Independent .NET SHA256 of {"a":{"c":3,"d":4},"z":{"a":1,"b":2}}.
        assert_eq!(
            canonical_hash(&left).unwrap(),
            "43ca7c87ea41ebd2526378ee34758d1564792318a931b02b2e9d8f9dd705bfe6"
        );
        assert_ne!(
            canonical_hash(&[1, 2]).unwrap(),
            canonical_hash(&[2, 1]).unwrap()
        );
    }

    #[test]
    fn fingerprint_includes_physical_layout_and_arrow_metadata_but_not_spans() {
        let original = layout();
        let hash = layout_hash(&original).unwrap();
        let mut changed = original.clone();
        for field in &mut changed.fields {
            field.span = crate::SourceSpan::new(999, 12);
        }
        assert_eq!(hash, layout_hash(&changed).unwrap());
        changed.fields[1].byte_length += 1; // FILLER remains part of the identity.
        assert_ne!(hash, layout_hash(&changed).unwrap());

        let mut changed = original.clone();
        changed.fields[2].physical_encoding = PhysicalEncoding::BigEndianBinary;
        assert_ne!(hash, layout_hash(&changed).unwrap());

        let mut changed = original.clone();
        changed.arrow_schema = original
            .arrow_schema
            .clone()
            .with_metadata(HashMap::from([("source".into(), "fixture".into())]));
        assert_ne!(hash, layout_hash(&changed).unwrap());

        let mut fields = original.arrow_schema.fields().to_vec();
        fields[0] = std::sync::Arc::new(
            fields[0]
                .as_ref()
                .clone()
                .with_metadata(HashMap::from([("unit".into(), "text".into())])),
        );
        changed.arrow_schema = Schema::new(fields);
        assert_ne!(hash, layout_hash(&changed).unwrap());
    }

    #[test]
    fn metadata_insertion_order_does_not_change_layout_identity() {
        let mut first = layout();
        let mut second = first.clone();
        first.arrow_schema = first.arrow_schema.with_metadata(HashMap::from([
            ("z".into(), "last".into()),
            ("a".into(), "first".into()),
        ]));
        second.arrow_schema = second.arrow_schema.with_metadata(HashMap::from([
            ("a".into(), "first".into()),
            ("z".into(), "last".into()),
        ]));
        assert_eq!(layout_hash(&first).unwrap(), layout_hash(&second).unwrap());
        second.arrow_schema = Schema::new(vec![Field::new("invalid", DataType::Boolean, false)]);
        assert!(layout_hash(&second).is_err());
    }

    #[test]
    fn descriptor_detects_tampering_and_unsupported_versions() {
        let original = sample_manifest(105, 2);
        original.validate().unwrap();
        let mut changed = original.clone();
        changed.input_sha256 = "c".repeat(64);
        assert!(changed.validate().is_err());
        changed = original.clone();
        changed.version = 2;
        assert!(matches!(
            changed.validate(),
            Err(RecoveryError::UnsupportedVersion { version: 2 })
        ));
        changed = original.clone();
        changed.profile.push_str("-unknown");
        assert!(matches!(
            changed.validate(),
            Err(RecoveryError::UnsupportedProfile { .. })
        ));
        for value in [
            "a".repeat(63),
            "a".repeat(65),
            "A".repeat(64),
            "g".repeat(64),
        ] {
            changed = original.clone();
            changed.job_id = value;
            assert!(changed.validate().is_err());
        }
        assert!(valid_hash(&"0123456789abcdef".repeat(4)));
    }

    #[test]
    fn receipts_and_completion_must_match_deterministic_conversion() {
        let manifest = sample_manifest(105, 2);
        let receipt = Receipt {
            version: VERSION,
            job_id: manifest.job_id.clone(),
            part_index: 1,
            start_record: 2,
            record_count: 1,
            parquet_bytes: 500,
            parquet_sha256: "c".repeat(64),
        };
        receipt.validate(&manifest, 1).unwrap();
        assert!(receipt.validate(&manifest, 0).is_err());
        let mut changed = receipt.clone();
        changed.record_count = 2;
        assert!(changed.validate(&manifest, 1).is_err());
        changed = receipt.clone();
        changed.parquet_bytes = 0;
        assert!(changed.validate(&manifest, 1).is_err());
        let completion = Completion {
            version: VERSION,
            job_id: manifest.job_id.clone(),
            part_count: 2,
            total_records: 3,
        };
        completion.validate(&manifest).unwrap();
        let mut changed = completion;
        changed.total_records = 2;
        assert!(changed.validate(&manifest).is_err());
    }

    #[test]
    fn typed_json_rejects_unknown_duplicate_missing_invalid_and_trailing_values() {
        let manifest = sample_manifest(105, 2);
        let original = String::from_utf8(json_bytes(&manifest).unwrap()).unwrap();
        let cases = [
            original.replacen('{', "{\"unknown\":1,", 1),
            original.replacen('{', "{\"version\":1,", 1),
            original.replace("\"version\":1,", ""),
            original.replace("\"version\":1", "\"version\":-1"),
            original.replace("\"input_bytes\":105", "\"input_bytes\":1.5"),
            original.replace(
                "\"input_bytes\":105",
                "\"input_bytes\":18446744073709551616",
            ),
            format!("{original} null"),
        ];
        for contents in cases {
            let file = TempFile::new(contents.as_bytes());
            assert!(
                matches!(
                    read_json::<Manifest>(&file.0),
                    Err(RecoveryError::Metadata { .. })
                ),
                "{contents}"
            );
        }
    }

    #[test]
    fn json_read_and_write_enforce_limit_before_parsing() {
        let manifest = sample_manifest(105, 2);
        let mut bytes = json_bytes(&manifest).unwrap();
        bytes.resize(MAX_JSON_BYTES, b' ');
        let exact = TempFile::new(&bytes);
        assert_eq!(read_json::<Manifest>(&exact.0).unwrap(), manifest);
        bytes.push(b' ');
        let oversized = TempFile::new(&bytes);
        assert!(matches!(
            read_json::<Manifest>(&oversized.0),
            Err(RecoveryError::InvalidState { .. })
        ));
        assert!(json_bytes(&"x".repeat(MAX_JSON_BYTES)).is_err());
        assert_eq!(
            json_bytes(&"x".repeat(MAX_JSON_BYTES - 2)).unwrap().len(),
            MAX_JSON_BYTES
        );
    }
}
