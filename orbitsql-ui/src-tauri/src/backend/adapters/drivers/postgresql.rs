use crate::backend::adapters::transport::{TransportGuard, prepare_endpoint_for};
use crate::backend::adapters::type_mapping::{identifier, type_ref};
use crate::backend::domain::catalog::{
    CATALOG_SCHEMA_VERSION, CatalogConstraint, CatalogDatabase, CatalogExtension, CatalogFunction,
    CatalogParameter, CatalogPartition, CatalogPermission, CatalogPrincipal, CatalogProcedure,
    CatalogSchema, CatalogSequence, CatalogSnapshot, CatalogTable, CatalogTrigger, CatalogType,
    CatalogView, CheckConstraintMetadata, ColumnMetadata, DialectId, ForeignKeyMetadata,
    IndexMetadata, PrimaryKeyMetadata, UniqueConstraintMetadata,
};
use crate::backend::ports::database::{CatalogRefreshRequest, ProviderResult};
use crate::backend::ports::{
    CatalogProvider, CatalogProviderError, CatalogProviderFuture, CatalogScope,
};
use sqlx::{
    Row,
    postgres::{PgConnectOptions, PgPoolOptions, PgRow, PgSslMode},
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct PgObject {
    id: i64,
    schema_id: String,
    name: String,
    kind: String,
    comment: Option<String>,
    estimated_rows: Option<u64>,
}

fn parse_ordinals(value: &str) -> Vec<u32> {
    value
        .trim_matches(['{', '}'])
        .split(',')
        .filter_map(|entry| entry.trim().parse::<u32>().ok())
        .collect()
}

fn column_ids(table_id: i64, ordinals: &str) -> Vec<String> {
    parse_ordinals(ordinals)
        .into_iter()
        .map(|ordinal| format!("column:{table_id}:{ordinal}"))
        .collect()
}

fn schema_id(value: i64) -> String {
    format!("schema:{value}")
}

async fn fetch_databases(pool: &sqlx::PgPool) -> ProviderResult<Vec<CatalogDatabase>> {
    let rows = sqlx::query(
        "SELECT datname, datistemplate FROM pg_database WHERE datallowconn ORDER BY datname",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let name = row_text(&row, "datname")?;
            Ok(CatalogDatabase {
                id: format!("database:{name}"),
                name: identifier(name),
                system: row.try_get("datistemplate")?,
            })
        })
        .collect()
}

async fn fetch_current_database(pool: &sqlx::PgPool) -> ProviderResult<String> {
    let row = sqlx::query("SELECT current_database() AS name")
        .fetch_one(pool)
        .await?;
    row_text(&row, "name")
}

fn object_id(value: i64) -> String {
    format!("object:{value}")
}

fn row_text(row: &PgRow, column: &str) -> ProviderResult<String> {
    Ok(row.try_get(column)?)
}

fn row_optional_text(row: &PgRow, column: &str) -> ProviderResult<Option<String>> {
    Ok(row.try_get(column)?)
}

async fn fetch_schemas(
    pool: &sqlx::PgPool,
    database_id: &str,
) -> ProviderResult<Vec<CatalogSchema>> {
    let rows = sqlx::query(
        "SELECT oid::bigint AS schema_id, nspname AS name, obj_description(oid, 'pg_namespace') AS comment
         FROM pg_namespace
         WHERE nspname NOT IN ('pg_catalog', 'information_schema')
           AND nspname NOT LIKE 'pg_temp_%' AND nspname NOT LIKE 'pg_toast%'
         ORDER BY nspname",
    )
    .fetch_all(pool)
    .await
    ?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("schema_id")?;
            Ok(CatalogSchema {
                id: schema_id(id),
                catalog_id: Some(database_id.to_owned()),
                database_id: database_id.to_owned(),
                name: identifier(row_text(&row, "name")?),
                comment: row_optional_text(&row, "comment")?,
            })
        })
        .collect()
}

async fn fetch_objects(pool: &sqlx::PgPool) -> ProviderResult<HashMap<i64, PgObject>> {
    fetch_objects_in_schema(pool, None).await
}

async fn fetch_objects_in_schema(
    pool: &sqlx::PgPool,
    schema_oid: Option<i64>,
) -> ProviderResult<HashMap<i64, PgObject>> {
    let rows = sqlx::query(
        "SELECT c.oid::bigint AS object_id, n.oid::bigint AS schema_id, c.relname AS name,
                c.relkind::text AS kind, obj_description(c.oid, 'pg_class') AS comment,
                CASE WHEN c.reltuples >= 0 THEN c.reltuples::bigint ELSE NULL END AS estimated_rows
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.relkind IN ('r', 'p', 'v', 'm', 'f')
           AND n.nspname NOT IN ('pg_catalog', 'information_schema')
           AND n.nspname NOT LIKE 'pg_temp_%' AND n.nspname NOT LIKE 'pg_toast%'
           AND ($1::bigint IS NULL OR n.oid::bigint = $1)
         ORDER BY n.nspname, c.relname",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("object_id")?;
            let schema: i64 = row.try_get("schema_id")?;
            let estimated_rows = row
                .try_get::<Option<i64>, _>("estimated_rows")?
                .and_then(|value| u64::try_from(value).ok());
            Ok((
                id,
                PgObject {
                    id,
                    schema_id: schema_id(schema),
                    name: row_text(&row, "name")?,
                    kind: row_text(&row, "kind")?,
                    comment: row_optional_text(&row, "comment")?,
                    estimated_rows,
                },
            ))
        })
        .collect::<ProviderResult<HashMap<_, _>>>()
}

async fn fetch_objects_for_ids(
    pool: &sqlx::PgPool,
    object_ids: &[i64],
) -> ProviderResult<HashMap<i64, PgObject>> {
    let rows = sqlx::query(
        "SELECT c.oid::bigint AS object_id, n.oid::bigint AS schema_id, c.relname AS name,
                c.relkind::text AS kind, obj_description(c.oid, 'pg_class') AS comment,
                CASE WHEN c.reltuples >= 0 THEN c.reltuples::bigint ELSE NULL END AS estimated_rows
         FROM pg_class c
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE c.oid::bigint = ANY($1::bigint[])
         ORDER BY n.nspname, c.relname",
    )
    .bind(object_ids.to_vec())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("object_id")?;
            let schema: i64 = row.try_get("schema_id")?;
            let estimated_rows = row
                .try_get::<Option<i64>, _>("estimated_rows")?
                .and_then(|value| u64::try_from(value).ok());
            Ok((
                id,
                PgObject {
                    id,
                    schema_id: schema_id(schema),
                    name: row_text(&row, "name")?,
                    kind: row_text(&row, "kind")?,
                    comment: row_optional_text(&row, "comment")?,
                    estimated_rows,
                },
            ))
        })
        .collect::<ProviderResult<HashMap<_, _>>>()
}

async fn fetch_foreign_key_target_ids(
    pool: &sqlx::PgPool,
    object_id: i64,
) -> ProviderResult<Vec<i64>> {
    let rows = sqlx::query(
        "SELECT confrelid::bigint AS target_object_id
         FROM pg_constraint
         WHERE conrelid::bigint = $1 AND contype = 'f' AND confrelid <> 0
         ORDER BY confrelid",
    )
    .bind(object_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| Ok(row.try_get("target_object_id")?))
        .collect()
}

async fn fetch_columns_for_ids(
    pool: &sqlx::PgPool,
    object_ids: &[i64],
) -> ProviderResult<HashMap<i64, Vec<ColumnMetadata>>> {
    let rows = sqlx::query(
        "SELECT a.attrelid::bigint AS object_id, a.attnum::int AS ordinal, a.attname AS name,
                format_type(a.atttypid, a.atttypmod) AS type_name,
                NOT a.attnotnull AS nullable, a.attidentity::text AS identity_kind,
                a.attgenerated::text AS generated_kind,
                pg_get_expr(ad.adbin, ad.adrelid) AS default_value,
                col_description(a.attrelid, a.attnum) AS comment,
                format_type(a.atttypid, NULL) AS base_type
         FROM pg_attribute a
         LEFT JOIN pg_attrdef ad ON ad.adrelid = a.attrelid AND ad.adnum = a.attnum
         WHERE a.attnum > 0 AND NOT a.attisdropped AND a.attrelid::bigint = ANY($1::bigint[])
         ORDER BY a.attrelid, a.attnum",
    )
    .bind(object_ids.to_vec())
    .fetch_all(pool)
    .await?;
    let mut columns_by_table: HashMap<i64, Vec<ColumnMetadata>> = HashMap::new();
    for row in rows {
        let object_id_value: i64 = row.try_get("object_id")?;
        let ordinal: i32 = row.try_get("ordinal")?;
        let type_name = row_text(&row, "type_name")?;
        let identity_kind = row_text(&row, "identity_kind")?;
        let generated_kind = row_text(&row, "generated_kind")?;
        columns_by_table
            .entry(object_id_value)
            .or_default()
            .push(ColumnMetadata {
                id: format!("column:{object_id_value}:{ordinal}"),
                table_id: object_id(object_id_value),
                ordinal: u32::try_from(ordinal)?,
                name: identifier(row_text(&row, "name")?),
                r#type: type_ref(&type_name, 0, 0),
                nullable: row.try_get("nullable")?,
                generated: !generated_kind.is_empty(),
                identity: !identity_kind.is_empty(),
                computed: generated_kind == "s",
                default_value: row_optional_text(&row, "default_value")?,
                primary_key: false,
                foreign_key_memberships: Vec::new(),
                unique_memberships: Vec::new(),
                comment: row_optional_text(&row, "comment")?,
            });
    }
    Ok(columns_by_table)
}

async fn fetch_indexes_for_ids(
    pool: &sqlx::PgPool,
    object_ids: &[i64],
) -> ProviderResult<Vec<IndexMetadata>> {
    let rows = sqlx::query(
        "SELECT i.indrelid::bigint AS table_id, i.indexrelid::bigint AS index_id,
                idx.relname AS name, i.indisunique AS unique_index,
                i.indisprimary AS primary_index, i.indkey::text AS column_ordinals
         FROM pg_index i
         JOIN pg_class idx ON idx.oid = i.indexrelid
         WHERE i.indrelid::bigint = ANY($1::bigint[])
         ORDER BY i.indrelid, idx.relname",
    )
    .bind(object_ids.to_vec())
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let table_id: i64 = row.try_get("table_id")?;
            let index_id: i64 = row.try_get("index_id")?;
            let ordinals = row_text(&row, "column_ordinals")?;
            Ok(IndexMetadata {
                id: format!("index:{index_id}"),
                name: identifier(row_text(&row, "name")?),
                table_id: object_id(table_id),
                column_ids: column_ids(table_id, &ordinals),
                unique: row.try_get("unique_index")?,
                primary: row.try_get("primary_index")?,
            })
        })
        .collect()
}

async fn fetch_constraints_for_ids(
    pool: &sqlx::PgPool,
    object_ids: &[i64],
) -> ProviderResult<(
    Vec<ForeignKeyMetadata>,
    HashMap<i64, Vec<PrimaryKeyMetadata>>,
    HashMap<i64, Vec<UniqueConstraintMetadata>>,
    HashMap<i64, Vec<CheckConstraintMetadata>>,
)> {
    let rows = sqlx::query(
        "SELECT con.oid::bigint AS constraint_id, con.conrelid::bigint AS source_table_id,
                con.confrelid::bigint AS target_table_id, con.conname AS name,
                con.contype::text AS constraint_type, con.conkey::text AS source_ordinals,
                con.confkey::text AS target_ordinals, pg_get_constraintdef(con.oid) AS definition
         FROM pg_constraint con
         WHERE con.contype IN ('p', 'u', 'c', 'f')
           AND con.conrelid::bigint = ANY($1::bigint[])
         ORDER BY con.conrelid, con.conname",
    )
    .bind(object_ids.to_vec())
    .fetch_all(pool)
    .await?;
    let mut foreign_keys = Vec::new();
    let mut primary_keys = HashMap::new();
    let mut unique_constraints = HashMap::new();
    let mut check_constraints = HashMap::new();
    for row in rows {
        let id: i64 = row.try_get("constraint_id")?;
        let source: i64 = row.try_get("source_table_id")?;
        let constraint_type = row_text(&row, "constraint_type")?;
        let name = identifier(row_text(&row, "name")?);
        let source_ordinals = row_optional_text(&row, "source_ordinals")?.unwrap_or_default();
        let source_column_ids = column_ids(source, &source_ordinals);
        if constraint_type == "f" {
            let target: i64 = row.try_get("target_table_id")?;
            let target_ordinals = row_optional_text(&row, "target_ordinals")?.unwrap_or_default();
            foreign_keys.push(ForeignKeyMetadata {
                id: format!("foreign-key:{id}"),
                name,
                source_table_id: object_id(source),
                source_column_ids,
                target_table_id: object_id(target),
                target_column_ids: column_ids(target, &target_ordinals),
            });
        } else if constraint_type == "p" {
            primary_keys
                .entry(source)
                .or_insert_with(Vec::new)
                .push(PrimaryKeyMetadata {
                    id: format!("primary-key:{id}"),
                    name,
                    table_id: object_id(source),
                    column_ids: source_column_ids,
                });
        } else if constraint_type == "u" {
            unique_constraints
                .entry(source)
                .or_insert_with(Vec::new)
                .push(UniqueConstraintMetadata {
                    id: format!("unique:{id}"),
                    name,
                    table_id: object_id(source),
                    column_ids: source_column_ids,
                });
        } else {
            check_constraints
                .entry(source)
                .or_insert_with(Vec::new)
                .push(CheckConstraintMetadata {
                    id: format!("check:{id}"),
                    name,
                    table_id: object_id(source),
                    expression: row_optional_text(&row, "definition")?,
                });
        }
    }
    Ok((
        foreign_keys,
        primary_keys,
        unique_constraints,
        check_constraints,
    ))
}

async fn fetch_routines(
    pool: &sqlx::PgPool,
    schema_oid: Option<i64>,
) -> ProviderResult<(Vec<CatalogFunction>, Vec<CatalogProcedure>)> {
    let routine_rows = sqlx::query(
        "SELECT n.oid::bigint AS schema_id, r.routine_schema, r.specific_name,
                r.routine_name, r.routine_type, r.data_type
         FROM information_schema.routines r
         JOIN pg_namespace n ON n.nspname = r.routine_schema
         WHERE r.routine_schema NOT IN ('information_schema', 'pg_catalog')
           AND ($1::bigint IS NULL OR n.oid::bigint = $1)
         ORDER BY r.routine_schema, r.routine_name, r.specific_name",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    let parameter_rows = sqlx::query(
        "SELECT specific_schema, specific_name, ordinal_position, parameter_name,
                data_type, parameter_mode
         FROM information_schema.parameters
         WHERE specific_schema NOT IN ('information_schema', 'pg_catalog')
           AND ($1::bigint IS NULL OR specific_schema = (SELECT nspname FROM pg_namespace WHERE oid::bigint = $1))
         ORDER BY specific_schema, specific_name, ordinal_position",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    let mut parameters_by_routine: HashMap<(String, String), Vec<CatalogParameter>> =
        HashMap::new();
    for row in parameter_rows {
        let schema = row_text(&row, "specific_schema")?;
        let specific = row_text(&row, "specific_name")?;
        let ordinal: i32 = row.try_get("ordinal_position")?;
        let mode = row_optional_text(&row, "parameter_mode")?.unwrap_or_else(|| "IN".to_owned());
        parameters_by_routine
            .entry((schema, specific.clone()))
            .or_default()
            .push(CatalogParameter {
                id: format!("parameter:{specific}:{ordinal}"),
                name: identifier(row_optional_text(&row, "parameter_name")?.unwrap_or_default()),
                r#type: type_ref(
                    row_optional_text(&row, "data_type")?
                        .as_deref()
                        .unwrap_or("unknown"),
                    0,
                    0,
                ),
                direction: match mode.as_str() {
                    "OUT" => "out".to_owned(),
                    "INOUT" => "inout".to_owned(),
                    _ => "in".to_owned(),
                },
                ordinal: u32::try_from(ordinal)?,
            });
    }
    let mut functions = Vec::new();
    let mut procedures = Vec::new();
    for row in routine_rows {
        let schema = row_text(&row, "routine_schema")?;
        let schema_oid: i64 = row.try_get("schema_id")?;
        let specific = row_text(&row, "specific_name")?;
        let name = row_text(&row, "routine_name")?;
        let routine_type = row_text(&row, "routine_type")?;
        let parameters = parameters_by_routine
            .remove(&(schema.clone(), specific.clone()))
            .unwrap_or_default();
        let return_type_name =
            row_optional_text(&row, "data_type")?.unwrap_or_else(|| "unknown".to_owned());
        let function = CatalogFunction {
            id: format!("function:{schema}:{specific}"),
            schema_id: Some(schema_id(schema_oid)),
            name: identifier(name.clone()),
            parameters,
            return_type: type_ref(&return_type_name, 0, 0),
            variadic: false,
            overloads: vec![specific],
            kind: Some("function".to_owned()),
        };
        if routine_type == "FUNCTION" {
            functions.push(function);
        } else {
            procedures.push(CatalogProcedure {
                id: format!("procedure:{schema}:{name}"),
                schema_id: Some(schema_id(schema_oid)),
                name: identifier(name),
                parameters: function.parameters,
            });
        }
    }
    let aggregate_rows = sqlx::query(
        "SELECT n.oid::bigint AS schema_id, n.nspname AS routine_schema,
                p.oid::bigint AS routine_id, p.proname AS routine_name,
                pg_get_function_result(p.oid) AS data_type
         FROM pg_proc p
         JOIN pg_namespace n ON n.oid = p.pronamespace
         JOIN pg_aggregate a ON a.aggfnoid = p.oid
         WHERE n.nspname NOT IN ('information_schema', 'pg_catalog')
           AND ($1::bigint IS NULL OR n.oid::bigint = $1)
         ORDER BY n.nspname, p.proname, p.oid",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    for row in aggregate_rows {
        let schema: i64 = row.try_get("schema_id")?;
        let routine_id: i64 = row.try_get("routine_id")?;
        let name = row_text(&row, "routine_name")?;
        let return_type_name =
            row_optional_text(&row, "data_type")?.unwrap_or_else(|| "unknown".to_owned());
        functions.push(CatalogFunction {
            id: format!("aggregate:{schema}:{routine_id}"),
            schema_id: Some(schema_id(schema)),
            name: identifier(name),
            parameters: Vec::new(),
            return_type: type_ref(&return_type_name, 0, 0),
            variadic: false,
            overloads: vec![routine_id.to_string()],
            kind: Some("aggregate".to_owned()),
        });
    }
    Ok((functions, procedures))
}

async fn fetch_sequences(
    pool: &sqlx::PgPool,
    schema_oid: Option<i64>,
) -> ProviderResult<Vec<CatalogSequence>> {
    let rows = sqlx::query(
        "SELECT n.oid::bigint AS schema_id, s.sequence_schema, s.sequence_name, s.data_type
         FROM information_schema.sequences s
         JOIN pg_namespace n ON n.nspname = s.sequence_schema
         WHERE ($1::bigint IS NULL OR n.oid::bigint = $1)
         ORDER BY s.sequence_schema, s.sequence_name",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let schema = row_text(&row, "sequence_schema")?;
            let schema_oid: i64 = row.try_get("schema_id")?;
            let name = row_text(&row, "sequence_name")?;
            Ok(CatalogSequence {
                id: format!("sequence:{schema}:{name}"),
                schema_id: schema_id(schema_oid),
                name: identifier(name),
                r#type: type_ref(&row_text(&row, "data_type")?, 0, 0),
            })
        })
        .collect()
}

async fn fetch_types(
    pool: &sqlx::PgPool,
    schema_oid: Option<i64>,
) -> ProviderResult<Vec<CatalogType>> {
    let rows = sqlx::query(
        "SELECT t.oid::bigint AS type_id, n.oid::bigint AS schema_id, t.typname AS name,
                t.typtype::text AS type_kind, format_type(t.oid, NULL) AS definition
         FROM pg_type t
         JOIN pg_namespace n ON n.oid = t.typnamespace
         WHERE t.typtype IN ('b', 'd', 'e', 'c')
           AND n.nspname NOT IN ('pg_catalog', 'information_schema')
           AND ($1::bigint IS NULL OR n.oid::bigint = $1)
         ORDER BY n.nspname, t.typname",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("type_id")?;
            let schema: i64 = row.try_get("schema_id")?;
            let definition = row_text(&row, "definition")?;
            let family = match row_text(&row, "type_kind")?.as_str() {
                "e" => crate::backend::domain::catalog::SqlTypeFamily::Enum,
                "d" | "c" => crate::backend::domain::catalog::SqlTypeFamily::Custom,
                _ => type_ref(&definition, 0, 0).family,
            };
            Ok(CatalogType {
                id: format!("type:{id}"),
                schema_id: Some(schema_id(schema)),
                name: identifier(row_text(&row, "name")?),
                family,
                definition: Some(definition),
            })
        })
        .collect()
}

async fn fetch_triggers(
    pool: &sqlx::PgPool,
    schema_oid: Option<i64>,
) -> ProviderResult<Vec<CatalogTrigger>> {
    let rows = sqlx::query(
        "SELECT t.oid::bigint AS trigger_id, n.oid::bigint AS schema_id, c.oid::bigint AS table_id,
                t.tgname AS name, t.tgenabled <> 'D' AS enabled, pg_get_triggerdef(t.oid) AS definition
         FROM pg_trigger t
         JOIN pg_class c ON c.oid = t.tgrelid
         JOIN pg_namespace n ON n.oid = c.relnamespace
         WHERE NOT t.tgisinternal
           AND ($1::bigint IS NULL OR n.oid::bigint = $1)
         ORDER BY n.nspname, c.relname, t.tgname",
    )
    .bind(schema_oid)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let trigger_id: i64 = row.try_get("trigger_id")?;
            let schema: i64 = row.try_get("schema_id")?;
            let table: i64 = row.try_get("table_id")?;
            Ok(CatalogTrigger {
                id: format!("trigger:{trigger_id}"),
                schema_id: Some(schema_id(schema)),
                table_id: Some(object_id(table)),
                name: identifier(row_text(&row, "name")?),
                enabled: row.try_get("enabled")?,
                definition: row_optional_text(&row, "definition")?,
            })
        })
        .collect()
}

async fn fetch_extensions(pool: &sqlx::PgPool) -> ProviderResult<Vec<CatalogExtension>> {
    let rows = sqlx::query("SELECT extname, extversion FROM pg_extension ORDER BY extname")
        .fetch_all(pool)
        .await?;
    rows.into_iter()
        .map(|row| {
            let name = row_text(&row, "extname")?;
            Ok(CatalogExtension {
                id: format!("extension:{name}"),
                name: identifier(name),
                version: row_optional_text(&row, "extversion")?,
            })
        })
        .collect()
}

async fn fetch_partitions(
    pool: &sqlx::PgPool,
    table_ids: Option<Vec<i64>>,
) -> ProviderResult<Vec<CatalogPartition>> {
    let rows = sqlx::query(
        "SELECT child.oid::bigint AS child_id, child.relname AS child_name,
                parent.oid::bigint AS parent_id,
                pg_get_expr(child.relpartbound, child.oid) AS bound
         FROM pg_inherits inheritance
         JOIN pg_class child ON child.oid = inheritance.inhrelid
         JOIN pg_class parent ON parent.oid = inheritance.inhparent
         WHERE ($1::bigint[] IS NULL OR child.oid::bigint = ANY($1))
         ORDER BY parent.relname, child.relname",
    )
    .bind(table_ids)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let child_id: i64 = row.try_get("child_id")?;
            let parent_id: i64 = row.try_get("parent_id")?;
            Ok(CatalogPartition {
                id: format!("partition:{child_id}"),
                table_id: object_id(child_id),
                name: Some(identifier(row_text(&row, "child_name")?)),
                parent_id: Some(object_id(parent_id)),
                bound: row_optional_text(&row, "bound")?,
            })
        })
        .collect()
}

async fn fetch_principals(pool: &sqlx::PgPool) -> ProviderResult<Vec<CatalogPrincipal>> {
    let rows = sqlx::query(
        "SELECT oid::bigint AS principal_id, rolname, rolcanlogin FROM pg_roles ORDER BY rolname",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let id: i64 = row.try_get("principal_id")?;
            Ok(CatalogPrincipal {
                id: format!("principal:{id}"),
                name: identifier(row_text(&row, "rolname")?),
                kind: "role".to_owned(),
                login: row
                    .try_get::<bool, _>("rolcanlogin")
                    .ok()
                    .map(|value| value.to_string()),
            })
        })
        .collect()
}

async fn fetch_permissions(pool: &sqlx::PgPool) -> ProviderResult<Vec<CatalogPermission>> {
    let rows = sqlx::query(
        "SELECT grants.grantee, grants.table_schema, grants.table_name, grants.privilege_type,
                grants.is_grantable, c.oid::bigint AS table_id
         FROM information_schema.role_table_grants grants
         JOIN pg_namespace n ON n.nspname = grants.table_schema
         JOIN pg_class c ON c.relnamespace = n.oid AND c.relname = grants.table_name
         ORDER BY grants.grantee, grants.table_schema, grants.table_name, grants.privilege_type",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let grantee = row_text(&row, "grantee")?;
            let table_id: i64 = row.try_get("table_id")?;
            Ok(CatalogPermission {
                id: format!("permission:{index}"),
                principal_id: format!("principal:{grantee}"),
                object_id: Some(object_id(table_id)),
                permission: row_text(&row, "privilege_type")?,
                state: row_text(&row, "is_grantable")?,
            })
        })
        .collect()
}

async fn connect_pool(
    request: &CatalogRefreshRequest,
    endpoint: &crate::backend::adapters::transport::PreparedEndpoint,
) -> ProviderResult<sqlx::PgPool> {
    let ssl_mode = match request.driver_properties.get("sslMode").map(String::as_str) {
        Some("disable") => PgSslMode::Disable,
        Some("require") => PgSslMode::Require,
        Some("verifyCa") => PgSslMode::VerifyCa,
        Some("verifyFull") => PgSslMode::VerifyFull,
        _ if request.trust_server_certificate => PgSslMode::Require,
        _ => PgSslMode::Prefer,
    };
    let mut options = PgConnectOptions::new()
        .host(&endpoint.host)
        .port(endpoint.port)
        .username(&request.user_name)
        .password(&request.password)
        .ssl_mode(ssl_mode);
    if let Some(application_name) = request.driver_properties.get("applicationName")
        && !application_name.trim().is_empty()
    {
        options = options.application_name(application_name);
    }
    if let Some(capacity) = request
        .driver_properties
        .get("statementCacheCapacity")
        .and_then(|value| value.parse().ok())
    {
        options = options.statement_cache_capacity(capacity);
    }
    if let Some(timeout) = request
        .driver_properties
        .get("statementTimeoutMilliseconds")
        .and_then(|value| value.parse::<u64>().ok())
        && timeout > 0
    {
        options = options.options([(String::from("statement_timeout"), timeout.to_string())]);
    }
    if let Some(extra_float_digits) = request
        .driver_properties
        .get("extraFloatDigits")
        .and_then(|value| value.parse::<i8>().ok())
    {
        options = options.extra_float_digits(extra_float_digits);
    }
    if let Some(search_path) = request.driver_properties.get("searchPath")
        && !search_path.trim().is_empty()
    {
        options = options.options([(String::from("search_path"), search_path)]);
    }
    let options = if request.database.trim().is_empty() {
        options
    } else {
        options.database(&request.database)
    };
    Ok(PgPoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await?)
}

async fn introspect_provider(
    request: CatalogRefreshRequest,
    pool: &sqlx::PgPool,
) -> ProviderResult<CatalogSnapshot> {
    let (
        schemas,
        objects,
        (functions, procedures),
        sequences,
        types,
        databases,
        triggers,
        extensions,
        partitions,
        principals,
        permissions,
    ) = tokio::try_join!(
        fetch_schemas(pool, &request.database),
        fetch_objects(pool),
        fetch_routines(pool, None),
        fetch_sequences(pool, None),
        fetch_types(pool, None),
        fetch_databases(pool),
        fetch_triggers(pool, None),
        fetch_extensions(pool),
        fetch_partitions(pool, None),
        fetch_principals(pool),
        fetch_permissions(pool),
    )?;
    let object_ids = objects.keys().copied().collect::<Vec<_>>();
    let (
        mut columns_by_table,
        indexes,
        (foreign_keys, mut primary_keys, mut unique_constraints, mut check_constraints),
    ) = tokio::try_join!(
        fetch_columns_for_ids(pool, &object_ids),
        fetch_indexes_for_ids(pool, &object_ids),
        fetch_constraints_for_ids(pool, &object_ids),
    )?;
    for columns in columns_by_table.values_mut() {
        for column in columns {
            column.primary_key = primary_keys.values().any(|keys| {
                keys.iter()
                    .any(|key| key.column_ids.iter().any(|id| id == &column.id))
            });
            column.foreign_key_memberships = foreign_keys
                .iter()
                .filter(|key| key.source_column_ids.iter().any(|id| id == &column.id))
                .map(|key| key.id.clone())
                .collect();
            column.unique_memberships = unique_constraints
                .values()
                .flat_map(|keys| keys.iter())
                .filter(|key| key.column_ids.iter().any(|id| id == &column.id))
                .map(|key| key.id.clone())
                .chain(
                    indexes
                        .iter()
                        .filter(|index| {
                            index.unique && index.column_ids.iter().any(|id| id == &column.id)
                        })
                        .map(|index| index.id.clone()),
                )
                .collect();
        }
    }
    let mut tables = Vec::new();
    let mut views = Vec::new();
    let mut materialized_views = Vec::new();
    for object in objects.values() {
        let columns = columns_by_table.remove(&object.id).unwrap_or_default();
        if object.kind == "v" {
            views.push(CatalogView {
                id: object_id(object.id),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                comment: object.comment.clone(),
            });
        } else if object.kind == "m" {
            materialized_views.push(CatalogView {
                id: object_id(object.id),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                comment: object.comment.clone(),
            });
        } else {
            let table_id = object_id(object.id);
            let primary_key = primary_keys
                .remove(&object.id)
                .and_then(|mut keys| keys.drain(..).next());
            let unique = unique_constraints.remove(&object.id).unwrap_or_default();
            let checks = check_constraints.remove(&object.id).unwrap_or_default();
            let table_indexes = indexes
                .iter()
                .filter(|index| index.table_id == table_id)
                .cloned()
                .collect();
            tables.push(CatalogTable {
                id: table_id.clone(),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                primary_key,
                foreign_keys: foreign_keys
                    .iter()
                    .filter(|key| key.source_table_id == table_id)
                    .cloned()
                    .collect(),
                unique_constraints: unique,
                check_constraints: checks,
                indexes: table_indexes,
                estimated_rows: object.estimated_rows,
                comment: object.comment.clone(),
                foreign: object.kind == "f",
                temporary: false,
            });
        }
    }
    let constraints = tables
        .iter()
        .flat_map(|table| {
            table
                .primary_key
                .iter()
                .cloned()
                .map(CatalogConstraint::PrimaryKey)
                .chain(
                    table
                        .unique_constraints
                        .iter()
                        .cloned()
                        .map(CatalogConstraint::Unique),
                )
                .chain(
                    table
                        .check_constraints
                        .iter()
                        .cloned()
                        .map(CatalogConstraint::Check),
                )
        })
        .collect();
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!("catalog:{}:{}", request.connection_id, request.version),
        connection_id: request.connection_id,
        database_id: request.database.clone(),
        dialect: DialectId::Postgresql,
        version: request.version,
        generated_at: request.generated_at,
        databases,
        schemas,
        tables,
        views,
        materialized_views,
        functions,
        procedures,
        sequences,
        types,
        triggers,
        extensions,
        partitions,
        principals,
        permissions,
        synonyms: Vec::new(),
        foreign_keys,
        indexes,
        constraints,
    })
}

async fn introspect_initial(
    mut request: CatalogRefreshRequest,
    pool: &sqlx::PgPool,
) -> ProviderResult<CatalogSnapshot> {
    if request.database.trim().is_empty() {
        let databases = fetch_databases(pool).await?;
        if request.show_all_catalog_items {
            return Ok(CatalogSnapshot {
                schema_version: CATALOG_SCHEMA_VERSION,
                id: format!(
                    "catalog:{}:{}:initial",
                    request.connection_id, request.version
                ),
                connection_id: request.connection_id,
                database_id: String::new(),
                dialect: DialectId::Postgresql,
                version: request.version,
                generated_at: request.generated_at,
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
            });
        }
        request.database = fetch_current_database(pool).await?;
    }
    let show_all_catalog_items = request.show_all_catalog_items;
    let mut snapshot = introspect_provider(request, pool).await?;
    if !show_all_catalog_items {
        let schema_ids = snapshot
            .schemas
            .iter()
            .filter(|schema| schema.name.raw == "public")
            .map(|schema| schema.id.clone())
            .collect::<HashSet<_>>();
        let table_ids = snapshot
            .tables
            .iter()
            .filter(|table| schema_ids.contains(&table.schema_id))
            .map(|table| table.id.clone())
            .collect::<HashSet<_>>();
        snapshot
            .schemas
            .retain(|schema| schema_ids.contains(&schema.id));
        snapshot
            .tables
            .retain(|table| schema_ids.contains(&table.schema_id));
        snapshot
            .views
            .retain(|view| schema_ids.contains(&view.schema_id));
        snapshot
            .materialized_views
            .retain(|view| schema_ids.contains(&view.schema_id));
        snapshot.functions.retain(|function| {
            function
                .schema_id
                .as_ref()
                .is_none_or(|id| schema_ids.contains(id))
        });
        snapshot.procedures.retain(|procedure| {
            procedure
                .schema_id
                .as_ref()
                .is_none_or(|id| schema_ids.contains(id))
        });
        snapshot
            .sequences
            .retain(|sequence| schema_ids.contains(&sequence.schema_id));
        snapshot.types.retain(|data_type| {
            data_type
                .schema_id
                .as_ref()
                .is_none_or(|id| schema_ids.contains(id))
        });
        snapshot.triggers.retain(|trigger| {
            trigger
                .schema_id
                .as_ref()
                .is_none_or(|id| schema_ids.contains(id))
        });
        snapshot
            .indexes
            .retain(|index| table_ids.contains(&index.table_id));
        snapshot
            .foreign_keys
            .retain(|key| table_ids.contains(&key.source_table_id));
        snapshot
            .partitions
            .retain(|partition| table_ids.contains(&partition.table_id));
        snapshot.constraints.retain(|constraint| match constraint {
            CatalogConstraint::PrimaryKey(value) => table_ids.contains(&value.table_id),
            CatalogConstraint::Unique(value) => table_ids.contains(&value.table_id),
            CatalogConstraint::Check(value) => table_ids.contains(&value.table_id),
        });
        snapshot.permissions.retain(|permission| {
            permission
                .object_id
                .as_ref()
                .is_none_or(|id| table_ids.contains(id))
        });
    }
    Ok(snapshot)
}

async fn introspect_database(
    request: CatalogRefreshRequest,
    database: String,
    pool: &sqlx::PgPool,
) -> ProviderResult<CatalogSnapshot> {
    introspect_provider(request_for_database(request, &database), pool).await
}

async fn introspect_schema(
    request: CatalogRefreshRequest,
    requested_schema_id: String,
    pool: &sqlx::PgPool,
) -> ProviderResult<CatalogSnapshot> {
    let schema_oid = requested_schema_id
        .strip_prefix("schema:")
        .ok_or_else(|| "Invalid PostgreSQL schema identifier".to_owned())?
        .parse::<i64>()
        .map_err(|error| format!("Invalid PostgreSQL schema identifier: {error}"))?;
    let objects = fetch_objects_in_schema(pool, Some(schema_oid)).await?;
    let object_ids = objects.keys().copied().collect::<Vec<_>>();
    let (
        mut columns_by_table,
        indexes,
        (foreign_keys, mut primary_keys, mut unique_constraints, mut check_constraints),
        (functions, procedures),
        sequences,
        types,
        triggers,
        partitions,
    ) = tokio::try_join!(
        fetch_columns_for_ids(pool, &object_ids),
        fetch_indexes_for_ids(pool, &object_ids),
        fetch_constraints_for_ids(pool, &object_ids),
        fetch_routines(pool, Some(schema_oid)),
        fetch_sequences(pool, Some(schema_oid)),
        fetch_types(pool, Some(schema_oid)),
        fetch_triggers(pool, Some(schema_oid)),
        fetch_partitions(pool, Some(object_ids.clone())),
    )?;
    for columns in columns_by_table.values_mut() {
        for column in columns {
            column.primary_key = primary_keys.values().any(|keys| {
                keys.iter()
                    .any(|key| key.column_ids.iter().any(|id| id == &column.id))
            });
            column.foreign_key_memberships = foreign_keys
                .iter()
                .filter(|key| key.source_column_ids.iter().any(|id| id == &column.id))
                .map(|key| key.id.clone())
                .collect();
            column.unique_memberships = unique_constraints
                .values()
                .flat_map(|keys| keys.iter())
                .filter(|key| key.column_ids.iter().any(|id| id == &column.id))
                .map(|key| key.id.clone())
                .chain(
                    indexes
                        .iter()
                        .filter(|index| {
                            index.unique && index.column_ids.iter().any(|id| id == &column.id)
                        })
                        .map(|index| index.id.clone()),
                )
                .collect();
        }
    }
    let mut tables = Vec::new();
    let mut views = Vec::new();
    let mut materialized_views = Vec::new();
    for object in objects.values() {
        let columns = columns_by_table.remove(&object.id).unwrap_or_default();
        if object.kind == "v" {
            views.push(CatalogView {
                id: object_id(object.id),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                comment: object.comment.clone(),
            });
        } else if object.kind == "m" {
            materialized_views.push(CatalogView {
                id: object_id(object.id),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                comment: object.comment.clone(),
            });
        } else {
            let table_id = object_id(object.id);
            let primary_key = primary_keys
                .remove(&object.id)
                .and_then(|mut keys| keys.drain(..).next());
            let unique = unique_constraints.remove(&object.id).unwrap_or_default();
            let checks = check_constraints.remove(&object.id).unwrap_or_default();
            let table_indexes = indexes
                .iter()
                .filter(|index| index.table_id == table_id)
                .cloned()
                .collect();
            tables.push(CatalogTable {
                id: table_id.clone(),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                primary_key,
                foreign_keys: foreign_keys
                    .iter()
                    .filter(|key| key.source_table_id == table_id)
                    .cloned()
                    .collect(),
                unique_constraints: unique,
                check_constraints: checks,
                indexes: table_indexes,
                estimated_rows: object.estimated_rows,
                comment: object.comment.clone(),
                foreign: object.kind == "f",
                temporary: false,
            });
        }
    }
    let constraints = tables
        .iter()
        .flat_map(|table| {
            table
                .primary_key
                .iter()
                .cloned()
                .map(CatalogConstraint::PrimaryKey)
                .chain(
                    table
                        .unique_constraints
                        .iter()
                        .cloned()
                        .map(CatalogConstraint::Unique),
                )
                .chain(
                    table
                        .check_constraints
                        .iter()
                        .cloned()
                        .map(CatalogConstraint::Check),
                )
        })
        .collect();
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:schema:{}",
            request.connection_id, request.version, schema_oid
        ),
        connection_id: request.connection_id,
        database_id: request.database,
        dialect: DialectId::Postgresql,
        version: request.version,
        generated_at: request.generated_at,
        databases: Vec::new(),
        schemas: Vec::new(),
        tables,
        views,
        materialized_views,
        functions,
        procedures,
        sequences,
        types,
        triggers,
        extensions: Vec::new(),
        partitions,
        principals: Vec::new(),
        permissions: Vec::new(),
        synonyms: Vec::new(),
        foreign_keys,
        indexes,
        constraints,
    })
}

async fn introspect_table(
    request: CatalogRefreshRequest,
    requested_table_id: String,
    pool: &sqlx::PgPool,
) -> ProviderResult<CatalogSnapshot> {
    let table_oid = requested_table_id
        .strip_prefix("object:")
        .ok_or_else(|| "Invalid PostgreSQL table identifier".to_owned())?
        .parse::<i64>()
        .map_err(|error| format!("Invalid PostgreSQL table identifier: {error}"))?;
    let mut object_ids = vec![table_oid];
    object_ids.extend(fetch_foreign_key_target_ids(pool, table_oid).await?);
    object_ids.sort_unstable();
    object_ids.dedup();
    let objects = fetch_objects_for_ids(pool, &object_ids).await?;
    let Some(table) = objects.get(&table_oid) else {
        return Err("PostgreSQL table was not found".into());
    };
    if !matches!(table.kind.as_str(), "r" | "p") {
        return Err("PostgreSQL catalog object is not a table".into());
    }
    let mut columns_by_table = fetch_columns_for_ids(pool, &object_ids).await?;
    let indexes = fetch_indexes_for_ids(pool, &[table_oid]).await?;
    let (foreign_keys, mut primary_keys, mut unique_constraints, mut check_constraints) =
        fetch_constraints_for_ids(pool, &[table_oid]).await?;
    for columns in columns_by_table.values_mut() {
        for column in columns {
            column.primary_key = primary_keys.values().any(|keys| {
                keys.iter()
                    .any(|key| key.column_ids.iter().any(|id| id == &column.id))
            });
            column.foreign_key_memberships = foreign_keys
                .iter()
                .filter(|key| key.source_column_ids.iter().any(|id| id == &column.id))
                .map(|key| key.id.clone())
                .collect();
            column.unique_memberships = unique_constraints
                .values()
                .flat_map(|keys| keys.iter())
                .filter(|key| key.column_ids.iter().any(|id| id == &column.id))
                .map(|key| key.id.clone())
                .chain(
                    indexes
                        .iter()
                        .filter(|index| {
                            index.unique && index.column_ids.iter().any(|id| id == &column.id)
                        })
                        .map(|index| index.id.clone()),
                )
                .collect();
        }
    }
    let table_id = object_id(table_oid);
    let table_columns = columns_by_table.remove(&table_oid).unwrap_or_default();
    let primary_key = primary_keys
        .remove(&table_oid)
        .and_then(|mut keys| keys.drain(..).next());
    let unique = unique_constraints.remove(&table_oid).unwrap_or_default();
    let checks = check_constraints.remove(&table_oid).unwrap_or_default();
    let constraints = primary_key
        .iter()
        .cloned()
        .map(CatalogConstraint::PrimaryKey)
        .chain(unique.iter().cloned().map(CatalogConstraint::Unique))
        .chain(checks.iter().cloned().map(CatalogConstraint::Check))
        .collect();
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!(
            "catalog:{}:{}:table",
            request.connection_id, request.version
        ),
        connection_id: request.connection_id,
        database_id: request.database.clone(),
        dialect: DialectId::Postgresql,
        version: request.version,
        generated_at: request.generated_at,
        databases: Vec::new(),
        schemas: Vec::new(),
        tables: vec![CatalogTable {
            id: table_id.clone(),
            catalog_id: Some(request.database.clone()),
            database_id: request.database,
            schema_id: table.schema_id.clone(),
            name: identifier(&table.name),
            columns: table_columns,
            primary_key,
            foreign_keys: foreign_keys.clone(),
            unique_constraints: unique,
            check_constraints: checks,
            indexes: indexes.clone(),
            estimated_rows: table.estimated_rows,
            comment: table.comment.clone(),
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

fn request_for_database(
    mut request: CatalogRefreshRequest,
    database: &str,
) -> CatalogRefreshRequest {
    request.database = database.to_owned();
    request
}

fn request_with_optional_database(
    request: CatalogRefreshRequest,
    database: Option<String>,
) -> CatalogRefreshRequest {
    match database {
        Some(database) => request_for_database(request, &database),
        None => request,
    }
}

#[derive(Clone, Default)]
pub struct PostgresqlAdapter {
    pools: Arc<Mutex<HashMap<String, sqlx::PgPool>>>,
    transport_guards: Arc<Mutex<HashMap<String, Arc<TransportGuard>>>>,
}

impl PostgresqlAdapter {
    fn pool_key(request: &CatalogRefreshRequest) -> String {
        format!("{}\0{}", request.connection_id, request.database)
    }

    async fn pool_for(&self, request: &CatalogRefreshRequest) -> ProviderResult<sqlx::PgPool> {
        let pool_key = Self::pool_key(request);
        if let Some(pool) = self
            .pools
            .lock()
            .expect("PostgreSQL catalog pool cache poisoned")
            .get(&pool_key)
            .cloned()
        {
            return Ok(pool);
        }
        let endpoint = prepare_endpoint_for(&request.server, request.port, &request.transport)
            .await
            .map_err(|error| -> Box<dyn std::error::Error + Send + Sync> {
                error.to_string().into()
            })?;
        let guard = endpoint.guard.clone();
        let pool = connect_pool(request, &endpoint).await?;
        let mut pools = self
            .pools
            .lock()
            .expect("PostgreSQL catalog pool cache poisoned");
        let pool = pools
            .entry(pool_key.clone())
            .or_insert_with(|| pool.clone())
            .clone();
        if let Some(guard) = guard {
            self.transport_guards
                .lock()
                .expect("PostgreSQL transport cache poisoned")
                .insert(pool_key, guard);
        }
        Ok(pool)
    }
}

impl CatalogProvider for PostgresqlAdapter {
    fn dialect(&self) -> DialectId {
        DialectId::Postgresql
    }

    fn invalidate(&self, connection_id: &str) {
        let prefix = format!("{connection_id}\0");
        self.pools
            .lock()
            .expect("PostgreSQL catalog pool cache poisoned")
            .retain(|key, _| !key.starts_with(&prefix));
        self.transport_guards
            .lock()
            .expect("PostgreSQL transport cache poisoned")
            .retain(|key, _| !key.starts_with(&prefix));
    }

    fn refresh(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        let adapter = self.clone();
        Box::pin(async move {
            let pool =
                adapter
                    .pool_for(&request)
                    .await
                    .map_err(|error| CatalogProviderError::Driver {
                        dialect: DialectId::Postgresql,
                        message: error.to_string(),
                    })?;
            introspect_provider(request, &pool)
                .await
                .map_err(|message| CatalogProviderError::Driver {
                    dialect: DialectId::Postgresql,
                    message: message.to_string(),
                })
        })
    }

    fn initial(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        let adapter = self.clone();
        Box::pin(async move {
            let pool =
                adapter
                    .pool_for(&request)
                    .await
                    .map_err(|error| CatalogProviderError::Driver {
                        dialect: DialectId::Postgresql,
                        message: error.to_string(),
                    })?;
            introspect_initial(request, &pool).await.map_err(|message| {
                CatalogProviderError::Driver {
                    dialect: DialectId::Postgresql,
                    message: message.to_string(),
                }
            })
        })
    }

    fn expand(&self, request: CatalogRefreshRequest, scope: CatalogScope) -> CatalogProviderFuture {
        let adapter = self.clone();
        Box::pin(async move {
            match scope {
                CatalogScope::Database { database_id } => {
                    let request = request_for_database(request, &database_id);
                    let pool = adapter.pool_for(&request).await.map_err(|error| {
                        CatalogProviderError::Driver {
                            dialect: DialectId::Postgresql,
                            message: error.to_string(),
                        }
                    })?;
                    introspect_database(request, database_id, &pool)
                        .await
                        .map_err(|message| CatalogProviderError::Driver {
                            dialect: DialectId::Postgresql,
                            message: message.to_string(),
                        })
                }
                CatalogScope::Schema {
                    schema_id,
                    database_id,
                } => {
                    let request = request_with_optional_database(request, database_id);
                    let pool = adapter.pool_for(&request).await.map_err(|error| {
                        CatalogProviderError::Driver {
                            dialect: DialectId::Postgresql,
                            message: error.to_string(),
                        }
                    })?;
                    introspect_schema(request, schema_id, &pool)
                        .await
                        .map_err(|message| CatalogProviderError::Driver {
                            dialect: DialectId::Postgresql,
                            message: message.to_string(),
                        })
                }
                CatalogScope::Table {
                    table_id,
                    database_id,
                } => {
                    let request = request_with_optional_database(request, database_id);
                    let pool = adapter.pool_for(&request).await.map_err(|error| {
                        CatalogProviderError::Driver {
                            dialect: DialectId::Postgresql,
                            message: error.to_string(),
                        }
                    })?;
                    introspect_table(request, table_id, &pool)
                        .await
                        .map_err(|message| CatalogProviderError::Driver {
                            dialect: DialectId::Postgresql,
                            message: message.to_string(),
                        })
                }
            }
        })
    }
}
