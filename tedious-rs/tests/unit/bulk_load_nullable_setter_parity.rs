use tedious_rs::bulk_load::{BulkLoad, BulkLoadColumnDefinition, BulkLoadOptions};

fn column(name: &str, nullable: bool) -> BulkLoadColumnDefinition {
    BulkLoadColumnDefinition {
        name: name.to_owned(),
        obj_name: name.to_owned(),
        declaration: "int".to_owned(),
        type_info: vec![0x26, 4],
        nullable,
        legacy_lob: false,
    }
}

#[test]
fn setting_nullable_unknown_omits_table_creation_nullability() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_column(column("id", false)).unwrap();
    bulk.set_nullable_unknown_columns([0]).unwrap();

    assert_eq!(
        bulk.get_table_creation_sql(),
        "CREATE TABLE dbo.target(\n[id] int\n)"
    );
}

#[test]
fn clearing_nullable_unknown_restores_the_explicit_column_nullability() {
    let mut bulk = BulkLoad::new("dbo.target", BulkLoadOptions::default()).unwrap();
    bulk.add_column(column("id", false)).unwrap();
    bulk.set_nullable_unknown_columns([0]).unwrap();
    bulk.set_nullable_unknown_columns([]).unwrap();

    assert_eq!(
        bulk.get_table_creation_sql(),
        "CREATE TABLE dbo.target(\n[id] int NOT NULL\n)"
    );
}
