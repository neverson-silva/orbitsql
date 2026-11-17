import { create } from "zustand";
import type { ExplorerFolder } from "./connections.store";

const defaultFolderColor = "#4f8ed8";

type SidebarFolderState = {
  folderDialogOpen: boolean;
  editingFolderId: string | null;
  folderName: string;
  folderColor: string;
  openNewFolder: () => void;
  openFolderEditor: (folder: ExplorerFolder) => void;
  setFolderName: (folderName: string) => void;
  setFolderColor: (folderColor: string) => void;
  closeFolderDialog: () => void;
};

export const useSidebarFolderStore = create<SidebarFolderState>((set) => ({
  folderDialogOpen: false,
  editingFolderId: null,
  folderName: "",
  folderColor: defaultFolderColor,
  openNewFolder: () =>
    set({
      folderDialogOpen: true,
      editingFolderId: null,
      folderName: "",
      folderColor: defaultFolderColor,
    }),
  openFolderEditor: (folder) =>
    set({
      folderDialogOpen: true,
      editingFolderId: folder.id,
      folderName: folder.name,
      folderColor: folder.color,
    }),
  setFolderName: (folderName) => set({ folderName }),
  setFolderColor: (folderColor) => set({ folderColor }),
  closeFolderDialog: () =>
    set({
      folderDialogOpen: false,
      editingFolderId: null,
      folderName: "",
      folderColor: defaultFolderColor,
    }),
}));
