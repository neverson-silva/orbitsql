use crate::backend::domain::catalog::{
    Identifier, IdentifierQuoteStyle, SqlTypeFamily, SqlTypeRef,
};

pub fn identifier(value: impl Into<String>) -> Identifier {
    let value = value.into();
    Identifier {
        normalized: value.clone(),
        raw: value,
        quoted: false,
        quote_style: IdentifierQuoteStyle::None,
    }
}

pub fn type_ref(name: &str, precision: u8, scale: u8) -> SqlTypeRef {
    let lower = name.trim().to_ascii_lowercase();
    let base = lower.split('(').next().unwrap_or(lower.as_str()).trim();
    let array_element = base.strip_suffix("[]");
    let family = if array_element.is_some() {
        SqlTypeFamily::Array
    } else {
        match base {
            "bool" | "boolean" => SqlTypeFamily::Boolean,
            "bit" | "tinyint" | "smallint" | "mediumint" | "int" | "integer" | "bigint"
            | "int2" | "int4" | "int8" | "serial" | "smallserial" | "bigserial" | "year" => {
                SqlTypeFamily::Integer
            }
            "decimal" | "numeric" | "money" | "smallmoney" => SqlTypeFamily::Decimal,
            "real" | "float" | "float4" | "float8" | "double" | "double precision" => {
                SqlTypeFamily::Float
            }
            "date" => SqlTypeFamily::Date,
            "time" | "time without time zone" | "time with time zone" => SqlTypeFamily::Time,
            "timestamp"
            | "timestamp without time zone"
            | "timestamp with time zone"
            | "datetime"
            | "datetime2"
            | "smalldatetime"
            | "datetimeoffset" => SqlTypeFamily::Timestamp,
            "binary" | "varbinary" | "blob" | "bytea" | "image" | "rowversion" => {
                SqlTypeFamily::Binary
            }
            "uuid" | "uniqueidentifier" => SqlTypeFamily::Uuid,
            "json" | "jsonb" => SqlTypeFamily::Json,
            "xml" => SqlTypeFamily::Xml,
            "interval" => SqlTypeFamily::Interval,
            "enum" | "set" => SqlTypeFamily::Enum,
            "geometry" | "geography" | "point" | "linestring" | "polygon" => SqlTypeFamily::Spatial,
            "char" | "character" | "varchar" | "character varying" | "nvarchar" | "nchar"
            | "text" | "tinytext" | "mediumtext" | "longtext" | "clob" | "string" => {
                SqlTypeFamily::Text
            }
            _ => SqlTypeFamily::Custom,
        }
    };
    let element_type = array_element.map(|element| Box::new(type_ref(element, 0, 0)));
    SqlTypeRef {
        family,
        name: name.to_owned(),
        nullable: None,
        precision: (precision > 0).then_some(u32::from(precision)),
        scale: (scale > 0).then_some(u32::from(scale)),
        element_type,
    }
}
