use crate::{m6_campaign as campaign, *};
use std::fs;
#[test]
fn m6_combined_resume_mutations() {
    let base = campaign::directory("base-m4");
    let input = base.join("input");
    fs::write(&input, include_bytes!("../tests/fixtures/sample_fixed.bin")).unwrap();
    let layout =
        parse_and_compile_copybook(include_str!("../tests/fixtures/sample_fixed.cpy")).unwrap();
    convert_parts(&layout, &input, &base.join("job"), 1, RecoveryMode::Create).unwrap();
    let files = campaign::snapshot(&base);
    campaign::cleanup(&base);
    campaign::run(
        "m4",
        |bytes| {
            let mut files = files.clone();
            // At least one guaranteed invalid mutation, followed by additional independent mutations.
            for b in bytes.iter().take(8) {
                match b % 6 {
                    0 => {
                        files.insert("job/unknown".into(), vec![*b]);
                    }
                    1 => {
                        files.insert(
                            "job/manifest.json".into(),
                            b"{\"version\":1,\"version\":2}".to_vec(),
                        );
                    }
                    2 => {
                        files.remove("job/commits/part-00000000000000000001.json");
                    }
                    3 => {
                        files
                            .get_mut("job/parts/part-00000000000000000000.parquet")
                            .unwrap()[0] ^= 1;
                        files.insert("job/complete.json".into(), b"{}".to_vec());
                    }
                    4 => {
                        files.insert(
                            "job/commits/part-00000000000000000000.json".into(),
                            vec![b' '; 4097],
                        );
                    }
                    _ => {
                        files.insert("job/complete.json".into(), b"{} trailing".to_vec());
                    }
                }
            }
            files.insert(
                "job/.complete.json.tmp".into(),
                b"must survive rejection".to_vec(),
            );
            campaign::Case {
                kind: "m4".into(),
                files,
            }
        },
        |_, root| {
            let before = campaign::snapshot(root);
            let result = convert_parts(
                &layout,
                &root.join("input"),
                &root.join("job"),
                1,
                RecoveryMode::Resume,
            );
            assert!(result.is_err());
            assert_eq!(before, campaign::snapshot(root));
        },
    );
}
