use crate::backend::adapters::type_mapping::{identifier, type_ref};
use crate::backend::domain::catalog::{
    CATALOG_SCHEMA_VERSION, CatalogConstraint, CatalogFunction, CatalogParameter, CatalogProcedure,
    CatalogSchema, CatalogSequence, CatalogSnapshot, CatalogTable, CatalogType, CatalogView,
    CheckConstraintMetadata, ColumnMetadata, DialectId, ForeignKeyMetadata, IndexMetadata,
    PrimaryKeyMetadata, UniqueConstraintMetadata,
};
use crate::backend::ports::database::{CatalogRefreshRequest, ProviderResult};
use crate::backend::ports::{CatalogProvider, CatalogProviderError, CatalogProviderFuture};
use sqlx::{
    Row,
    mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlRow, MySqlSslMode},
};
use std::collections::HashMap;

#[derive(Clone)]
struct MySqlObject {
    id: String,
    schema_id: String,
    name: String,
    kind: String,
    comment: Option<String>,
    estimated_rows: Option<u64>,
}

type ConstraintGroupKey = (String, String, String);
type ConstraintMember = (u32, String, Option<String>);

fn schema_key(name: &str) -> String {
    format!("schema:{name}")
}

fn object_key(schema: &str, name: &str) -> String {
    format!("object:{schema}:{name}")
}

fn column_key(object_id: &str, ordinal: u32) -> String {
    format!("column:{object_id}:{ordinal}")
}

fn row_text(row: &MySqlRow, column: &str) -> ProviderResult<String> {
    Ok(row.try_get(column)?)
}

fn row_optional_text(row: &MySqlRow, column: &str) -> ProviderResult<Option<String>> {
    Ok(row.try_get(column)?)
}

fn row_u32(row: &MySqlRow, column: &str) -> ProviderResult<u32> {
    let value: i64 = row.try_get(column)?;
    Ok(u32::try_from(value)?)
}

async fn fetch_schemas(
    pool: &sqlx::MySqlPool,
    database_id: &str,
) -> ProviderResult<Vec<CatalogSchema>> {
    let rows = sqlx::query(
        "SELECT SCHEMA_NAME AS schema_name
         FROM information_schema.SCHEMATA
         WHERE SCHEMA_NAME = ?
         ORDER BY SCHEMA_NAME",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let name = row_text(&row, "schema_name")?;
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
    pool: &sqlx::MySqlPool,
    database_id: &str,
) -> ProviderResult<HashMap<String, MySqlObject>> {
    let rows = sqlx::query(
        "SELECT TABLE_SCHEMA AS schema_name, TABLE_NAME AS table_name, TABLE_TYPE AS table_type,
                TABLE_ROWS AS estimated_rows, TABLE_COMMENT AS comment
         FROM information_schema.TABLES
         WHERE TABLE_SCHEMA = ?
         ORDER BY TABLE_SCHEMA, TABLE_NAME",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let schema = row_text(&row, "schema_name")?;
            let name = row_text(&row, "table_name")?;
            let kind = row_text(&row, "table_type")?;
            let id = object_key(&schema, &name);
            let estimated_rows = row.try_get::<Option<u64>, _>("estimated_rows")?;
            Ok((
                id.clone(),
                MySqlObject {
                    id,
                    schema_id: schema_key(&schema),
                    name,
                    kind,
                    comment: row_optional_text(&row, "comment")?,
                    estimated_rows,
                },
            ))
        })
        .collect()
}

async fn fetch_columns(
    pool: &sqlx::MySqlPool,
    objects: &HashMap<String, MySqlObject>,
    database_id: &str,
) -> ProviderResult<HashMap<String, Vec<ColumnMetadata>>> {
    let rows = sqlx::query(
        "SELECT TABLE_SCHEMA AS schema_name, TABLE_NAME AS table_name, COLUMN_NAME AS column_name,
                CAST(ORDINAL_POSITION AS SIGNED) AS ordinal, DATA_TYPE AS data_type, COLUMN_TYPE AS column_type,
                IS_NULLABLE AS nullable, EXTRA AS extra, COLUMN_DEFAULT AS default_value,
                GENERATION_EXPRESSION AS generation_expression, COLUMN_COMMENT AS comment
         FROM information_schema.COLUMNS
         WHERE TABLE_SCHEMA = ?
         ORDER BY TABLE_SCHEMA, TABLE_NAME, ORDINAL_POSITION",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await
    ?;
    let mut columns_by_object = HashMap::new();
    for row in rows {
        let schema = row_text(&row, "schema_name")?;
        let name = row_text(&row, "table_name")?;
        let object_id = object_key(&schema, &name);
        objects.get(&object_id).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("column metadata references unknown object {object_id}"),
            )
        })?;
        let ordinal = row_u32(&row, "ordinal")?;
        let extra = row_text(&row, "extra")?.to_ascii_lowercase();
        let generation_expression = row_optional_text(&row, "generation_expression")?;
        let type_name = row_text(&row, "column_type")?;
        columns_by_object
            .entry(object_id.clone())
            .or_insert_with(Vec::new)
            .push(ColumnMetadata {
                id: column_key(&object_id, ordinal),
                table_id: object_id,
                ordinal,
                name: identifier(row_text(&row, "column_name")?),
                r#type: type_ref(&type_name, 0, 0),
                nullable: row_text(&row, "nullable")? == "YES",
                generated: generation_expression
                    .as_deref()
                    .is_some_and(|value| !value.is_empty()),
                identity: extra.contains("auto_increment"),
                computed: extra.contains("generated"),
                default_value: row_optional_text(&row, "default_value")?,
                primary_key: false,
                foreign_key_memberships: Vec::new(),
                unique_memberships: Vec::new(),
                comment: row_optional_text(&row, "comment")?,
            });
    }
    Ok(columns_by_object)
}

async fn fetch_indexes(
    pool: &sqlx::MySqlPool,
    columns_by_object: &HashMap<String, Vec<ColumnMetadata>>,
    database_id: &str,
) -> ProviderResult<Vec<IndexMetadata>> {
    let rows = sqlx::query(
        "SELECT TABLE_SCHEMA AS schema_name, TABLE_NAME AS table_name, INDEX_NAME AS index_name,
                NON_UNIQUE AS non_unique, CAST(SEQ_IN_INDEX AS SIGNED) AS ordinal, COLUMN_NAME AS column_name
         FROM information_schema.STATISTICS
         WHERE TABLE_SCHEMA = ? AND INDEX_NAME IS NOT NULL
         ORDER BY TABLE_SCHEMA, TABLE_NAME, INDEX_NAME, SEQ_IN_INDEX",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await
    ?;
    let mut groups: HashMap<(String, String), Vec<(u32, String)>> = HashMap::new();
    let mut unique_by_group = HashMap::new();
    for row in rows {
        let object_id = object_key(
            &row_text(&row, "schema_name")?,
            &row_text(&row, "table_name")?,
        );
        let index_name = row_text(&row, "index_name")?;
        let ordinal = row_u32(&row, "ordinal")?;
        let column_name = row_text(&row, "column_name")?;
        let key = (object_id, index_name);
        groups
            .entry(key.clone())
            .or_default()
            .push((ordinal, column_name));
        let non_unique: i64 = row.try_get("non_unique")?;
        unique_by_group.insert(key, non_unique == 0);
    }
    groups
        .into_iter()
        .map(|((object_id, name), mut members)| {
            members.sort_by_key(|(ordinal, _)| *ordinal);
            let column_ids = members
                .iter()
                .filter_map(|(_, name)| {
                    columns_by_object
                        .get(&object_id)
                        .and_then(|columns| {
                            columns
                                .iter()
                                .find(|column| column.name.normalized.eq_ignore_ascii_case(name))
                        })
                        .map(|column| column.id.clone())
                })
                .collect();
            let unique = unique_by_group
                .get(&(object_id.clone(), name.clone()))
                .copied()
                .unwrap_or(false);
            Ok(IndexMetadata {
                id: format!("index:{object_id}:{name}"),
                name: identifier(name.clone()),
                table_id: object_id,
                column_ids,
                unique,
                primary: name.eq_ignore_ascii_case("PRIMARY"),
            })
        })
        .collect()
}

async fn fetch_constraints(
    pool: &sqlx::MySqlPool,
    columns_by_object: &HashMap<String, Vec<ColumnMetadata>>,
    database_id: &str,
) -> Result<
    (
        Vec<ForeignKeyMetadata>,
        HashMap<String, Vec<PrimaryKeyMetadata>>,
        HashMap<String, Vec<UniqueConstraintMetadata>>,
        HashMap<String, Vec<CheckConstraintMetadata>>,
    ),
    Box<dyn std::error::Error + Send + Sync>,
> {
    let rows = sqlx::query(
        "SELECT kcu.CONSTRAINT_SCHEMA AS schema_name, kcu.TABLE_NAME AS table_name,
                kcu.CONSTRAINT_NAME AS constraint_name, tc.CONSTRAINT_TYPE AS constraint_type,
                CAST(kcu.ORDINAL_POSITION AS SIGNED) AS ordinal, kcu.COLUMN_NAME AS column_name,
                kcu.REFERENCED_TABLE_SCHEMA AS target_schema,
                kcu.REFERENCED_TABLE_NAME AS target_table,
                kcu.REFERENCED_COLUMN_NAME AS target_column
         FROM information_schema.KEY_COLUMN_USAGE kcu
         JOIN information_schema.TABLE_CONSTRAINTS tc
           ON tc.CONSTRAINT_SCHEMA = kcu.CONSTRAINT_SCHEMA
          AND tc.TABLE_NAME = kcu.TABLE_NAME
          AND tc.CONSTRAINT_NAME = kcu.CONSTRAINT_NAME
         WHERE tc.CONSTRAINT_SCHEMA = ?
           AND tc.CONSTRAINT_TYPE IN ('PRIMARY KEY', 'UNIQUE', 'FOREIGN KEY')
         ORDER BY kcu.CONSTRAINT_SCHEMA, kcu.TABLE_NAME, kcu.CONSTRAINT_NAME, kcu.ORDINAL_POSITION",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    let mut groups: HashMap<ConstraintGroupKey, Vec<ConstraintMember>> = HashMap::new();
    for row in rows {
        let object_id = object_key(
            &row_text(&row, "schema_name")?,
            &row_text(&row, "table_name")?,
        );
        let constraint_name = row_text(&row, "constraint_name")?;
        let constraint_type = row_text(&row, "constraint_type")?;
        let ordinal = row_u32(&row, "ordinal")?;
        let column_name = row_text(&row, "column_name")?;
        let target = match (
            row_optional_text(&row, "target_schema")?,
            row_optional_text(&row, "target_table")?,
            row_optional_text(&row, "target_column")?,
        ) {
            (Some(schema), Some(table), Some(column)) => {
                Some(format!("{}|{}|{}", schema, table, column))
            }
            _ => None,
        };
        groups
            .entry((object_id, constraint_name, constraint_type))
            .or_default()
            .push((ordinal, column_name, target));
    }
    let mut foreign_keys = Vec::new();
    let mut primary_keys = HashMap::new();
    let mut unique_constraints = HashMap::new();
    for ((object_id, name, kind), mut members) in groups {
        members.sort_by_key(|(ordinal, _, _)| *ordinal);
        let source_columns = members
            .iter()
            .filter_map(|(_, name, _)| {
                columns_by_object
                    .get(&object_id)
                    .and_then(|columns| {
                        columns
                            .iter()
                            .find(|column| column.name.normalized.eq_ignore_ascii_case(name))
                    })
                    .map(|column| column.id.clone())
            })
            .collect::<Vec<_>>();
        if kind == "FOREIGN KEY" {
            let targets = members
                .iter()
                .filter_map(|(_, _, target)| target.as_deref())
                .filter_map(|target| {
                    let mut parts = target.split('|');
                    Some((
                        object_key(parts.next()?, parts.next()?),
                        parts.next()?.to_owned(),
                    ))
                })
                .collect::<Vec<_>>();
            let (target_table_id, _) = targets.first().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("foreign key {name} has no target metadata"),
                )
            })?;
            let target_column_ids = targets
                .iter()
                .filter_map(|(table_id, column_name)| {
                    columns_by_object
                        .get(table_id)
                        .and_then(|columns| {
                            columns.iter().find(|column| {
                                column.name.normalized.eq_ignore_ascii_case(column_name)
                            })
                        })
                        .map(|column| column.id.clone())
                })
                .collect();
            foreign_keys.push(ForeignKeyMetadata {
                id: format!("foreign-key:{object_id}:{name}"),
                name: identifier(name),
                source_table_id: object_id,
                source_column_ids: source_columns,
                target_table_id: target_table_id.clone(),
                target_column_ids,
            });
        } else if kind == "PRIMARY KEY" {
            primary_keys
                .entry(object_id.clone())
                .or_insert_with(Vec::new)
                .push(PrimaryKeyMetadata {
                    id: format!("primary-key:{object_id}:{name}"),
                    name: identifier(name),
                    table_id: object_id,
                    column_ids: source_columns,
                });
        } else {
            unique_constraints
                .entry(object_id.clone())
                .or_insert_with(Vec::new)
                .push(UniqueConstraintMetadata {
                    id: format!("unique:{object_id}:{name}"),
                    name: identifier(name),
                    table_id: object_id,
                    column_ids: source_columns,
                });
        }
    }
    let check_rows = sqlx::query(
        "SELECT tc.CONSTRAINT_SCHEMA AS schema_name, tc.TABLE_NAME AS table_name,
                tc.CONSTRAINT_NAME AS constraint_name, cc.CHECK_CLAUSE AS expression
         FROM information_schema.TABLE_CONSTRAINTS tc
         JOIN information_schema.CHECK_CONSTRAINTS cc
           ON cc.CONSTRAINT_SCHEMA = tc.CONSTRAINT_SCHEMA
          AND cc.CONSTRAINT_NAME = tc.CONSTRAINT_NAME
         WHERE tc.CONSTRAINT_SCHEMA = ? AND tc.CONSTRAINT_TYPE = 'CHECK'",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    let mut checks = HashMap::new();
    for row in check_rows {
        let object_id = object_key(
            &row_text(&row, "schema_name")?,
            &row_text(&row, "table_name")?,
        );
        let name = row_text(&row, "constraint_name")?;
        checks
            .entry(object_id.clone())
            .or_insert_with(Vec::new)
            .push(CheckConstraintMetadata {
                id: format!("check:{object_id}:{name}"),
                name: identifier(name),
                table_id: object_id,
                expression: row_optional_text(&row, "expression")?,
            });
    }
    Ok((foreign_keys, primary_keys, unique_constraints, checks))
}

async fn fetch_routines(
    pool: &sqlx::MySqlPool,
    database_id: &str,
) -> ProviderResult<(Vec<CatalogFunction>, Vec<CatalogProcedure>)> {
    let routine_rows = sqlx::query(
        "SELECT ROUTINE_SCHEMA AS schema_name, SPECIFIC_NAME AS specific_name,
                ROUTINE_NAME AS routine_name, ROUTINE_TYPE AS routine_type, DATA_TYPE AS data_type
         FROM information_schema.ROUTINES
         WHERE ROUTINE_SCHEMA = ?
         ORDER BY ROUTINE_SCHEMA, ROUTINE_NAME, SPECIFIC_NAME",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    let parameter_rows = sqlx::query(
        "SELECT SPECIFIC_SCHEMA AS schema_name, SPECIFIC_NAME AS specific_name,
                CAST(ORDINAL_POSITION AS SIGNED) AS ordinal, PARAMETER_NAME AS parameter_name,
                DATA_TYPE AS data_type, PARAMETER_MODE AS parameter_mode
         FROM information_schema.PARAMETERS
         WHERE SPECIFIC_SCHEMA = ? AND ORDINAL_POSITION IS NOT NULL
         ORDER BY SPECIFIC_SCHEMA, SPECIFIC_NAME, ORDINAL_POSITION",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    let mut parameters = HashMap::new();
    for row in parameter_rows {
        let schema = row_text(&row, "schema_name")?;
        let specific = row_text(&row, "specific_name")?;
        let ordinal = row_u32(&row, "ordinal")?;
        let mode = row_optional_text(&row, "parameter_mode")?.unwrap_or_else(|| "IN".to_owned());
        parameters
            .entry((schema, specific.clone()))
            .or_insert_with(Vec::new)
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
                ordinal,
            });
    }
    let mut functions = Vec::new();
    let mut procedures = Vec::new();
    for row in routine_rows {
        let schema = row_text(&row, "schema_name")?;
        let specific = row_text(&row, "specific_name")?;
        let name = row_text(&row, "routine_name")?;
        let params = parameters
            .remove(&(schema.clone(), specific.clone()))
            .unwrap_or_default();
        let return_type_name =
            row_optional_text(&row, "data_type")?.unwrap_or_else(|| "unknown".to_owned());
        if row_text(&row, "routine_type")? == "FUNCTION" {
            functions.push(CatalogFunction {
                id: format!("function:{schema}:{specific}"),
                schema_id: Some(schema_key(&schema)),
                name: identifier(name),
                parameters: params,
                return_type: type_ref(&return_type_name, 0, 0),
                variadic: false,
                overloads: vec![specific],
                kind: Some("function".to_owned()),
            });
        } else {
            procedures.push(CatalogProcedure {
                id: format!("procedure:{schema}:{specific}"),
                schema_id: Some(schema_key(&schema)),
                name: identifier(name),
                parameters: params,
            });
        }
    }
    Ok((functions, procedures))
}

async fn fetch_types(
    pool: &sqlx::MySqlPool,
    database_id: &str,
) -> ProviderResult<Vec<CatalogType>> {
    let rows = sqlx::query(
        "SELECT TABLE_SCHEMA AS schema_name, COLUMN_TYPE AS column_type
         FROM information_schema.COLUMNS
         WHERE TABLE_SCHEMA = ? AND DATA_TYPE IN ('enum', 'set')
         GROUP BY TABLE_SCHEMA, COLUMN_TYPE
         ORDER BY TABLE_SCHEMA, COLUMN_TYPE",
    )
    .bind(database_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|row| {
            let schema = row_text(&row, "schema_name")?;
            let definition = row_text(&row, "column_type")?;
            let family = type_ref(&definition, 0, 0).family;
            Ok(CatalogType {
                id: format!("type:{schema}:{definition}"),
                schema_id: Some(schema_key(&schema)),
                name: identifier(definition.clone()),
                family,
                definition: Some(definition),
            })
        })
        .collect()
}

fn sequences_from_objects(objects: &HashMap<String, MySqlObject>) -> Vec<CatalogSequence> {
    objects
        .values()
        .filter(|object| object.kind.eq_ignore_ascii_case("SEQUENCE"))
        .map(|object| CatalogSequence {
            id: format!("sequence:{}", object.id),
            schema_id: object.schema_id.clone(),
            name: identifier(&object.name),
            r#type: type_ref("bigint", 0, 0),
        })
        .collect()
}

async fn introspect_provider(request: CatalogRefreshRequest) -> ProviderResult<CatalogSnapshot> {
    let options = MySqlConnectOptions::new()
        .host(&request.server)
        .port(request.port)
        .username(&request.user_name)
        .password(&request.password)
        .database(&request.database)
        .ssl_mode(if request.trust_server_certificate {
            MySqlSslMode::Required
        } else {
            MySqlSslMode::VerifyIdentity
        });
    let pool = MySqlPoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await?;
    let schemas = fetch_schemas(&pool, &request.database).await?;
    let objects = fetch_objects(&pool, &request.database).await?;
    let mut columns_by_object = fetch_columns(&pool, &objects, &request.database).await?;
    let indexes = fetch_indexes(&pool, &columns_by_object, &request.database).await?;
    let (foreign_keys, mut primary_keys, mut unique_constraints, mut check_constraints) =
        fetch_constraints(&pool, &columns_by_object, &request.database).await?;
    for columns in columns_by_object.values_mut() {
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
    for object in objects.values() {
        let columns = columns_by_object.remove(&object.id).unwrap_or_default();
        if object.kind.eq_ignore_ascii_case("VIEW") {
            views.push(CatalogView {
                id: object.id.clone(),
                catalog_id: Some(request.database.clone()),
                database_id: request.database.clone(),
                schema_id: object.schema_id.clone(),
                name: identifier(&object.name),
                columns,
                comment: object.comment.clone(),
            });
            continue;
        }
        if object.kind.eq_ignore_ascii_case("SEQUENCE") {
            continue;
        }
        let primary_key = primary_keys
            .remove(&object.id)
            .and_then(|mut keys| keys.drain(..).next());
        let unique = unique_constraints.remove(&object.id).unwrap_or_default();
        let checks = check_constraints.remove(&object.id).unwrap_or_default();
        let table_indexes = indexes
            .iter()
            .filter(|index| index.table_id == object.id)
            .cloned()
            .collect();
        tables.push(CatalogTable {
            id: object.id.clone(),
            catalog_id: Some(request.database.clone()),
            database_id: request.database.clone(),
            schema_id: object.schema_id.clone(),
            name: identifier(&object.name),
            columns,
            primary_key,
            foreign_keys: foreign_keys
                .iter()
                .filter(|key| key.source_table_id == object.id)
                .cloned()
                .collect(),
            unique_constraints: unique,
            check_constraints: checks,
            indexes: table_indexes,
            estimated_rows: object.estimated_rows,
            comment: object.comment.clone(),
            foreign: false,
            temporary: false,
        });
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
    let (functions, procedures) = fetch_routines(&pool, &request.database).await?;
    let sequences = sequences_from_objects(&objects);
    let types = fetch_types(&pool, &request.database).await?;
    pool.close().await;
    Ok(CatalogSnapshot {
        schema_version: CATALOG_SCHEMA_VERSION,
        id: format!("catalog:{}:{}", request.connection_id, request.version),
        connection_id: request.connection_id,
        database_id: request.database,
        dialect: if request.dialect == DialectId::Mariadb {
            DialectId::Mariadb
        } else {
            DialectId::Mysql
        },
        version: request.version,
        generated_at: request.generated_at,
        databases: Vec::new(),
        schemas,
        tables,
        views,
        materialized_views: Vec::new(),
        functions,
        procedures,
        sequences,
        types,
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

pub async fn introspect(request: CatalogRefreshRequest) -> Result<CatalogSnapshot, String> {
    introspect_provider(request)
        .await
        .map_err(|error| error.to_string())
}

pub struct MySqlAdapter;

impl CatalogProvider for MySqlAdapter {
    fn dialect(&self) -> DialectId {
        DialectId::Mysql
    }

    fn refresh(&self, request: CatalogRefreshRequest) -> CatalogProviderFuture {
        Box::pin(async move {
            introspect(request)
                .await
                .map_err(|message| CatalogProviderError::Driver {
                    dialect: DialectId::Mysql,
                    message,
                })
        })
    }
}
