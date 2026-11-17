use serde::{Deserialize, Serialize};

pub const CATALOG_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DialectId {
    Postgresql,
    Mysql,
    Mariadb,
    Sqlserver,
    Oracle,
    Sqlite,
    Generic,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCapabilities {
    pub databases: bool,
    pub schemas: bool,
    pub tables: bool,
    pub views: bool,
    pub materialized_views: bool,
    pub procedures: bool,
    pub functions: bool,
    pub triggers: bool,
    pub sequences: bool,
    pub extensions: bool,
    pub partitions: bool,
    pub users: bool,
    pub roles: bool,
    pub permissions: bool,
    pub synonyms: bool,
    pub indexes: bool,
    pub foreign_keys: bool,
    pub custom_types: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SqlTypeFamily {
    Unknown,
    Null,
    Boolean,
    Integer,
    Decimal,
    Float,
    Text,
    Binary,
    Date,
    Time,
    Timestamp,
    Interval,
    Uuid,
    Json,
    Xml,
    Array,
    Enum,
    Spatial,
    Custom,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SqlTypeRef {
    pub family: SqlTypeFamily,
    pub name: String,
    pub nullable: Option<bool>,
    pub precision: Option<u32>,
    pub scale: Option<u32>,
    pub element_type: Option<Box<SqlTypeRef>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum IdentifierQuoteStyle {
    None,
    Double,
    Backtick,
    Bracket,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Identifier {
    pub raw: String,
    pub normalized: String,
    pub quoted: bool,
    pub quote_style: IdentifierQuoteStyle,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSchema {
    pub id: String,
    pub catalog_id: Option<String>,
    pub database_id: String,
    pub name: Identifier,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDatabase {
    pub id: String,
    pub name: Identifier,
    pub system: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ColumnMetadata {
    pub id: String,
    pub table_id: String,
    pub ordinal: u32,
    pub name: Identifier,
    pub r#type: SqlTypeRef,
    pub nullable: bool,
    pub generated: bool,
    pub identity: bool,
    pub computed: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
    pub foreign_key_memberships: Vec<String>,
    pub unique_memberships: Vec<String>,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryKeyMetadata {
    pub id: String,
    pub name: Identifier,
    pub table_id: String,
    pub column_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UniqueConstraintMetadata {
    pub id: String,
    pub name: Identifier,
    pub table_id: String,
    pub column_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CheckConstraintMetadata {
    pub id: String,
    pub name: Identifier,
    pub table_id: String,
    pub expression: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IndexMetadata {
    pub id: String,
    pub name: Identifier,
    pub table_id: String,
    pub column_ids: Vec<String>,
    pub unique: bool,
    pub primary: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ForeignKeyMetadata {
    pub id: String,
    pub name: Identifier,
    pub source_table_id: String,
    pub source_column_ids: Vec<String>,
    pub target_table_id: String,
    pub target_column_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTable {
    pub id: String,
    pub catalog_id: Option<String>,
    pub database_id: String,
    pub schema_id: String,
    pub name: Identifier,
    pub columns: Vec<ColumnMetadata>,
    pub primary_key: Option<PrimaryKeyMetadata>,
    pub foreign_keys: Vec<ForeignKeyMetadata>,
    pub unique_constraints: Vec<UniqueConstraintMetadata>,
    pub check_constraints: Vec<CheckConstraintMetadata>,
    pub indexes: Vec<IndexMetadata>,
    pub estimated_rows: Option<u64>,
    pub comment: Option<String>,
    pub temporary: bool,
    #[serde(default)]
    pub foreign: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogView {
    pub id: String,
    pub catalog_id: Option<String>,
    pub database_id: String,
    pub schema_id: String,
    pub name: Identifier,
    pub columns: Vec<ColumnMetadata>,
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogParameter {
    pub id: String,
    pub name: Identifier,
    pub r#type: SqlTypeRef,
    pub direction: String,
    pub ordinal: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogFunction {
    pub id: String,
    pub schema_id: Option<String>,
    pub name: Identifier,
    pub parameters: Vec<CatalogParameter>,
    pub return_type: SqlTypeRef,
    pub variadic: bool,
    pub overloads: Vec<String>,
    pub kind: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProcedure {
    pub id: String,
    pub schema_id: Option<String>,
    pub name: Identifier,
    pub parameters: Vec<CatalogParameter>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSequence {
    pub id: String,
    pub schema_id: String,
    pub name: Identifier,
    pub r#type: SqlTypeRef,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogType {
    pub id: String,
    pub schema_id: Option<String>,
    pub name: Identifier,
    pub family: SqlTypeFamily,
    pub definition: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTrigger {
    pub id: String,
    pub schema_id: Option<String>,
    pub table_id: Option<String>,
    pub name: Identifier,
    pub enabled: bool,
    pub definition: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogExtension {
    pub id: String,
    pub name: Identifier,
    pub version: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPartition {
    pub id: String,
    pub table_id: String,
    pub name: Option<Identifier>,
    pub parent_id: Option<String>,
    pub bound: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPrincipal {
    pub id: String,
    pub name: Identifier,
    pub kind: String,
    pub login: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPermission {
    pub id: String,
    pub principal_id: String,
    pub object_id: Option<String>,
    pub permission: String,
    pub state: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSynonym {
    pub id: String,
    pub schema_id: String,
    pub name: Identifier,
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum CatalogConstraint {
    PrimaryKey(PrimaryKeyMetadata),
    Unique(UniqueConstraintMetadata),
    Check(CheckConstraintMetadata),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshot {
    pub schema_version: u32,
    pub id: String,
    pub connection_id: String,
    pub database_id: String,
    pub dialect: DialectId,
    pub version: String,
    pub generated_at: String,
    pub databases: Vec<CatalogDatabase>,
    pub schemas: Vec<CatalogSchema>,
    pub tables: Vec<CatalogTable>,
    pub views: Vec<CatalogView>,
    pub materialized_views: Vec<CatalogView>,
    pub functions: Vec<CatalogFunction>,
    pub procedures: Vec<CatalogProcedure>,
    pub sequences: Vec<CatalogSequence>,
    pub types: Vec<CatalogType>,
    pub triggers: Vec<CatalogTrigger>,
    pub extensions: Vec<CatalogExtension>,
    pub partitions: Vec<CatalogPartition>,
    pub principals: Vec<CatalogPrincipal>,
    pub permissions: Vec<CatalogPermission>,
    pub synonyms: Vec<CatalogSynonym>,
    pub foreign_keys: Vec<ForeignKeyMetadata>,
    pub indexes: Vec<IndexMetadata>,
    pub constraints: Vec<CatalogConstraint>,
}
