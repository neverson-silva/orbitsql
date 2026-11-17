import {
  BracketsCurly,
  Database,
  Eye,
  FolderSimple,
  Function as FunctionIcon,
  type IconWeight,
  ListNumbers,
  type Icon as PhosphorIcon,
  Rows,
  Sigma,
  SortAscending,
  Stack,
  Table,
  Terminal,
  TextAa,
  User,
} from "@phosphor-icons/react";
import type {
  ExplorerChild,
  ExplorerGroupKind,
} from "../../stores/connections.store";
import type { IconName } from "../ui/icons";

export const sidebarEngineIcons: Record<string, IconName> = {
  PostgreSQL: "database",
  "Microsoft SQL Server": "database",
};

export type DatabaseObjectVisual = {
  icon: PhosphorIcon;
  color: string;
  opacity: number;
  size: number;
  weight: IconWeight;
};

const objectColor = (name: string): string => `var(--ds-db-object-${name})`;

export const DATABASE_OBJECT_VISUALS: Record<
  Exclude<ExplorerChild["kind"], "group"> | ExplorerGroupKind,
  DatabaseObjectVisual
> = {
  database: {
    icon: Database,
    color: objectColor("database"),
    opacity: 0.96,
    size: 16,
    weight: "duotone",
  },
  schema: {
    icon: FolderSimple,
    color: objectColor("schema"),
    opacity: 0.88,
    size: 15,
    weight: "duotone",
  },
  user: {
    icon: User,
    color: objectColor("metadata"),
    opacity: 0.72,
    size: 14,
    weight: "regular",
  },
  tables: {
    icon: Table,
    color: objectColor("table"),
    opacity: 0.84,
    size: 15,
    weight: "duotone",
  },
  views: {
    icon: Eye,
    color: objectColor("view"),
    opacity: 0.84,
    size: 15,
    weight: "duotone",
  },
  procedures: {
    icon: Terminal,
    color: objectColor("procedure"),
    opacity: 0.82,
    size: 15,
    weight: "duotone",
  },
  foreignTables: {
    icon: Rows,
    color: objectColor("foreign-table"),
    opacity: 0.8,
    size: 15,
    weight: "duotone",
  },
  materializedViews: {
    icon: Stack,
    color: objectColor("materialized-view"),
    opacity: 0.84,
    size: 15,
    weight: "duotone",
  },
  indexes: {
    icon: ListNumbers,
    color: objectColor("index"),
    opacity: 0.82,
    size: 15,
    weight: "duotone",
  },
  functions: {
    icon: FunctionIcon,
    color: objectColor("function"),
    opacity: 0.84,
    size: 15,
    weight: "duotone",
  },
  sequences: {
    icon: SortAscending,
    color: objectColor("sequence"),
    opacity: 0.8,
    size: 15,
    weight: "duotone",
  },
  dataTypes: {
    icon: TextAa,
    color: objectColor("type"),
    opacity: 0.78,
    size: 15,
    weight: "duotone",
  },
  aggregateFunctions: {
    icon: Sigma,
    color: objectColor("aggregate"),
    opacity: 0.84,
    size: 15,
    weight: "duotone",
  },
  table: {
    icon: Table,
    color: objectColor("table"),
    opacity: 0.72,
    size: 14,
    weight: "regular",
  },
  view: {
    icon: Eye,
    color: objectColor("view"),
    opacity: 0.72,
    size: 14,
    weight: "regular",
  },
  procedure: {
    icon: Terminal,
    color: objectColor("procedure"),
    opacity: 0.7,
    size: 14,
    weight: "regular",
  },
  foreignTable: {
    icon: Rows,
    color: objectColor("foreign-table"),
    opacity: 0.68,
    size: 14,
    weight: "regular",
  },
  materializedView: {
    icon: Stack,
    color: objectColor("materialized-view"),
    opacity: 0.72,
    size: 14,
    weight: "regular",
  },
  index: {
    icon: ListNumbers,
    color: objectColor("index"),
    opacity: 0.7,
    size: 14,
    weight: "regular",
  },
  function: {
    icon: FunctionIcon,
    color: objectColor("function"),
    opacity: 0.72,
    size: 14,
    weight: "regular",
  },
  sequence: {
    icon: SortAscending,
    color: objectColor("sequence"),
    opacity: 0.68,
    size: 14,
    weight: "regular",
  },
  dataType: {
    icon: TextAa,
    color: objectColor("type"),
    opacity: 0.66,
    size: 14,
    weight: "regular",
  },
  metadata: {
    icon: BracketsCurly,
    color: objectColor("metadata"),
    opacity: 0.66,
    size: 13,
    weight: "regular",
  },
};

export function getDatabaseObjectVisual(
  child: Pick<ExplorerChild, "kind" | "group">,
): DatabaseObjectVisual {
  const key = child.kind === "group" ? (child.group ?? "tables") : child.kind;
  return DATABASE_OBJECT_VISUALS[key] ?? DATABASE_OBJECT_VISUALS.metadata;
}
