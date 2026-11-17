use tedious_rs::bulk_load::BulkLoadOptions;
use tedious_rs::payloads::{BulkLoadColumn, BulkLoadPayload};

fn column(nullable: bool) -> BulkLoadColumn {
    BulkLoadColumn {
        name: "id".to_owned(),
        table_name: "items".to_owned(),
        type_info: vec![0x38, 4],
        nullable,
        legacy_lob: false,
    }
}

#[test]
fn bulk_order_keeps_declared_insertion_order() {
    let bulk = tedious_rs::BulkLoad::new(
        "dbo.target",
        BulkLoadOptions {
            order: vec![
                ("second".to_owned(), "DESC".to_owned()),
                ("first".to_owned(), "ASC".to_owned()),
            ],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        bulk.get_options_sql(),
        " WITH (ORDER (second DESC, first ASC))"
    );
}

#[test]
fn bulk_metadata_sets_nullable_unknown_only_for_tds_72_and_newer() {
    let modern = BulkLoadPayload {
        columns: vec![column(false)],
        rows: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    let legacy = BulkLoadPayload {
        columns: vec![column(false)],
        rows: Vec::new(),
        tds_version: "7_1".to_owned(),
    };
    assert_eq!(
        &modern.column_metadata_with_nullable_unknown(&[0]).unwrap()[7..9],
        &[0x04, 0x80]
    );
    assert_eq!(
        &legacy.column_metadata_with_nullable_unknown(&[0]).unwrap()[5..7],
        &[0x04, 0x00]
    );
}

#[test]
fn bulk_metadata_keeps_explicit_nullable_flags() {
    let nullable = BulkLoadPayload {
        columns: vec![column(true)],
        rows: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    let non_nullable = BulkLoadPayload {
        columns: vec![column(false)],
        rows: Vec::new(),
        tds_version: "7_4".to_owned(),
    };
    assert_eq!(&nullable.column_metadata().unwrap()[7..9], &[0x05, 0x00]);
    assert_eq!(
        &non_nullable.column_metadata().unwrap()[7..9],
        &[0x04, 0x00]
    );
}
