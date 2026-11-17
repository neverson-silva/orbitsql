import type { IconProps, Icon as PhosphorIcon } from "@phosphor-icons/react";
import {
  ArrowDown,
  ArrowSquareOut,
  ArrowsClockwise,
  ArrowsIn,
  ArrowsOut,
  ArrowsSplit,
  ArrowUpRight,
  ArrowUUpLeft,
  Bell,
  Binary,
  BracketsCurly,
  CalendarBlank,
  CaretDown,
  CaretLeft,
  CaretRight,
  Check,
  Circle,
  Clock,
  ClockCounterClockwise,
  Code,
  Columns,
  Copy,
  Cpu,
  Cube,
  Cylinder,
  Database,
  DotsThree,
  DownloadSimple,
  Eye,
  EyeSlash,
  File,
  FileSql,
  Fingerprint,
  Folder,
  Funnel,
  Gauge,
  Gear,
  GitBranch,
  GitCommit,
  Hash,
  Key,
  Layout,
  Lightning,
  Link,
  List,
  LockKey,
  MagnifyingGlass,
  Minus,
  Network,
  Pause,
  PencilSimple,
  Play,
  Plug,
  Plugs,
  PlugsConnected,
  Plus,
  PlusCircle,
  Pulse,
  SealCheck,
  ShieldCheck,
  SidebarSimple,
  Sliders,
  Snowflake,
  SpinnerGap,
  SquaresFour,
  Table,
  Terminal,
  TextT,
  ToggleLeft,
  Trash,
  TreeStructure,
  User,
  Warning,
  X,
} from "@phosphor-icons/react";

export type IconName =
  | "activity"
  | "add-circle"
  | "arrow-down"
  | "arrow-up-right"
  | "bell"
  | "bolt"
  | "check"
  | "chevron-down"
  | "chevron-left"
  | "chevron-right"
  | "circle"
  | "commit"
  | "clock"
  | "close"
  | "code"
  | "columns"
  | "copy"
  | "cube"
  | "cylinder"
  | "database"
  | "diagram"
  | "dock"
  | "download"
  | "external"
  | "eye"
  | "eye-off"
  | "file"
  | "filter"
  | "folder"
  | "grid"
  | "history"
  | "key"
  | "layout"
  | "link"
  | "list"
  | "lock"
  | "maximize"
  | "memory"
  | "minus"
  | "more"
  | "network"
  | "pause"
  | "pencil"
  | "plug"
  | "plugs"
  | "plugs-connected"
  | "person"
  | "play"
  | "plus"
  | "refresh"
  | "restore"
  | "rollback"
  | "search"
  | "settings"
  | "shield"
  | "snowflake"
  | "spinner"
  | "speed"
  | "sql-file"
  | "split"
  | "sync"
  | "table"
  | "terminal"
  | "trash"
  | "tune"
  | "type-array"
  | "type-binary"
  | "type-boolean"
  | "type-date"
  | "type-json"
  | "type-number"
  | "type-text"
  | "type-uuid"
  | "user"
  | "verified"
  | "warning";

type IconPropsWithName = Omit<IconProps, "size"> & {
  name: IconName;
  size?: number;
};

const icons: Record<IconName, PhosphorIcon> = {
  activity: Pulse,
  "add-circle": PlusCircle,
  "arrow-down": ArrowDown,
  "arrow-up-right": ArrowUpRight,
  bell: Bell,
  bolt: Lightning,
  check: Check,
  "chevron-down": CaretDown,
  "chevron-left": CaretLeft,
  "chevron-right": CaretRight,
  circle: Circle,
  commit: GitCommit,
  clock: Clock,
  close: X,
  code: Code,
  columns: Columns,
  copy: Copy,
  cube: Cube,
  cylinder: Cylinder,
  database: Database,
  diagram: TreeStructure,
  dock: SidebarSimple,
  download: DownloadSimple,
  external: ArrowSquareOut,
  eye: Eye,
  "eye-off": EyeSlash,
  file: File,
  filter: Funnel,
  folder: Folder,
  grid: SquaresFour,
  history: ClockCounterClockwise,
  key: Key,
  layout: Layout,
  link: Link,
  list: List,
  lock: LockKey,
  maximize: ArrowsOut,
  memory: Cpu,
  minus: Minus,
  more: DotsThree,
  network: Network,
  pause: Pause,
  pencil: PencilSimple,
  plug: Plug,
  plugs: Plugs,
  "plugs-connected": PlugsConnected,
  person: User,
  play: Play,
  plus: Plus,
  refresh: ArrowsClockwise,
  restore: ArrowsIn,
  rollback: ArrowUUpLeft,
  search: MagnifyingGlass,
  settings: Gear,
  shield: ShieldCheck,
  snowflake: Snowflake,
  spinner: SpinnerGap,
  speed: Gauge,
  "sql-file": FileSql,
  split: ArrowsSplit,
  sync: GitBranch,
  table: Table,
  terminal: Terminal,
  "type-array": List,
  "type-binary": Binary,
  "type-boolean": ToggleLeft,
  "type-date": CalendarBlank,
  "type-json": BracketsCurly,
  "type-number": Hash,
  "type-text": TextT,
  "type-uuid": Fingerprint,
  trash: Trash,
  tune: Sliders,
  user: User,
  verified: SealCheck,
  warning: Warning,
};

export function Icon({
  name,
  size = 16,
  strokeWidth: _strokeWidth,
  ...props
}: IconPropsWithName) {
  const Component = icons[name];
  return (
    <Component aria-hidden="true" size={size} weight="regular" {...props} />
  );
}
