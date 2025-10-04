import { createStore, create } from "zustand";

// eslint-disable-next-line @typescript-eslint/no-unused-vars
const PROGRAM_LANGUAGE_LIST = [
  "JavaScript",
  "TypeScript",
  "Java",
  "Python",
  "Rust",
  "Golang",
  "Ruby",
  "C#",
  "C++",
  "C",
  "PHP",
  "Kotlin",
  "Swift",
  "Dart",
  "Scala",
  "Elixir",
];

export type CodeLanguage = (typeof PROGRAM_LANGUAGE_LIST)[number];

export interface AppState {
  currentSelectedLanguage: CodeLanguage;
  updateCurrentSelectedLanguage: (value: CodeLanguage) => void;
}

export const useAppStateStore = create<AppState>((set) => {
  return {
    currentSelectedLanguage: "",
    updateCurrentSelectedLanguage: (value) =>
      set(() => ({ currentSelectedLanguage: value })),
  };
});

export interface AppStateNoHook {
  currentScreenShotPath: string;
  updateCurrentScreenShotPath: (value: string) => void;
  startShowSolution: boolean;
  updateStartShowSolution: (value: boolean) => void;
}

export const useAppStateStoreWithNoHook = createStore<AppStateNoHook>((set) => {
  return {
    currentScreenShotPath: "",
    updateCurrentScreenShotPath: (value) =>
      set(() => ({ currentScreenShotPath: value })),
    startShowSolution: false,
    updateStartShowSolution: (value) =>
      set(() => ({ startShowSolution: value })),
  };
});
