// Typed wrappers for the builder's Tauri commands (crates/x4sd/src/app.rs).
// Vectors and quaternions are in the game's frame (left-handed, Y up);
// view3d.ts converts them for Three.js.
import { invoke } from "@tauri-apps/api/core";

export type Vec3 = [number, number, number];
export type Quality = "fast" | "normal" | "detailed";
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
  /** A build stage ends after this module (`bookmark` in the plan). */
  stage_end: boolean;
  free_snaps: SnapInfo[];
}
export interface Issue { index: number | null; text: string }
export interface PlotInfo { pos: Vec3; neg: Vec3; size: Vec3; text: string }
/** The plot, the plot the station needs, whether it fits, whether any plot (20 km max) can hold it. */
export interface PlotStatus { plot: PlotInfo | null; required: PlotInfo | null; fits: boolean; possible: boolean }
export interface Scene {
  id: string;
  name: string;
  patches: string[];
  can_undo: boolean;
  can_redo: boolean;
  entries: Entry[];
  issues: Issue[];
  plot: PlotStatus | null;
  /** Changes since the last save, export or open. */
  dirty: boolean;
  /** After a build-order change: the moved module's new index (modules are renumbered). */
  focus?: number;
}
export interface CostLine { ware: string; name: string; amount: number; value: number | null }
export interface BuildCost { method: string; methods: { id: string; name: string }[]; lines: CostLine[]; value: number; unknown: string[] }
export interface Flow { ware: string; name: string; produced: number; used: number; net: number }
export interface StationAnalysis {
  sunlight: number;
  flows: Flow[];
  workforce: { needed: number; housed: number; by_race: { race: string; capacity: number }[]; fill: number };
  storage: { transport: string; capacity: number; volume_per_hour: number; hours: number | null }[];
  dock_areas: number;
  piers: number;
  unknown: string[];
  notes: string[];
}
export interface PlanRef { file: string; source?: string; id: string; name: string; modules: number }

let queue: Promise<unknown> = Promise.resolve();

/** Commands that read or change the design run one at a time, in call order:
 * otherwise replies can arrive out of order and an older scene overwrites a
 * newer one (seen with quick plot edits). */
function serial<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const call = queue.then(() => invoke<T>(command, args));
  queue = call.catch(() => undefined);
  return call;
}

export const api = {
  loadCatalog: () => invoke<{ modules: ModuleInfo[]; profile: string | null; game: string | null }>("load_catalog"),
  moduleShape: (macroName: string) => invoke<Shape>("module_shape", { macroName }),
  /** Raw model bytes; see `module_model` in app.rs. */
  moduleModel: (macroName: string, quality: Quality) => invoke<ArrayBuffer>("module_model", { macroName, quality }),
  materialMaps: (name: string) => invoke<{ diffuse: string | null; normal: string | null }>("material_maps", { name }),
  /** Raw texture bytes; see `texture` in app.rs. */
  texture: (path: string) => invoke<ArrayBuffer>("texture", { path }),
  scene: () => serial<Scene>("scene"),
  newDesign: (name: string) => serial<Scene>("new_design", { name }),
  listGamePlans: () => invoke<PlanRef[]>("list_game_plans"),
  listPlansInFile: (file: string) => invoke<PlanRef[]>("list_plans_in_file", { file }),
  openDesign: (file: string, id: string) => serial<Scene>("open_design", { file, id }),
  saveDesign: (file: string) => serial<{ file: string }>("save_design", { file }),
  exportDesign: (newId: boolean, overwrite: boolean) => serial<{ file: string; id: string }>("export_design", { newId, overwrite }),
  renameDesign: (name: string) => serial<Scene>("rename_design", { name }),
  addModule: (macroName: string, position: Vec3) => serial<Scene>("add_module", { macroName, position }),
  attachModule: (macroName: string, snap: string, to: number, toSnap: string, twist: number) =>
    serial<Scene>("attach_module", { macroName, snap, to, toSnap, twist }),
  setTwist: (index: number, twist: number) => serial<Scene>("set_twist", { index, twist }),
  removeModule: (index: number) => serial<Scene>("remove_module", { index }),
  setPlot: (pos: Vec3, neg: Vec3) => serial<Scene>("set_plot", { pos, neg }),
  fitPlot: (growOnly: boolean) => serial<Scene>("fit_plot", { growOnly }),
  moveInBuildOrder: (index: number, position: number) => serial<Scene>("move_in_build_order", { index, position }),
  buildFirst: (index: number) => serial<Scene>("build_first", { index }),
  buildLast: (index: number) => serial<Scene>("build_last", { index }),
  setStageEnd: (index: number, end: boolean) => serial<Scene>("set_stage_end", { index, end }),
  reattachModule: (index: number, snap: string, to: number, toSnap: string, twist: number) =>
    serial<Scene>("reattach_module", { index, snap, to, toSnap, twist }),
  detachModule: (index: number) => serial<Scene>("detach_module", { index }),
  moveRoot: (index: number, position: Vec3) => serial<Scene>("move_root", { index, position }),
  buildCost: (index: number | null, method: string) => serial<BuildCost>("build_cost", { index, method }),
  analyzeStation: (sunlight: number) => serial<StationAnalysis>("analyze_station", { sunlight }),
  undo: () => serial<Scene>("undo"),
  redo: () => serial<Scene>("redo"),
};
