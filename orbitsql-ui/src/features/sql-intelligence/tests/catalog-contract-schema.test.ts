import Ajv from "ajv/dist/2020";
import { describe, expect, it } from "vitest";
import schema from "../../../../docs/catalog-snapshot.schema.json";
import { parseCatalogSnapshot } from "../catalog/contract";
import { createCatalog } from "./fixtures";

function identifier(raw: string) {
  return {
    raw,
    normalized: raw,
    quoted: false,
    quoteStyle: "none" as const,
  };
}

describe("catalog JSON contract", () => {
  it("accepts the synthetic canonical snapshot", () => {
    const validate = new Ajv({ strict: false }).compile(schema);
    expect(validate(createCatalog().snapshot)).toBe(true);
  });

  it("rejects a snapshot with an invalid dialect", () => {
    const validate = new Ajv({ strict: false }).compile(schema);
    expect(
      validate({ ...createCatalog().snapshot, dialect: "unsupported" }),
    ).toBe(false);
  });

  it("parses the complete catalog contract", () => {
    const base = createCatalog().snapshot;
    const table = {
      ...base.tables[0],
      columns: base.tables[0].columns.map((column, index) =>
        index === 0
          ? { ...column, defaultValue: "1", comment: "identifier" }
          : column,
      ),
      primaryKey: {
        id: "constraint:users:primary",
        name: identifier("users_pkey"),
        tableId: "table:users",
        columnIds: ["column:users:id"],
        expression: "id is not null",
      },
      uniqueConstraints: [
        {
          id: "constraint:users:unique",
          name: identifier("users_email_key"),
          tableId: "table:users",
          columnIds: ["column:users:email"],
        },
      ],
      checkConstraints: [
        {
          id: "constraint:users:check",
          name: identifier("users_name_check"),
          tableId: "table:users",
          expression: "name <> ''",
        },
      ],
      indexes: [
        {
          id: "index:users:email",
          name: identifier("users_email_idx"),
          tableId: "table:users",
          columnIds: ["column:users:email"],
          unique: true,
          primary: false,
        },
      ],
      estimatedRows: 3,
      comment: "users table",
    };
    const rich = {
      ...base,
      databases: [{ id: "db:test", name: identifier("test"), system: false }],
      schemas: [
        ...base.schemas,
        {
          id: "schema:private",
          databaseId: "db:test",
          name: identifier("private"),
        },
      ],
      tables: [table],
      views: [
        {
          id: "view:users",
          databaseId: "db:test",
          schemaId: "schema:public",
          name: identifier("active_users"),
          columns: [],
          comment: "active users view",
        },
      ],
      materializedViews: [
        {
          id: "materialized-view:users",
          databaseId: "db:test",
          schemaId: "schema:public",
          name: identifier("active_users_mv"),
          columns: [],
        },
      ],
      functions: [
        {
          id: "function:users",
          name: identifier("find_user"),
          parameters: [
            {
              id: "parameter:find_user:id",
              name: identifier("user_id"),
              type: {
                family: "integer",
                name: "integer",
                nullable: false,
                precision: 10,
                scale: 0,
                elementType: { family: "text", name: "text" },
              },
              direction: "inout",
              ordinal: 0,
            },
          ],
          returnType: { family: "text", name: "text" },
          variadic: true,
          overloads: ["find_user(integer)"],
        },
      ],
      procedures: [
        {
          id: "procedure:users",
          name: identifier("refresh_users"),
          parameters: [],
        },
      ],
      sequences: [
        {
          id: "sequence:users",
          schemaId: "schema:public",
          name: identifier("users_id_seq"),
          type: { family: "integer", name: "integer" },
        },
      ],
      types: [
        {
          id: "type:status",
          name: identifier("status"),
          family: "enum",
          definition: "enum('active','inactive')",
        },
      ],
      triggers: [
        {
          id: "trigger:users",
          schemaId: "schema:public",
          tableId: "table:users",
          name: identifier("users_updated"),
          enabled: true,
          definition: "before update",
        },
      ],
      extensions: [
        { id: "extension:uuid", name: identifier("uuid-ossp"), version: "1" },
      ],
      partitions: [
        {
          id: "partition:users",
          tableId: "table:users",
          name: identifier("users_2026"),
          parentId: "table:users",
          bound: "2026",
        },
      ],
      principals: [
        {
          id: "principal:users",
          name: identifier("analyst"),
          kind: "role",
          login: "analyst",
        },
      ],
      permissions: [
        {
          id: "permission:users",
          principalId: "principal:users",
          objectId: "table:users",
          permission: "select",
          state: "grant",
        },
      ],
      synonyms: [
        {
          id: "synonym:users",
          schemaId: "schema:public",
          name: identifier("people"),
          target: "public.users",
        },
      ],
      constraints: [
        {
          id: "constraint:users:table",
          name: identifier("users_table_check"),
          tableId: "table:users",
          columnIds: ["column:users:id"],
          expression: "id > 0",
        },
      ],
    };

    expect(parseCatalogSnapshot(rich)).toBe(rich);
  });
});
