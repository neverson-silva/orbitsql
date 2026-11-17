use crate::backend::adapters::transport::{TransportGuard, prepare_endpoint_for};
use crate::backend::adapters::type_mapping::{identifier, type_ref};
use crate::backend::domain::catalog::{
    CATALOG_SCHEMA_VERSION, CatalogConstraint, CatalogDatabase, CatalogFunction, CatalogPartition,
    CatalogPermission, CatalogPrincipal, CatalogProcedure, CatalogSchema, CatalogSequence,
    CatalogSnapshot, CatalogSynonym, CatalogTable, CatalogTrigger, CatalogType, CatalogView,
    CheckConstraintMetadata, ColumnMetadata, DialectId, ForeignKeyMetadata, IndexMetadata,
    PrimaryKeyMetadata, SqlTypeRef, UniqueConstraintMetadata,
};
use crate::backend::domain::connection::ConnectionAuthMethod;
use crate::backend::ports::database::{CatalogRefreshRequest, ProviderResult};
use crate::backend::ports::{
    CatalogProvider, CatalogProviderError, CatalogProviderFuture, CatalogScope,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tedious_rs::Connection;
use tedious_rs::token::parser::Token;
use tedious_rs::token::stream::StreamToken;
use tedious_rs::value_codec::Value;

#[derive(Clone)]
struct SqlServerObject {
    object_id: i64,
    schema_id: String,
    name: String,
    kind: String,
}

#[derive(Clone)]
struct KeyMember {
    object_id: i64,
    name: String,
    parent_object_id: i64,
    target_object_id: Option<i64>,
    constraint_type: String,
    parent_column_id: i64,
    target_column_id: Option<i64>,
}

#[derive(Clone)]
struct IndexMember {
    object_id: i64,
    index_id: i64,
    name: String,
    unique: bool,
    primary: bool,
    ordinal: i64,
    column_id: i64,
}

fn schema_key(name: &str) -> String {
    format!("schema:{name}")
}

fn object_key(schema: &str, name: &str) -> String {
    format!("object:{schema}:{name}")
}

fn column_key(object: &str, ordinal: i64) -> String {
    format!("column:{object}:{ordinal}")
}

fn value_text(value: &Value) -> Result<String, String> {
    match value {
        Value::Text(value) | Value::Guid(value) | Value::BigInteger(value) => Ok(value.clone()),
        Value::Integer(value) => Ok(value.to_string()),
        Value::Boolean(value) => Ok(value.to_string()),
        Value::Real(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(value.to_string()),
        Value::Decimal { unscaled, scale } => {
            if *scale == 0 {
                return Ok(unscaled.to_string());
            }
            let negative = *unscaled < 0;
            let digits = unscaled.unsigned_abs().to_string();
            let scale = usize::from(*scale);
            let formatted = if digits.len() <= scale {
                format!("0.{}{}", "0".repeat(scale - digits.len()), digits)
            } else {
                let split = digits.len() - scale;
                format!("{}.{}", &digits[..split], &digits[split..])
            };
            Ok(if negative {
                format!("-{formatted}")
            } else {
                formatted
            })
        }
        Value::Null => Ok(String::new()),
        other => Err(format!("unsupported SQL Server catalog value: {other:?}")),
    }
}

fn value_i64(value: &Value) -> Result<i64, String> {
    match value {
        Value::Integer(value) => Ok(*value),
        Value::BigInteger(value) => value
            .parse::<i64>()
            .map_err(|error| format!("invalid integer catalog value: {error}")),
        Value::Text(value) => value
            .parse::<i64>()
            .map_err(|error| format!("invalid integer catalog value: {error}")),
        other => Err(format!("unsupported integer catalog value: {other:?}")),
    }
}

fn value_bool(value: &Value) -> Result<bool, String> {
    match value {
        Value::Boolean(value) => Ok(*value),
        Value::Integer(value) => Ok(*value != 0),
        Value::Text(value) => Ok(matches!(value.as_str(), "1" | "true" | "TRUE")),
        other => Err(format!("unsupported boolean catalog value: {other:?}")),
    }
}

fn cell<'a>(row: &'a [Value], index: usize, field: &str) -> Result<&'a Value, String> {
    row.get(index)
        .ok_or_else(|| format!("SQL Server catalog row is missing {field}"))
}

fn text(row: &[Value], index: usize, field: &str) -> Result<String, String> {
    value_text(cell(row, index, field)?)
}

fn integer(row: &[Value], index: usize, field: &str) -> Result<i64, String> {
    value_i64(cell(row, index, field)?)
}

fn boolean(row: &[Value], index: usize, field: &str) -> Result<bool, String> {
    value_bool(cell(row, index, field)?)
}

fn optional_text(row: &[Value], index: usize, field: &str) -> Result<Option<String>, String> {
    match cell(row, index, field)? {
        Value::Null => Ok(None),
        value => Ok(Some(value_text(value)?)),
    }
}

async fn query(connection: &mut Connection, sql: &str) -> Result<Vec<Vec<Value>>, String> {
    let tokens = connection
        .exec_sql_batch(sql)
        .await
        .map_err(|error| error.to_string())?;
    let mut rows = Vec::new();
    let mut errors = Vec::new();
    let mut sql_error = false;
    for token in tokens {
        match token {
            StreamToken::Row(row) | StreamToken::NbcRow(row) => {
                rows.push(row.columns.into_iter().map(|column| column.value).collect());
            }
            StreamToken::Control(Token::Error(error)) => errors.push(format!(
                "SQL Server error {} (state {}, class {}): {}",
                error.number, error.state, error.class, error.message
            )),
            StreamToken::Control(Token::Done(done)) => sql_error |= done.sql_error,
            StreamToken::Control(_) | StreamToken::ColMetadata(_) => {}
        }
    }
    if errors.is_empty() && sql_error {
        errors.push("SQL Server reported an error while executing the catalog query".to_owned());
    }
    if !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(rows)
}

async fn fetch_schemas(
    connection: &mut Connection,
    database_id: &str,
) -> Result<Vec<CatalogSchema>, String> {
    let rows = query(
        connection,
        "SELECT name FROM sys.schemas WHERE name NOT IN ('guest', 'INFORMATION_SCHEMA') ORDER BY name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let name = text(&row, 0, "schema name")?;
            Ok(CatalogSchema {
                id: schema_key(&name),
                catalog_id: Some(database_id.to_owned()),
                database_id: database_id.to_owned(),
                name: identifier(name),
                comment: None,
            })
        })
        .collect()
}

async fn fetch_objects(
    connection: &mut Connection,
) -> Result<HashMap<i64, SqlServerObject>, String> {
    let rows = query(
        connection,
        "SELECT o.object_id, s.name, o.name, o.type FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id WHERE o.type IN ('U', 'V') AND o.is_ms_shipped = 0 ORDER BY s.name, o.name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let object_id = integer(&row, 0, "object id")?;
            Ok((
                object_id,
                SqlServerObject {
                    object_id,
                    schema_id: text(&row, 1, "schema name")?,
                    name: text(&row, 2, "object name")?,
                    kind: text(&row, 3, "object type")?,
                },
            ))
        })
        .collect()
}

async fn fetch_objects_for_schema(
    connection: &mut Connection,
    schema_name: &str,
) -> Result<HashMap<i64, SqlServerObject>, String> {
    let escaped_schema = schema_name.replace('\'', "''");
    let sql = format!(
        "SELECT o.object_id, s.name, o.name, o.type FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id WHERE o.type IN ('U', 'V') AND o.is_ms_shipped = 0 AND s.name = N'{escaped_schema}' ORDER BY o.name"
    );
    let rows = query(connection, &sql).await?;
    rows.into_iter()
        .map(|row| {
            let object_id = integer(&row, 0, "object id")?;
            Ok((
                object_id,
                SqlServerObject {
                    object_id,
                    schema_id: text(&row, 1, "schema name")?,
                    name: text(&row, 2, "object name")?,
                    kind: text(&row, 3, "object type")?,
                },
            ))
        })
        .collect()
}

async fn fetch_object_for_name(
    connection: &mut Connection,
    schema_name: &str,
    object_name: &str,
) -> Result<Option<SqlServerObject>, String> {
    let escaped_schema = schema_name.replace('\'', "''");
    let escaped_name = object_name.replace('\'', "''");
    let sql = format!(
        "SELECT o.object_id, s.name, o.name, o.type FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id WHERE o.type IN ('U', 'V') AND o.is_ms_shipped = 0 AND s.name = N'{escaped_schema}' AND o.name = N'{escaped_name}'"
    );
    let mut rows = query(connection, &sql).await?;
    rows.pop()
        .map(|row| {
            let object_id = integer(&row, 0, "object id")?;
            Ok(SqlServerObject {
                object_id,
                schema_id: text(&row, 1, "schema name")?,
                name: text(&row, 2, "object name")?,
                kind: text(&row, 3, "object type")?,
            })
        })
        .transpose()
}

async fn fetch_objects_for_ids(
    connection: &mut Connection,
    object_ids: &[i64],
) -> Result<HashMap<i64, SqlServerObject>, String> {
    let ids = object_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT o.object_id, s.name, o.name, o.type FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id WHERE o.type IN ('U', 'V') AND o.is_ms_shipped = 0 AND o.object_id IN ({ids}) ORDER BY s.name, o.name"
    );
    let rows = query(connection, &sql).await?;
    rows.into_iter()
        .map(|row| {
            let object_id = integer(&row, 0, "object id")?;
            Ok((
                object_id,
                SqlServerObject {
                    object_id,
                    schema_id: text(&row, 1, "schema name")?,
                    name: text(&row, 2, "object name")?,
                    kind: text(&row, 3, "object type")?,
                },
            ))
        })
        .collect()
}

async fn fetch_columns(
    connection: &mut Connection,
    objects: &HashMap<i64, SqlServerObject>,
) -> Result<HashMap<i64, Vec<ColumnMetadata>>, String> {
    let rows = query(
        connection,
        "SELECT c.object_id, c.column_id, c.name, TYPE_NAME(c.user_type_id), c.is_nullable, c.is_identity, c.is_computed, dc.definition FROM sys.columns c LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id WHERE c.object_id IN (SELECT object_id FROM sys.objects WHERE type IN ('U', 'V') AND is_ms_shipped = 0) ORDER BY c.object_id, c.column_id",
    )
    .await?;
    let mut columns = HashMap::new();
    for row in rows {
        let object_id = integer(&row, 0, "column object id")?;
        let Some(object) = objects.get(&object_id) else {
            continue;
        };
        let ordinal = integer(&row, 1, "column id")?;
        let type_name = text(&row, 3, "column type")?;
        let mut sql_type = type_ref(&type_name, 0, 0);
        sql_type.nullable = Some(boolean(&row, 4, "column nullability")?);
        let object_key = object_key(&object.schema_id, &object.name);
        columns
            .entry(object_id)
            .or_insert_with(Vec::new)
            .push(ColumnMetadata {
                id: column_key(&object_key, ordinal),
                table_id: object_key,
                ordinal: u32::try_from(ordinal).map_err(|error| error.to_string())?,
                name: identifier(text(&row, 2, "column name")?),
                r#type: sql_type,
                nullable: boolean(&row, 4, "column nullability")?,
                generated: boolean(&row, 6, "computed column")?,
                identity: boolean(&row, 5, "identity column")?,
                computed: boolean(&row, 6, "computed column")?,
                default_value: optional_text(&row, 7, "default definition")?,
                primary_key: false,
                foreign_key_memberships: Vec::new(),
                unique_memberships: Vec::new(),
                comment: None,
            });
    }
    Ok(columns)
}

async fn fetch_columns_for_ids(
    connection: &mut Connection,
    objects: &HashMap<i64, SqlServerObject>,
    object_ids: &[i64],
) -> Result<HashMap<i64, Vec<ColumnMetadata>>, String> {
    let ids = object_ids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT c.object_id, c.column_id, c.name, TYPE_NAME(c.user_type_id), c.is_nullable, c.is_identity, c.is_computed, dc.definition FROM sys.columns c LEFT JOIN sys.default_constraints dc ON dc.parent_object_id = c.object_id AND dc.parent_column_id = c.column_id WHERE c.object_id IN ({ids}) ORDER BY c.object_id, c.column_id"
    );
    let rows = query(connection, &sql).await?;
    let mut columns = HashMap::new();
    for row in rows {
        let object_id = integer(&row, 0, "column object id")?;
        let Some(object) = objects.get(&object_id) else {
            continue;
        };
        let ordinal = integer(&row, 1, "column id")?;
        let type_name = text(&row, 3, "column type")?;
        let mut sql_type = type_ref(&type_name, 0, 0);
        sql_type.nullable = Some(boolean(&row, 4, "column nullability")?);
        let object_key = object_key(&object.schema_id, &object.name);
        columns
            .entry(object_id)
            .or_insert_with(Vec::new)
            .push(ColumnMetadata {
                id: column_key(&object_key, ordinal),
                table_id: object_key,
                ordinal: u32::try_from(ordinal).map_err(|error| error.to_string())?,
                name: identifier(text(&row, 2, "column name")?),
                r#type: sql_type,
                nullable: boolean(&row, 4, "column nullability")?,
                generated: boolean(&row, 6, "computed column")?,
                identity: boolean(&row, 5, "identity column")?,
                computed: boolean(&row, 6, "computed column")?,
                default_value: optional_text(&row, 7, "default definition")?,
                primary_key: false,
                foreign_key_memberships: Vec::new(),
                unique_memberships: Vec::new(),
                comment: None,
            });
    }
    Ok(columns)
}

async fn fetch_indexes(connection: &mut Connection) -> Result<Vec<IndexMember>, String> {
    let rows = query(
        connection,
        "SELECT i.object_id, i.index_id, i.name, i.is_unique, i.is_primary_key, ic.key_ordinal, ic.column_id FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id WHERE i.object_id IN (SELECT object_id FROM sys.objects WHERE type = 'U' AND is_ms_shipped = 0) AND i.index_id > 0 AND ic.key_ordinal > 0 ORDER BY i.object_id, i.index_id, ic.key_ordinal",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(IndexMember {
                object_id: integer(&row, 0, "index object id")?,
                index_id: integer(&row, 1, "index id")?,
                name: text(&row, 2, "index name")?,
                unique: boolean(&row, 3, "index uniqueness")?,
                primary: boolean(&row, 4, "primary index")?,
                ordinal: integer(&row, 5, "index ordinal")?,
                column_id: integer(&row, 6, "index column id")?,
            })
        })
        .collect()
}

async fn fetch_indexes_for_object(
    connection: &mut Connection,
    object_id: i64,
) -> Result<Vec<IndexMember>, String> {
    let sql = format!(
        "SELECT i.object_id, i.index_id, i.name, i.is_unique, i.is_primary_key, ic.key_ordinal, ic.column_id FROM sys.indexes i JOIN sys.index_columns ic ON ic.object_id = i.object_id AND ic.index_id = i.index_id WHERE i.object_id = {object_id} AND i.index_id > 0 AND ic.key_ordinal > 0 ORDER BY i.index_id, ic.key_ordinal"
    );
    let rows = query(connection, &sql).await?;
    rows.into_iter()
        .map(|row| {
            Ok(IndexMember {
                object_id: integer(&row, 0, "index object id")?,
                index_id: integer(&row, 1, "index id")?,
                name: text(&row, 2, "index name")?,
                unique: boolean(&row, 3, "index uniqueness")?,
                primary: boolean(&row, 4, "primary index")?,
                ordinal: integer(&row, 5, "index ordinal")?,
                column_id: integer(&row, 6, "index column id")?,
            })
        })
        .collect()
}

async fn fetch_key_members(connection: &mut Connection) -> Result<Vec<KeyMember>, String> {
    let rows = query(
        connection,
        "SELECT kc.object_id, kc.name, kc.parent_object_id, kc.type, ic.key_ordinal, ic.parent_column_id, NULL, NULL FROM sys.key_constraints kc JOIN sys.index_columns ic ON ic.object_id = kc.parent_object_id AND ic.index_id = kc.unique_index_id WHERE ic.key_ordinal > 0 UNION ALL SELECT fk.object_id, fk.name, fk.parent_object_id, 'F', fkc.constraint_column_id, fkc.parent_column_id, fk.referenced_object_id, fkc.referenced_column_id FROM sys.foreign_keys fk JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id UNION ALL SELECT cc.object_id, cc.name, cc.parent_object_id, 'C', 0, 0, NULL, NULL FROM sys.check_constraints cc WHERE cc.is_ms_shipped = 0 ORDER BY 3, 2, 5",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let target_object_id = match cell(&row, 6, "target object id")? {
                Value::Null => None,
                value => Some(value_i64(value)?),
            };
            let target_column_id = match cell(&row, 7, "target column id")? {
                Value::Null => None,
                value => Some(value_i64(value)?),
            };
            Ok(KeyMember {
                object_id: integer(&row, 0, "constraint id")?,
                name: text(&row, 1, "constraint name")?,
                parent_object_id: integer(&row, 2, "constraint object id")?,
                constraint_type: text(&row, 3, "constraint type")?,
                parent_column_id: integer(&row, 5, "constraint column id")?,
                target_object_id,
                target_column_id,
            })
        })
        .collect()
}

async fn fetch_key_members_for_object(
    connection: &mut Connection,
    object_id: i64,
) -> Result<Vec<KeyMember>, String> {
    let sql = format!(
        "SELECT kc.object_id, kc.name, kc.parent_object_id, kc.type, ic.key_ordinal, ic.parent_column_id, NULL, NULL FROM sys.key_constraints kc JOIN sys.index_columns ic ON ic.object_id = kc.parent_object_id AND ic.index_id = kc.unique_index_id WHERE kc.parent_object_id = {object_id} AND ic.key_ordinal > 0 UNION ALL SELECT fk.object_id, fk.name, fk.parent_object_id, 'F', fkc.constraint_column_id, fkc.parent_column_id, fk.referenced_object_id, fkc.referenced_column_id FROM sys.foreign_keys fk JOIN sys.foreign_key_columns fkc ON fkc.constraint_object_id = fk.object_id WHERE fk.parent_object_id = {object_id} UNION ALL SELECT cc.object_id, cc.name, cc.parent_object_id, 'C', 0, 0, NULL, NULL FROM sys.check_constraints cc WHERE cc.is_ms_shipped = 0 AND cc.parent_object_id = {object_id} ORDER BY 3, 2, 5"
    );
    let rows = query(connection, &sql).await?;
    rows.into_iter()
        .map(|row| {
            let target_object_id = match cell(&row, 6, "target object id")? {
                Value::Null => None,
                value => Some(value_i64(value)?),
            };
            let target_column_id = match cell(&row, 7, "target column id")? {
                Value::Null => None,
                value => Some(value_i64(value)?),
            };
            Ok(KeyMember {
                object_id: integer(&row, 0, "constraint id")?,
                name: text(&row, 1, "constraint name")?,
                parent_object_id: integer(&row, 2, "constraint object id")?,
                constraint_type: text(&row, 3, "constraint type")?,
                parent_column_id: integer(&row, 5, "constraint column id")?,
                target_object_id,
                target_column_id,
            })
        })
        .collect()
}

async fn fetch_checks(connection: &mut Connection) -> Result<HashMap<i64, String>, String> {
    let rows = query(
        connection,
        "SELECT object_id, definition FROM sys.check_constraints WHERE is_ms_shipped = 0 ORDER BY object_id",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                integer(&row, 0, "check id")?,
                text(&row, 1, "check definition")?,
            ))
        })
        .collect()
}

async fn fetch_checks_for_object(
    connection: &mut Connection,
    object_id: i64,
) -> Result<HashMap<i64, String>, String> {
    let sql = format!(
        "SELECT object_id, definition FROM sys.check_constraints WHERE is_ms_shipped = 0 AND parent_object_id = {object_id} ORDER BY object_id"
    );
    let rows = query(connection, &sql).await?;
    rows.into_iter()
        .map(|row| {
            Ok((
                integer(&row, 0, "check id")?,
                text(&row, 1, "check definition")?,
            ))
        })
        .collect()
}

async fn fetch_routines(
    connection: &mut Connection,
) -> Result<(Vec<CatalogProcedure>, Vec<CatalogFunction>), String> {
    let rows = query(
        connection,
        "SELECT o.object_id, s.name, o.name, o.type FROM sys.objects o JOIN sys.schemas s ON s.schema_id = o.schema_id WHERE o.type IN ('P', 'FN', 'IF', 'TF') AND o.is_ms_shipped = 0 ORDER BY s.name, o.name",
    )
    .await?;
    let mut procedures = Vec::new();
    let mut functions = Vec::new();
    for row in rows {
        let object_id = integer(&row, 0, "routine id")?;
        let schema = text(&row, 1, "routine schema")?;
        let name = text(&row, 2, "routine name")?;
        let kind = text(&row, 3, "routine kind")?;
        let schema_id = schema_key(&schema);
        if kind == "P" {
            procedures.push(CatalogProcedure {
                id: format!("procedure:{object_id}"),
                schema_id: Some(schema_id),
                name: identifier(name),
                parameters: Vec::new(),
            });
        } else {
            functions.push(CatalogFunction {
                id: format!("function:{object_id}"),
                schema_id: Some(schema_id),
                name: identifier(name),
                parameters: Vec::new(),
                return_type: SqlTypeRef {
                    family: crate::backend::domain::catalog::SqlTypeFamily::Unknown,
                    name: "unknown".to_owned(),
                    nullable: None,
                    precision: None,
                    scale: None,
                    element_type: None,
                },
                variadic: false,
                overloads: Vec::new(),
                kind: Some(kind),
            });
        }
    }
    Ok((procedures, functions))
}

async fn fetch_sequences(connection: &mut Connection) -> Result<Vec<CatalogSequence>, String> {
    let rows = query(
        connection,
        "SELECT s.object_id, sc.name, s.name, TYPE_NAME(s.user_type_id) FROM sys.sequences s JOIN sys.schemas sc ON sc.schema_id = s.schema_id ORDER BY sc.name, s.name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let object_id = integer(&row, 0, "sequence id")?;
            let schema = text(&row, 1, "sequence schema")?;
            Ok(CatalogSequence {
                id: format!("sequence:{object_id}"),
                schema_id: schema_key(&schema),
                name: identifier(text(&row, 2, "sequence name")?),
                r#type: type_ref(&text(&row, 3, "sequence type")?, 0, 0),
            })
        })
        .collect()
}

async fn fetch_types(connection: &mut Connection) -> Result<Vec<CatalogType>, String> {
    let rows = query(
        connection,
        "SELECT t.user_type_id, s.name, t.name, TYPE_NAME(t.system_type_id) FROM sys.types t JOIN sys.schemas s ON s.schema_id = t.schema_id WHERE t.is_user_defined = 1 ORDER BY s.name, t.name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let type_id = integer(&row, 0, "type id")?;
            let schema = text(&row, 1, "type schema")?;
            let type_name = text(&row, 2, "type name")?;
            let system_type = text(&row, 3, "system type")?;
            Ok(CatalogType {
                id: format!("type:{type_id}"),
                schema_id: Some(schema_key(&schema)),
                name: identifier(type_name),
                family: type_ref(&system_type, 0, 0).family,
                definition: Some(system_type),
            })
        })
        .collect()
}

fn column_ids(
    columns: &HashMap<i64, Vec<ColumnMetadata>>,
    objects: &HashMap<i64, SqlServerObject>,
    object_id: i64,
    ids: &[i64],
) -> Vec<String> {
    let Some(object) = objects.get(&object_id) else {
        return Vec::new();
    };
    let key = object_key(&object.schema_id, &object.name);
    ids.iter()
        .filter_map(|id| {
            columns
                .get(&object_id)
                .and_then(|items| items.iter().find(|column| column.ordinal == *id as u32))
                .map(|_| column_key(&key, *id))
        })
        .collect()
}

async fn fetch_databases(connection: &mut Connection) -> Result<Vec<CatalogDatabase>, String> {
    let rows = query(
        connection,
        "SELECT name, database_id, CAST(CASE WHEN database_id <= 4 THEN 1 ELSE 0 END AS bit) FROM sys.databases WHERE LOWER(name) NOT IN ('model', 'msdb', 'tempdb') ORDER BY name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let name = text(&row, 0, "database name")?;
            let id = integer(&row, 1, "database id")?;
            Ok(CatalogDatabase {
                id: format!("database:{id}"),
                name: identifier(name),
                system: boolean(&row, 2, "system database")?,
            })
        })
        .collect()
}

async fn current_database_id(connection: &mut Connection) -> Result<String, String> {
    let rows = query(connection, "SELECT DB_ID()").await?;
    let row = rows
        .first()
        .ok_or_else(|| "SQL Server did not return the current database id".to_owned())?;
    Ok(format!(
        "database:{}",
        integer(row, 0, "current database id")?
    ))
}

fn database_listing_snapshot(
    request: &CatalogRefreshRequest,
    databases: Vec<CatalogDatabase>,
) -> CatalogSnapshot {
    CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:initial",
            request.connection_id, request.version
        ),
        connection_id: request.connection_id.clone(),
        database_id: String::new(),
        dialect: DialectId::Sqlserver,
        version: request.version.clone(),
        generated_at: request.generated_at.clone(),
        databases,
        schemas: Vec::new(),
        tables: Vec::new(),
        views: Vec::new(),
        materialized_views: Vec::new(),
        functions: Vec::new(),
        procedures: Vec::new(),
        sequences: Vec::new(),
        types: Vec::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        partitions: Vec::new(),
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys: Vec::new(),
        indexes: Vec::new(),
        constraints: Vec::new(),
    }
}

fn request_for_database(
    mut request: CatalogRefreshRequest,
    database: &str,
) -> CatalogRefreshRequest {
    request.database = database.to_owned();
    request
}

async fn fetch_triggers(connection: &mut Connection) -> Result<Vec<CatalogTrigger>, String> {
    let rows = query(
        connection,
        "SELECT tr.object_id, s.name, t.object_id, tr.name, tr.is_disabled, OBJECT_DEFINITION(tr.object_id)
         FROM sys.triggers tr
         JOIN sys.tables t ON t.object_id = tr.parent_id
         JOIN sys.schemas s ON s.schema_id = t.schema_id
         WHERE tr.is_ms_shipped = 0
         ORDER BY s.name, t.name, tr.name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let trigger_id = integer(&row, 0, "trigger id")?;
            let schema = text(&row, 1, "trigger schema")?;
            let table = integer(&row, 2, "trigger table id")?;
            Ok(CatalogTrigger {
                id: format!("trigger:{trigger_id}"),
                schema_id: Some(schema_key(&schema)),
                table_id: Some(format!("object:{table}")),
                name: identifier(text(&row, 3, "trigger name")?),
                enabled: !boolean(&row, 4, "trigger disabled")?,
                definition: optional_text(&row, 5, "trigger definition")?,
            })
        })
        .collect()
}

async fn fetch_partitions(connection: &mut Connection) -> Result<Vec<CatalogPartition>, String> {
    let rows = query(
        connection,
        "SELECT p.object_id, o.name, MAX(p.partition_number), ps.name
         FROM sys.partitions p
         JOIN sys.objects o ON o.object_id = p.object_id
         LEFT JOIN sys.indexes i ON i.object_id = p.object_id AND i.index_id = p.index_id
         LEFT JOIN sys.partition_schemes ps ON ps.data_space_id = i.data_space_id
         WHERE o.type = 'U' AND o.is_ms_shipped = 0
         GROUP BY p.object_id, o.name, ps.name
         HAVING MAX(p.partition_number) > 1
         ORDER BY o.name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let table_id = integer(&row, 0, "partition table id")?;
            let count = integer(&row, 2, "partition count")?;
            Ok(CatalogPartition {
                id: format!("partition:{table_id}"),
                table_id: format!("object:{table_id}"),
                name: Some(identifier(text(&row, 1, "partition table name")?)),
                parent_id: None,
                bound: Some(format!(
                    "{count} partitions{}",
                    optional_text(&row, 3, "partition scheme")?
                        .map(|value| format!(" on {value}"))
                        .unwrap_or_default()
                )),
            })
        })
        .collect()
}

async fn fetch_principals(connection: &mut Connection) -> Result<Vec<CatalogPrincipal>, String> {
    let rows = query(
        connection,
        "SELECT principal_id, name, type_desc, SUSER_SNAME(sid) FROM sys.database_principals WHERE name NOT IN ('guest', 'INFORMATION_SCHEMA', 'sys') ORDER BY name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let id = integer(&row, 0, "principal id")?;
            Ok(CatalogPrincipal {
                id: format!("principal:{id}"),
                name: identifier(text(&row, 1, "principal name")?),
                kind: text(&row, 2, "principal kind")?,
                login: optional_text(&row, 3, "principal login")?,
            })
        })
        .collect()
}

async fn fetch_permissions(connection: &mut Connection) -> Result<Vec<CatalogPermission>, String> {
    let rows = query(
        connection,
        "SELECT dp.grantee_principal_id, dp.major_id, dp.permission_name, dp.state_desc FROM sys.database_permissions dp ORDER BY dp.grantee_principal_id, dp.major_id, dp.permission_name",
    )
    .await?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let principal = integer(&row, 0, "permission principal id")?;
            let object = integer(&row, 1, "permission object id")?;
            Ok(CatalogPermission {
                id: format!("permission:{index}"),
                principal_id: format!("principal:{principal}"),
                object_id: (object != 0).then(|| format!("object:{object}")),
                permission: text(&row, 2, "permission name")?,
                state: text(&row, 3, "permission state")?,
            })
        })
        .collect()
}

async fn fetch_synonyms(connection: &mut Connection) -> Result<Vec<CatalogSynonym>, String> {
    let rows = query(
        connection,
        "SELECT s.name, sy.name, sy.base_object_name FROM sys.synonyms sy JOIN sys.schemas s ON s.schema_id = sy.schema_id ORDER BY s.name, sy.name",
    )
    .await?;
    rows.into_iter()
        .map(|row| {
            let schema = text(&row, 0, "synonym schema")?;
            Ok(CatalogSynonym {
                id: format!("synonym:{schema}:{}", text(&row, 1, "synonym name")?),
                schema_id: schema_key(&schema),
                name: identifier(text(&row, 1, "synonym name")?),
                target: text(&row, 2, "synonym target")?,
            })
        })
        .collect()
}

async fn connect_connection(
    request: &CatalogRefreshRequest,
    endpoint: &crate::backend::adapters::transport::PreparedEndpoint,
) -> ProviderResult<Connection> {
    let mut config = match request.auth_method {
        Some(ConnectionAuthMethod::WindowsNtlm) => tedious_rs::ConnectionConfig::ntlm(
            endpoint.host.clone(),
            request.user_name.clone(),
            request.password.clone(),
            None,
        ),
        _ => tedious_rs::ConnectionConfig::sql(
            endpoint.host.clone(),
            request.user_name.clone(),
            request.password.clone(),
        ),
    };
    config.port = endpoint.port;
    config.database = (!request.database.trim().is_empty()).then(|| request.database.clone());
    config.trust_server_certificate = request.trust_server_certificate;
    if let Some(application_name) = request.driver_properties.get("applicationName")
        && !application_name.trim().is_empty()
    {
        config.app_name = application_name.clone();
    }
    if let Some(timeout) = request
        .driver_properties
        .get("connectTimeoutSeconds")
        .and_then(|value| value.parse().ok())
    {
        config.connect_timeout = std::time::Duration::from_secs(timeout);
    }
    if let Some(timeout) = request
        .driver_properties
        .get("requestTimeoutSeconds")
        .and_then(|value| value.parse().ok())
    {
        config.request_timeout = std::time::Duration::from_secs(timeout);
    }
    if let Some(packet_size) = request
        .driver_properties
        .get("packetSize")
        .and_then(|value| value.parse().ok())
    {
        config.packet_size = packet_size;
    }
    if let Some(multi_subnet_failover) = request
        .driver_properties
        .get("multiSubnetFailover")
        .and_then(|value| value.parse().ok())
    {
        config.multi_subnet_failover = multi_subnet_failover;
    }
    if let Some(encrypt) = request
        .driver_properties
        .get("encrypt")
        .and_then(|value| value.parse().ok())
    {
        config.encrypt = encrypt;
    }
    if let Some(fallback) = request
        .driver_properties
        .get("fallbackToDefaultDb")
        .and_then(|value| value.parse().ok())
    {
        config.fallback_to_default_db = fallback;
    }
    if let Some(use_utc) = request
        .driver_properties
        .get("useUtc")
        .and_then(|value| value.parse().ok())
    {
        config.use_utc = use_utc;
    }
    if let Some(row_collection_on_done) = request
        .driver_properties
        .get("rowCollectionOnDone")
        .and_then(|value| value.parse().ok())
    {
        config.row_collection_on_done = row_collection_on_done;
    }
    let mut connection = Connection::new(config);
    connection
        .connect()
        .await
        .map_err(|error| error.to_string())?;
    Ok(connection)
}

async fn introspect_initial(
    connection: &mut Connection,
    request: &CatalogRefreshRequest,
) -> ProviderResult<CatalogSnapshot> {
    if request.database.trim().is_empty() && request.show_all_catalog_items {
        return Ok(database_listing_snapshot(
            request,
            fetch_databases(connection).await?,
        ));
    }
    let database_id = current_database_id(connection).await?;
    let mut databases = fetch_databases(connection).await?;
    let schemas = fetch_schemas(connection, &database_id).await?;
    let objects = fetch_objects(connection).await?;
    let columns = fetch_columns(connection, &objects).await?;
    let mut tables = Vec::new();
    let mut views = Vec::new();
    for object in objects.values() {
        let id = object_key(&object.schema_id, &object.name);
        let object_columns = columns.get(&object.object_id).cloned().unwrap_or_default();
        if object.kind == "V" {
            views.push(CatalogView {
                id,
                catalog_id: Some(database_id.clone()),
                database_id: database_id.clone(),
                schema_id: schema_key(&object.schema_id),
                name: identifier(object.name.clone()),
                columns: object_columns,
                comment: None,
            });
        } else {
            tables.push(CatalogTable {
                id,
                catalog_id: Some(database_id.clone()),
                database_id: database_id.clone(),
                schema_id: schema_key(&object.schema_id),
                name: identifier(object.name.clone()),
                columns: object_columns,
                primary_key: None,
                foreign_keys: Vec::new(),
                unique_constraints: Vec::new(),
                check_constraints: Vec::new(),
                indexes: Vec::new(),
                estimated_rows: None,
                comment: None,
                foreign: false,
                temporary: false,
            });
        }
    }
    if !request.show_all_catalog_items {
        databases.retain(|database| {
            if request.database.trim().is_empty() {
                database.id == database_id
            } else {
                database.name.raw.eq_ignore_ascii_case(&request.database)
            }
        });
    }
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:initial",
            request.connection_id, request.version
        ),
        connection_id: request.connection_id.clone(),
        database_id,
        dialect: DialectId::Sqlserver,
        version: request.version.clone(),
        generated_at: request.generated_at.clone(),
        databases,
        schemas,
        tables,
        views,
        materialized_views: Vec::new(),
        functions: Vec::new(),
        procedures: Vec::new(),
        sequences: Vec::new(),
        types: Vec::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        partitions: Vec::new(),
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys: Vec::new(),
        indexes: Vec::new(),
        constraints: Vec::new(),
    })
}

async fn introspect_database(
    connection: &mut Connection,
    request: &CatalogRefreshRequest,
) -> Result<CatalogSnapshot, String> {
    let database_id = current_database_id(connection).await?;
    let schemas = fetch_schemas(connection, &database_id).await?;
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:database:{}",
            request.connection_id, request.version, request.database
        ),
        connection_id: request.connection_id.clone(),
        database_id,
        dialect: DialectId::Sqlserver,
        version: request.version.clone(),
        generated_at: request.generated_at.clone(),
        databases: Vec::new(),
        schemas,
        tables: Vec::new(),
        views: Vec::new(),
        materialized_views: Vec::new(),
        functions: Vec::new(),
        procedures: Vec::new(),
        sequences: Vec::new(),
        types: Vec::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        partitions: Vec::new(),
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys: Vec::new(),
        indexes: Vec::new(),
        constraints: Vec::new(),
    })
}

async fn introspect_schema(
    connection: &mut Connection,
    request: CatalogRefreshRequest,
    requested_schema_id: String,
) -> ProviderResult<CatalogSnapshot> {
    let schema_name = requested_schema_id
        .strip_prefix("schema:")
        .ok_or_else(|| "Invalid SQL Server schema identifier".to_owned())?;
    let objects = fetch_objects_for_schema(connection, schema_name).await?;
    let database_id = current_database_id(connection).await?;
    let mut tables = Vec::new();
    let mut views = Vec::new();
    for object in objects.values() {
        let id = object_key(&object.schema_id, &object.name);
        if object.kind == "V" {
            views.push(CatalogView {
                id,
                catalog_id: Some(database_id.clone()),
                database_id: database_id.clone(),
                schema_id: schema_key(&object.schema_id),
                name: identifier(object.name.clone()),
                columns: Vec::new(),
                comment: None,
            });
        } else {
            tables.push(CatalogTable {
                id,
                catalog_id: Some(database_id.clone()),
                database_id: database_id.clone(),
                schema_id: schema_key(&object.schema_id),
                name: identifier(object.name.clone()),
                columns: Vec::new(),
                primary_key: None,
                foreign_keys: Vec::new(),
                unique_constraints: Vec::new(),
                check_constraints: Vec::new(),
                indexes: Vec::new(),
                estimated_rows: None,
                comment: None,
                foreign: false,
                temporary: false,
            });
        }
    }
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:schema",
            request.connection_id, request.version
        ),
        connection_id: request.connection_id,
        database_id,
        dialect: DialectId::Sqlserver,
        version: request.version,
        generated_at: request.generated_at,
        databases: Vec::new(),
        schemas: Vec::new(),
        tables,
        views,
        materialized_views: Vec::new(),
        functions: Vec::new(),
        procedures: Vec::new(),
        sequences: Vec::new(),
        types: Vec::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        partitions: Vec::new(),
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys: Vec::new(),
        indexes: Vec::new(),
        constraints: Vec::new(),
    })
}

async fn introspect_table_connected(
    connection: &mut Connection,
    request: &CatalogRefreshRequest,
    schema_name: &str,
    object_name: &str,
) -> ProviderResult<CatalogSnapshot> {
    let Some(source) = fetch_object_for_name(connection, schema_name, object_name).await? else {
        return Err("SQL Server table was not found".into());
    };
    if source.kind != "U" {
        return Err("SQL Server catalog object is not a table".into());
    }
    let key_members = fetch_key_members_for_object(connection, source.object_id).await?;
    let mut object_ids = vec![source.object_id];
    object_ids.extend(
        key_members
            .iter()
            .filter_map(|member| member.target_object_id),
    );
    object_ids.sort_unstable();
    object_ids.dedup();
    let objects = fetch_objects_for_ids(connection, &object_ids).await?;
    let mut columns = fetch_columns_for_ids(connection, &objects, &object_ids).await?;
    let index_members = fetch_indexes_for_object(connection, source.object_id).await?;
    let checks = fetch_checks_for_object(connection, source.object_id).await?;

    let mut indexes_by_id: HashMap<i64, Vec<IndexMember>> = HashMap::new();
    for member in index_members {
        indexes_by_id
            .entry(member.index_id)
            .or_default()
            .push(member);
    }
    let mut indexes = Vec::new();
    let mut primary_key = None;
    let mut unique_constraints = Vec::new();
    for (index_id, mut members) in indexes_by_id {
        members.sort_by_key(|member| member.ordinal);
        let Some(first) = members.first() else {
            continue;
        };
        let member_column_ids = members
            .iter()
            .map(|member| member.column_id)
            .collect::<Vec<_>>();
        let column_ids = column_ids(&columns, &objects, source.object_id, &member_column_ids);
        let table_id = object_key(&source.schema_id, &source.name);
        let index = IndexMetadata {
            id: format!("index:{}:{index_id}", source.object_id),
            name: identifier(first.name.clone()),
            table_id: table_id.clone(),
            column_ids: column_ids.clone(),
            unique: first.unique,
            primary: first.primary,
        };
        if first.primary {
            primary_key = Some(PrimaryKeyMetadata {
                id: format!("primary-key:{}:{index_id}", source.object_id),
                name: identifier(first.name.clone()),
                table_id: table_id.clone(),
                column_ids: column_ids.clone(),
            });
        } else if first.unique {
            unique_constraints.push(UniqueConstraintMetadata {
                id: format!("unique:{}:{index_id}", source.object_id),
                name: identifier(first.name.clone()),
                table_id: table_id.clone(),
                column_ids,
            });
        }
        indexes.push(index);
    }

    let mut foreign_keys = Vec::new();
    for member in key_members {
        if member.constraint_type != "F" {
            continue;
        }
        let Some(target_object_id) = member.target_object_id else {
            continue;
        };
        let Some(target) = objects.get(&target_object_id) else {
            continue;
        };
        foreign_keys.push(ForeignKeyMetadata {
            id: format!("foreign-key:{}", member.object_id),
            name: identifier(member.name),
            source_table_id: object_key(&source.schema_id, &source.name),
            source_column_ids: column_ids(
                &columns,
                &objects,
                source.object_id,
                &[member.parent_column_id],
            ),
            target_table_id: object_key(&target.schema_id, &target.name),
            target_column_ids: member
                .target_column_id
                .map(|column_id| column_ids(&columns, &objects, target_object_id, &[column_id]))
                .unwrap_or_default(),
        });
    }
    for object_columns in columns.values_mut() {
        for column in object_columns {
            column.primary_key = primary_key
                .as_ref()
                .is_some_and(|key| key.column_ids.contains(&column.id));
            column.foreign_key_memberships = foreign_keys
                .iter()
                .filter(|key| key.source_column_ids.contains(&column.id))
                .map(|key| key.id.clone())
                .collect();
            column.unique_memberships = unique_constraints
                .iter()
                .filter(|key| key.column_ids.contains(&column.id))
                .map(|key| key.id.clone())
                .chain(
                    indexes
                        .iter()
                        .filter(|index| index.unique && index.column_ids.contains(&column.id))
                        .map(|index| index.id.clone()),
                )
                .collect();
        }
    }

    let table_id = object_key(&source.schema_id, &source.name);
    let table_columns = columns.remove(&source.object_id).unwrap_or_default();
    let check_constraints = checks
        .into_iter()
        .map(|(id, expression)| CheckConstraintMetadata {
            id: format!("check:{id}"),
            name: identifier(format!("check_{id}")),
            table_id: table_id.clone(),
            expression: Some(expression),
        })
        .collect::<Vec<_>>();
    let constraints = primary_key
        .iter()
        .cloned()
        .map(CatalogConstraint::PrimaryKey)
        .chain(
            unique_constraints
                .iter()
                .cloned()
                .map(CatalogConstraint::Unique),
        )
        .chain(
            check_constraints
                .iter()
                .cloned()
                .map(CatalogConstraint::Check),
        )
        .collect();
    let database_id = current_database_id(connection).await?;
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:table",
            request.connection_id, request.version
        ),
        connection_id: request.connection_id.clone(),
        database_id: database_id.clone(),
        dialect: DialectId::Sqlserver,
        version: request.version.clone(),
        generated_at: request.generated_at.clone(),
        databases: Vec::new(),
        schemas: Vec::new(),
        tables: vec![CatalogTable {
            id: table_id.clone(),
            catalog_id: Some(database_id.clone()),
            database_id,
            schema_id: schema_key(&source.schema_id),
            name: identifier(source.name),
            columns: table_columns,
            primary_key,
            foreign_keys: foreign_keys.clone(),
            unique_constraints,
            check_constraints,
            indexes: indexes.clone(),
            estimated_rows: None,
            comment: None,
            foreign: false,
            temporary: false,
        }],
        views: Vec::new(),
        materialized_views: Vec::new(),
        functions: Vec::new(),
        procedures: Vec::new(),
        sequences: Vec::new(),
        types: Vec::new(),
        triggers: Vec::new(),
        extensions: Vec::new(),
        partitions: Vec::new(),
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys,
        indexes,
        constraints,
    })
}

async fn introspect_table(
    connection: &mut Connection,
    request: CatalogRefreshRequest,
    requested_table_id: String,
) -> ProviderResult<CatalogSnapshot> {
    let table_key = requested_table_id
        .strip_prefix("object:")
        .ok_or_else(|| "Invalid SQL Server table identifier".to_owned())?;
    let mut parts = table_key.splitn(2, ':');
    let schema_name = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Invalid SQL Server table identifier".to_owned())?;
    let object_name = parts
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Invalid SQL Server table identifier".to_owned())?;
    introspect_table_connected(connection, &request, schema_name, object_name).await
}

async fn introspect_connected(
    connection: &mut Connection,
    request: &CatalogRefreshRequest,
) -> ProviderResult<CatalogSnapshot> {
    let database_id = current_database_id(connection).await?;
    let schemas = fetch_schemas(connection, &database_id).await?;
    let objects = fetch_objects(connection).await?;
    let columns = fetch_columns(connection, &objects).await?;
    let index_members = fetch_indexes(connection).await?;
    let key_members = fetch_key_members(connection).await?;
    let checks = fetch_checks(connection).await?;
    let (procedures, functions) = fetch_routines(connection).await?;
    let sequences = fetch_sequences(connection).await?;
    let types = fetch_types(connection).await?;
    let databases = fetch_databases(connection).await?;
    let triggers = fetch_triggers(connection).await?;
    let partitions = fetch_partitions(connection).await?;
    let principals = fetch_principals(connection).await?;
    let permissions = fetch_permissions(connection).await?;
    let synonyms = fetch_synonyms(connection).await?;

    let mut indexes_by_id: HashMap<(i64, i64), Vec<IndexMember>> = HashMap::new();
    for member in index_members {
        indexes_by_id
            .entry((member.object_id, member.index_id))
            .or_default()
            .push(member);
    }
    let mut indexes = Vec::new();
    let mut primary_keys: HashMap<i64, PrimaryKeyMetadata> = HashMap::new();
    let mut unique_constraints: HashMap<i64, UniqueConstraintMetadata> = HashMap::new();
    for ((object_id, index_id), mut members) in indexes_by_id {
        members.sort_by_key(|member| member.ordinal);
        let Some(first) = members.first() else {
            continue;
        };
        let ids = column_ids(
            &columns,
            &objects,
            object_id,
            &members
                .iter()
                .map(|member| member.column_id)
                .collect::<Vec<_>>(),
        );
        let Some(object) = objects.get(&object_id) else {
            continue;
        };
        let table_id = object_key(&object.schema_id, &object.name);
        let id = format!("index:{object_id}:{index_id}");
        indexes.push(IndexMetadata {
            id: id.clone(),
            name: identifier(first.name.clone()),
            table_id: table_id.clone(),
            column_ids: ids.clone(),
            unique: first.unique,
            primary: first.primary,
        });
        if first.primary {
            primary_keys.insert(
                object_id,
                PrimaryKeyMetadata {
                    id: format!("primary-key:{object_id}:{index_id}"),
                    name: identifier(first.name.clone()),
                    table_id,
                    column_ids: ids,
                },
            );
        } else if first.unique {
            unique_constraints.insert(
                object_id,
                UniqueConstraintMetadata {
                    id: format!("unique:{object_id}:{index_id}"),
                    name: identifier(first.name.clone()),
                    table_id,
                    column_ids: ids,
                },
            );
        }
    }

    let mut foreign_keys = Vec::new();
    let mut check_constraints = Vec::new();
    let mut foreign_keys_by_source: HashMap<i64, Vec<String>> = HashMap::new();
    for member in &key_members {
        let Some(source) = objects.get(&member.parent_object_id) else {
            continue;
        };
        let source_table_id = object_key(&source.schema_id, &source.name);
        if member.constraint_type == "C" {
            check_constraints.push(CheckConstraintMetadata {
                id: format!("check:{}", member.object_id),
                name: identifier(member.name.clone()),
                table_id: source_table_id,
                expression: checks.get(&member.object_id).cloned(),
            });
            continue;
        }
        if member.constraint_type != "F" {
            continue;
        }
        let Some(target_id) = member.target_object_id else {
            continue;
        };
        let Some(target) = objects.get(&target_id) else {
            continue;
        };
        let target_table_id = object_key(&target.schema_id, &target.name);
        let id = format!("foreign-key:{}", member.object_id);
        foreign_keys_by_source
            .entry(member.parent_object_id)
            .or_default()
            .push(id.clone());
        foreign_keys.push(ForeignKeyMetadata {
            id,
            name: identifier(member.name.clone()),
            source_table_id: source_table_id.clone(),
            source_column_ids: column_ids(
                &columns,
                &objects,
                member.parent_object_id,
                &[member.parent_column_id],
            ),
            target_table_id,
            target_column_ids: column_ids(
                &columns,
                &objects,
                target_id,
                &[member.target_column_id.unwrap_or_default()],
            ),
        });
    }

    let mut tables = Vec::new();
    let mut views = Vec::new();
    for object in objects.values() {
        let table_id = object_key(&object.schema_id, &object.name);
        let mut object_columns = columns.get(&object.object_id).cloned().unwrap_or_default();
        if let Some(primary_key) = primary_keys.get(&object.object_id) {
            for column in &mut object_columns {
                column.primary_key = primary_key.column_ids.contains(&column.id);
            }
        }
        for unique in unique_constraints
            .values()
            .filter(|unique| unique.table_id == table_id)
        {
            for column in &mut object_columns {
                if unique.column_ids.contains(&column.id) {
                    column.unique_memberships.push(unique.id.clone());
                }
            }
        }
        for foreign_key in foreign_keys
            .iter()
            .filter(|foreign_key| foreign_key.source_table_id == table_id)
        {
            for column in &mut object_columns {
                if foreign_key.source_column_ids.contains(&column.id) {
                    column.foreign_key_memberships.push(foreign_key.id.clone());
                }
            }
        }
        if object.kind == "V" {
            views.push(CatalogView {
                id: table_id,
                catalog_id: Some(database_id.clone()),
                database_id: database_id.clone(),
                schema_id: schema_key(&object.schema_id),
                name: identifier(object.name.clone()),
                columns: object_columns,
                comment: None,
            });
        } else {
            tables.push(CatalogTable {
                id: table_id,
                catalog_id: Some(database_id.clone()),
                database_id: database_id.clone(),
                schema_id: schema_key(&object.schema_id),
                name: identifier(object.name.clone()),
                columns: object_columns,
                primary_key: primary_keys.get(&object.object_id).cloned(),
                foreign_keys: foreign_keys
                    .iter()
                    .filter(|foreign_key| {
                        foreign_key.source_table_id == object_key(&object.schema_id, &object.name)
                    })
                    .cloned()
                    .collect(),
                unique_constraints: unique_constraints
                    .values()
                    .filter(|unique| unique.table_id == object_key(&object.schema_id, &object.name))
                    .cloned()
                    .collect(),
                check_constraints: check_constraints
                    .iter()
                    .filter(|constraint| {
                        constraint.table_id == object_key(&object.schema_id, &object.name)
                    })
                    .cloned()
                    .collect(),
                indexes: indexes
                    .iter()
                    .filter(|index| index.table_id == object_key(&object.schema_id, &object.name))
                    .cloned()
                    .collect(),
                estimated_rows: None,
                comment: None,
                foreign: false,
                temporary: false,
            });
        }
    }

    let constraints = primary_keys
        .values()
        .cloned()
        .map(CatalogConstraint::PrimaryKey)
        .chain(
            unique_constraints
                .values()
                .cloned()
                .map(CatalogConstraint::Unique),
        )
        .chain(
            check_constraints
                .iter()
                .cloned()
                .map(CatalogConstraint::Check),
        )
        .collect();
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!("catalog:{}:{}", request.connection_id, request.version),
        connection_id: request.connection_id.clone(),
        database_id,
        dialect: DialectId::Sqlserver,
        version: request.version.clone(),
        generated_at: request.generated_at.clone(),
        databases,
        schemas,
        tables,
        views,
        materialized_views: Vec::new(),
        functions,
        procedures,
        sequences,
        types,
        triggers,
        extensions: Vec::new(),
        partitions,
        principals,
        permissions,
        synonyms,
        foreign_keys,
        indexes,
        constraints,
    })
}

#[derive(Clone, Default)]
pub struct SqlServerAdapter {
    sessions: Arc<Mutex<HashMap<String, Arc<tokio::sync::Mutex<Connection>>>>>,
    transport_guards: Arc<Mutex<HashMap<String, Arc<TransportGuard>>>>,
}

impl SqlServerAdapter {
    fn session_key(request: &CatalogRefreshRequest) -> String {
        if request.database.trim().is_empty() {
            request.connection_id.clone()
        } else {
            format!("{}::database:{}", request.connection_id, request.database)
        }
    }

    async fn session_for(
        &self,
        request: &CatalogRefreshRequest,
    ) -> ProviderResult<Arc<tokio::sync::Mutex<Connection>>> {
        let session_key = Self::session_key(request);
        if let Some(session) = self
            .sessions
            .lock()
            .expect("SQL Server catalog session cache poisoned")
            .get(&session_key)
            .cloned()
        {
            return Ok(session);
        }
        let endpoint = prepare_endpoint_for(&request.server, request.port, &request.transport)
            .await
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> {
                error.to_string().into()
            })?;
        let guard = endpoint.guard.clone();
        let connection = connect_connection(request, &endpoint).await?;
        let session = Arc::new(tokio::sync::Mutex::new(connection));
        let mut sessions = self
            .sessions
            .lock()
            .expect("SQL Server catalog session cache poisoned");
        let session = sessions
            .entry(session_key.clone())
            .or_insert_with(|| session.clone())
            .clone();
        if let Some(guard) = guard {
            self.transport_guards
                .lock()
                .expect("SQL Server transport cache poisoned")
                .insert(session_key, guard);
        }
        Ok(session)
    }
}

impl CatalogProvider for SqlServerAdapter {
    fn dialect(&self) -> DialectId {
        DialectId::Sqlserver
    }

    fn invalidate(&self, connection_id: &str) {
        let prefix = format!("{connection_id}::database:");
        self.sessions
            .lock()
            .expect("SQL Server catalog session cache poisoned")
            .retain(|key, _| key != connection_id && !key.starts_with(&prefix));
        self.transport_guards
            .lock()
            .expect("SQL Server transport cache poisoned")
            .retain(|key, _| key != connection_id && !key.starts_with(&prefix));
    }

    fn refresh(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        let adapter = self.clone();
        Box::pin(async move {
            let session = adapter.session_for(&request).await.map_err(|error| {
                CatalogProviderError::Driver {
                    dialect: DialectId::Sqlserver,
                    message: error.to_string(),
                }
            })?;
            let mut connection = session.lock().await;
            let result = if request.database.trim().is_empty() && request.show_all_catalog_items {
                fetch_databases(&mut connection)
                    .await
                    .map(|databases| database_listing_snapshot(&request, databases))
            } else {
                introspect_connected(&mut connection, &request)
                    .await
                    .map_err(|error| error.to_string())
            };
            result.map_err(|message| CatalogProviderError::Driver {
                dialect: DialectId::Sqlserver,
                message,
            })
        })
    }

    fn initial(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        let adapter = self.clone();
        Box::pin(async move {
            let session = adapter.session_for(&request).await.map_err(|error| {
                CatalogProviderError::Driver {
                    dialect: DialectId::Sqlserver,
                    message: error.to_string(),
                }
            })?;
            let mut connection = session.lock().await;
            introspect_initial(&mut connection, &request)
                .await
                .map_err(|message| CatalogProviderError::Driver {
                    dialect: DialectId::Sqlserver,
                    message: message.to_string(),
                })
        })
    }

    fn expand(&self, request: CatalogRefreshRequest, scope: CatalogScope) -> CatalogProviderFuture {
        let adapter = self.clone();
        Box::pin(async move {
            match scope {
                CatalogScope::Database { database_id } => {
                    let request = request_for_database(request, &database_id);
                    let session = adapter.session_for(&request).await.map_err(|error| {
                        CatalogProviderError::Driver {
                            dialect: DialectId::Sqlserver,
                            message: error.to_string(),
                        }
                    })?;
                    let mut connection = session.lock().await;
                    introspect_database(&mut connection, &request)
                        .await
                        .map_err(|message| CatalogProviderError::Driver {
                            dialect: DialectId::Sqlserver,
                            message: message.to_string(),
                        })
                }
                CatalogScope::Schema {
                    schema_id,
                    database_id,
                } => {
                    let request = match database_id {
                        Some(database) => request_for_database(request, &database),
                        None => request,
                    };
                    let session = adapter.session_for(&request).await.map_err(|error| {
                        CatalogProviderError::Driver {
                            dialect: DialectId::Sqlserver,
                            message: error.to_string(),
                        }
                    })?;
                    let mut connection = session.lock().await;
                    introspect_schema(&mut connection, request, schema_id)
                        .await
                        .map_err(|message| CatalogProviderError::Driver {
                            dialect: DialectId::Sqlserver,
                            message: message.to_string(),
                        })
                }
                CatalogScope::Table {
                    table_id,
                    database_id,
                } => {
                    let request = match database_id {
                        Some(database) => request_for_database(request, &database),
                        None => request,
                    };
                    let session = adapter.session_for(&request).await.map_err(|error| {
                        CatalogProviderError::Driver {
                            dialect: DialectId::Sqlserver,
                            message: error.to_string(),
                        }
                    })?;
                    let mut connection = session.lock().await;
                    introspect_table(&mut connection, request, table_id)
                        .await
                        .map_err(|message| CatalogProviderError::Driver {
                            dialect: DialectId::Sqlserver,
                            message: message.to_string(),
                        })
                }
            }
        })
    }
}
