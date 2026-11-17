use tedious_rs::bulk_load::{BulkLoad, BulkLoadColumnDefinition, BulkLoadOptions};
use tedious_rs::data_type::type_by_name;

#[test]
fn undefined_nullable_omits_nullability_from_table_creation_sql() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_typed_column_with_nullable(
        "known_nullable",
        type_by_name("Int").unwrap(),
        Default::default(),
        Some(true),
    )
    .unwrap();
    bulk.add_typed_column_with_nullable(
        "unknown_nullable",
        type_by_name("Int").unwrap(),
        Default::default(),
        None,
    )
    .unwrap();

    assert_eq!(
        bulk.get_table_creation_sql(),
        "CREATE TABLE dbo.target(\n[known_nullable] int NULL,\n[unknown_nullable] int\n)"
    );
}

#[test]
fn undefined_nullable_sets_nullable_unknown_metadata_flag() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_column_with_nullable(
        BulkLoadColumnDefinition {
            name: "unknown_nullable".to_owned(),
            obj_name: "unknown_nullable".to_owned(),
            declaration: "int".to_owned(),
            type_info: vec![0x26, 4],
            nullable: false,
            legacy_lob: false,
        },
        None,
    )
    .unwrap();

    assert_eq!(&bulk.get_col_metadata().unwrap()[7..9], &[0x04, 0x80]);
}
