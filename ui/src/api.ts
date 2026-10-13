// Typed wrappers for the builder's Tauri commands (crates/x4sd/src/app.rs).
// Vectors and quaternions are in the game's frame (left-handed, Y up);
// view3d.ts converts them for Three.js.
import { invoke } from "@tauri-apps/api/core";

export type Vec3 = [number, number, number];
/** w, x, y, z */
export type Quat = [number, number, number, number];

export interface ModuleInfo { macro: string; class: string; race: string | null; source: string | null; name: string }
export interface SnapInfo { name: string; position: Vec3; facing: Vec3 }
export interface ShapeBox { position: Vec3; rotation: Quat; half: Vec3 }
export interface Shape { macro: string; class: string; snaps: SnapInfo[]; boxes: ShapeBox[] }
export interface Attachment { to: number; to_snap: string; snap: string; twist: number | null }
export interface Entry {
  index: number;
  macro: string;
  name: string | null;
  class: string | null;
  position: Vec3;
  rotation: Quat;
  attached: Attachment | null;
  free_snaps: SnapInfo[];
}
export interface Issue { index: number | null; text: string }
export interface Scene { id: string; name: string; patches: string[]; can_undo: boolean; can_redo: boolean; entries: Entry[]; issues: Issue[] }
export interface PlanRef { file: string; source?: string; id: string; name: string; modules: number }

export const api = {
  loadCatalog: () => invoke<{ modules: ModuleInfo[]; profile: string | null; game: string | null }>("load_catalog"),
  moduleShape: (macroName: string) => invoke<Shape>("module_shape", { macroName }),
  scene: () => invoke<Scene>("scene"),
  newDesign: (name: string) => invoke<Scene>("new_design", { name }),
  listGamePlans: () => invoke<PlanRef[]>("list_game_plans"),
  listPlansInFile: (file: string) => invoke<PlanRef[]>("list_plans_in_file", { file }),
  openDesign: (file: string, id: string) => invoke<Scene>("open_design", { file, id }),
  saveDesign: (file: string) => invoke<{ file: string }>("save_design", { file }),
  exportDesign: (newId: boolean, overwrite: boolean) => invoke<{ file: string; id: string }>("export_design", { newId, overwrite }),
  renameDesign: (name: string) => invoke<Scene>("rename_design", { name }),
  addModule: (macroName: string, position: Vec3) => invoke<Scene>("add_module", { macroName, position }),
  attachModule: (macroName: string, snap: string, to: number, toSnap: string, twist: number) =>
    invoke<Scene>("attach_module", { macroName, snap, to, toSnap, twist }),
  setTwist: (index: number, twist: number) => invoke<Scene>("set_twist", { index, twist }),
  removeModule: (index: number) => invoke<Scene>("remove_module", { index }),
  undo: () => invoke<Scene>("undo"),
  redo: () => invoke<Scene>("redo"),
};
