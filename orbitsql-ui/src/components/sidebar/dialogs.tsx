import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "../ui/dialog";
import { Input } from "../ui/input";
import { Label } from "../ui/label";
import { Button } from "../ui/ui";
import type {
  SidebarDeleteTarget,
  SidebarObjectMutation,
} from "./sidebar.types";

export function ConnectionDialog({
  open,
  name,
  onOpenChange,
  onChange,
  onCancel,
  onSave,
}: {
  open: boolean;
  name: string;
  onOpenChange: (open: boolean) => void;
  onChange: (value: string) => void;
  onCancel: () => void;
  onSave: () => void;
}) {
  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(360px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>Rename connection</DialogTitle>
          <DialogDescription>
            Choose the name shown in your connection explorer.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <label
            className="block text-label-caps text-on-surface-variant"
            htmlFor="sidebar-name"
          >
            Connection name
            <Input
              autoFocus
              className="mt-1.5"
              id="sidebar-name"
              onChange={(event) => onChange(event.target.value)}
              value={name}
            />
          </label>
        </div>
        <DialogFooter>
          <Button onClick={onCancel} variant="ghost">
            Cancel
          </Button>
          <Button onClick={onSave} variant="primary">
            Save changes
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function FolderDialog({
  open,
  editing,
  name,
  color,
  onOpenChange,
  onNameChange,
  onColorChange,
  onCancel,
  onSave,
}: {
  open: boolean;
  editing: boolean;
  name: string;
  color: string;
  onOpenChange: (open: boolean) => void;
  onNameChange: (value: string) => void;
  onColorChange: (value: string) => void;
  onCancel: () => void;
  onSave: () => void;
}) {
  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(360px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>{editing ? "Edit folder" : "New folder"}</DialogTitle>
          <DialogDescription>
            Organize connections with a name and an accent color.
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-3">
          <label
            className="block text-label-caps text-on-surface-variant"
            htmlFor="sidebar-folder-name"
          >
            Folder name
            <Input
              autoFocus
              className="mt-1.5"
              id="sidebar-folder-name"
              onChange={(event) => onNameChange(event.target.value)}
              placeholder="e.g. Production"
              value={name}
            />
          </label>
          <div className="flex items-center justify-between gap-3">
            <Label htmlFor="sidebar-folder-color">Folder color</Label>
            <input
              aria-label="Folder color"
              className="h-8 w-12 cursor-pointer rounded-md border border-outline-variant bg-transparent p-0.5"
              id="sidebar-folder-color"
              onChange={(event) => onColorChange(event.target.value)}
              type="color"
              value={color}
            />
          </div>
        </div>
        <DialogFooter>
          <Button onClick={onCancel} variant="ghost">
            Cancel
          </Button>
          <Button onClick={onSave} variant="primary">
            Save folder
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function DeleteDialog({
  open,
  target,
  onOpenChange,
  onCancel,
  onConfirm,
}: {
  open: boolean;
  target: SidebarDeleteTarget | null;
  onOpenChange: (open: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const folder = target?.kind === "folder";
  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent className="w-[min(380px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>Delete {folder ? "folder" : "connection"}?</DialogTitle>
          <DialogDescription>
            {folder
              ? "The folder will be removed and its connections will return to the root."
              : "This removes the connection from the local explorer."}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button onClick={onCancel} variant="ghost">
            Cancel
          </Button>
          <Button onClick={onConfirm} variant="danger">
            Delete
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function ObjectMutationDialog({
  mutation,
  newName,
  onNameChange,
  onOpenChange,
  onCancel,
  onConfirm,
}: {
  mutation: SidebarObjectMutation | null;
  newName: string;
  onNameChange: (value: string) => void;
  onOpenChange: (open: boolean) => void;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const rename = mutation?.operation === "rename";
  return (
    <Dialog onOpenChange={onOpenChange} open={Boolean(mutation)}>
      <DialogContent className="w-[min(560px,calc(100vw-2rem))]">
        <DialogHeader>
          <DialogTitle>
            {rename
              ? `Rename ${mutation?.child.name}`
              : `Delete ${mutation?.child.name}?`}
          </DialogTitle>
          <DialogDescription>
            {rename
              ? "The statement below will be executed on the connected database."
              : "The statement below will permanently remove this database object."}
          </DialogDescription>
        </DialogHeader>
        {rename ? (
          <label
            className="block text-label-caps text-on-surface-variant"
            htmlFor="sidebar-object-name"
          >
            New name
            <Input
              autoFocus
              className="mt-1.5"
              id="sidebar-object-name"
              onChange={(event) => onNameChange(event.target.value)}
              value={newName}
            />
          </label>
        ) : null}
        <pre className="max-h-40 overflow-auto rounded-md border border-outline-variant/60 bg-app-content p-3 font-mono text-label-caps text-on-surface">
          {mutation?.sql}
        </pre>
        <DialogFooter>
          <Button onClick={onCancel} variant="ghost">
            Cancel
          </Button>
          <Button onClick={onConfirm} variant={rename ? "primary" : "danger"}>
            {rename ? "Rename" : "Delete"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
