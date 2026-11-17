import { create } from "zustand";

type EditorState = {
  query: string;
  setQuery: (query: string) => void;
};

export const useEditorStore = create<EditorState>((set) => ({
  query: "SELECT ",
  setQuery: (query) => set({ query }),
}));
