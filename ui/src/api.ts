// Typed wrappers for the builder's Tauri commands (crates/x4sd/src/app.rs).
// Vectors and quaternions are in the game's frame (left-handed, Y up);
// view3d.ts converts them for Three.js.
import { invoke } from "@tauri-apps/api/core";

export type Vec3 = [number, number, number];
export type Quality = "fast" | "normal" | "detailed";
/** w, x, y, z */
export type Quat = [number, number, number, number];

export interface ModuleInfo { macro: string; class: string; race: string | null; source: string | null; name: string }
/** `rotation`: the snap's frame (its axis is local +Z), for the attach math. */
export interface SnapInfo { name: string; position: Vec3; facing: Vec3; rotation: Quat }
export interface ShapeBox { position: Vec3; rotation: Quat; half: Vec3 }
export interface Shape { macro: string; class: string; snaps: SnapInfo[]; boxes: ShapeBox[] }
/** `tilt`: degrees off the snap axis (0 = straight); `joint`: where the snaps meet. */
export interface Attachment { to: number; to_snap: string; snap: string; twist: number | null; tilt: number | null; joint: Vec3 | null }
export interface Entry {
  index: number;
  /** Turret and shield groups (one per group and kind): equipped, total. */
  equipment: [number, number];
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
  /** After auto-placement: what was placed, the connection structures added, what didn't fit. */
  placement?: Placement;
}
export interface Placement {
  placed: { index: number; macro: string }[];
  connectors: { index: number; macro: string }[];
  unplaced: { macro: string; count: number }[];
  /** Docks and piers left without a clear corridor (no such spot). */
  docks_blocked: number;
  /** Modules with sides that found no spot on their side with a clear view out. */
  sides_blocked: number;
}
export interface CostLine { ware: string; name: string; amount: number; value: number | null }
export interface BuildCost { method: string; methods: { id: string; name: string }[]; lines: CostLine[]; value: number; unknown: string[] }
export interface Flow { ware: string; name: string; produced: number; used: number; net: number; raw: boolean }
export interface StationAnalysis {
  sunlight: number;
  flows: Flow[];
  /** Every made ware is covered; raw materials (mined) may be short. */
  self_sufficient: boolean;
  workforce: { needed: number; housed: number; by_race: { race: string; capacity: number }[]; fill: number };
  storage: { transport: string; capacity: number; volume_per_hour: number; hours: number | null }[];
  dock_areas: number;
  piers: number;
  unknown: string[];
  notes: string[];
}
export interface PlannedModule { macro: string; name: string; count: number; role: "target" | "extra" | "chain" | "habitat" | "storage" | "dock"; ware: string | null }
export interface PlanArgs {
  targets: { ware: string; modules: number }[];
  extra_modules: { macro: string; count: number }[];
  /** m³ per cargo type (container, solid, liquid) the station has at least. */
  storage: Record<string, number>;
  race: string | null;
  workforce: boolean;
  storage_hours: number;
  buy: string[];
  sunlight: number;
  method: string;
}
/** What a planner target's modules make per hour (`per_hour`), or why nothing does (`no_blueprint`). */
export interface TargetRate { ware: string; macro?: string; name?: string; per_module?: number; per_hour?: number; no_blueprint?: boolean }
export interface ProductionPlan { modules: PlannedModule[]; analysis: StationAnalysis; cost: { lines: CostLine[]; value: number; unknown: string[] }; unknown_targets: string[]; no_blueprint: string[] }
/** The player's blueprints from a save (`ops::Blueprints`): while `on`, only `modules` are offered. */
export interface Blueprints { on: boolean; save: string | null; read_at: number | null; modules: string[] }
export interface EquipmentItem { macro: string; name: string; kind: "turrets" | "shields"; size: string | null; race: string | null; mk: number | null; price: number | null }
export interface EquipmentGroup {
  group: string;
  kind: "turrets" | "shields";
  size: string | null;
  slots: number;
  current: { macro: string; name: string | null; count: number } | null;
  /** Equipment macros that fit every slot. */
  fits: string[];
}
/** A sector and its sunlight in percent. */
export interface Sector { macro: string; name: string; sunlight: number }

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
  deleteImportFile: (file: string) => invoke<unknown>("delete_import_file", { file }),
  openDesign: (file: string, id: string) => serial<Scene>("open_design", { file, id }),
  saveDesign: (file: string) => serial<{ file: string; new_plan: boolean }>("save_design", { file }),
  exportDesign: (newId: boolean, overwrite: boolean) =>
    serial<{ file: string; id: string; new_plan: boolean }>("export_design", { newId, overwrite }),
  renameDesign: (name: string) => serial<Scene>("rename_design", { name }),
  addModule: (macroName: string, position: Vec3) => serial<Scene>("add_module", { macroName, position }),
  attachModule: (macroName: string, snap: string, to: number, toSnap: string, twist: number) =>
    serial<Scene>("attach_module", { macroName, snap, to, toSnap, twist }),
  setTwist: (index: number, twist: number) => serial<Scene>("set_twist", { index, twist }),
  /** Removes a module alone (what was on it stays, unattached) or, with `sequence`, with everything on it. */
  removeModule: (index: number, sequence: boolean) => serial<Scene>("remove_module", { index, sequence }),
  /** Copies a module and everything on it onto a free snap; `focus` is the copy. */
  copySequence: (index: number, snap: string, to: number, toSnap: string, twist: number) =>
    serial<Scene>("copy_sequence", { index, snap, to, toSnap, twist }),
  setPlot: (pos: Vec3, neg: Vec3) => serial<Scene>("set_plot", { pos, neg }),
  fitPlot: (growOnly: boolean) => serial<Scene>("fit_plot", { growOnly }),
  moveInBuildOrder: (index: number, position: number) => serial<Scene>("move_in_build_order", { index, position }),
  buildFirst: (index: number) => serial<Scene>("build_first", { index }),
  buildLast: (index: number) => serial<Scene>("build_last", { index }),
  setStageEnd: (index: number, end: boolean) => serial<Scene>("set_stage_end", { index, end }),
  reattachModule: (index: number, snap: string, to: number, toSnap: string, twist: number) =>
    serial<Scene>("reattach_module", { index, snap, to, toSnap, twist }),
  detachModule: (index: number) => serial<Scene>("detach_module", { index }),
  /** Turns an attached module (and everything on it) about its joint, staying attached. */
  rotateAttached: (index: number, rotation: Quat) => serial<Scene>("rotate_attached", { index, rotation }),
  straighten: (index: number) => serial<Scene>("straighten", { index }),
  /** Moves (and with `rotation`, turns) a free-standing module and everything on it. */
  moveRoot: (index: number, position: Vec3, rotation: Quat | null = null) => serial<Scene>("move_root", { index, position, rotation }),
  /** Checks as if the module were dropped where the gizmo is now; the design doesn't change. */
  previewGizmo: (index: number, position: Vec3, rotation: Quat) => invoke<{ issues: Issue[]; plot: PlotStatus | null }>("preview_gizmo", { index, position, rotation }),
  buildCost: (index: number | null, method: string) => serial<BuildCost>("build_cost", { index, method }),
  listSectors: () => invoke<Sector[]>("list_sectors"),
  analyzeStation: (sunlight: number) => serial<StationAnalysis>("analyze_station", { sunlight }),
  listWares: () => serial<{ ware: string; name: string }[]>("list_wares"),
  /** `baseline`: plan for a station with these modules instead of the open design. */
  targetRates: (args: PlanArgs) => invoke<TargetRate[]>("target_rates", { args }),
  planProduction: (args: PlanArgs, baseline: { macro: string; count: number }[] | null) =>
    serial<ProductionPlan>("plan_production", { args, baseline }),
  /** Places the modules at free snaps without overlaps (the plot may grow); one undo step. */
  autoPlace: (modules: { macro: string; count: number; sides?: string[] }[], race: string | null, fewerConnectors: boolean) =>
    serial<Scene>("auto_place", { modules, race, fewerConnectors }),
  /** Reads the blueprints from a save (about half a second) and turns the limit on. */
  readBlueprints: (file: string) => invoke<Blueprints>("read_blueprints", { file }),
  setBlueprints: (blueprints: Blueprints) => invoke<Scene>("set_blueprints", { blueprints }),
  /** A shared operation (`ops::Op` in Rust, as MCP agents use them): its result, and the scene if it may have changed. */
  runOp: <T>(op: { op: string } & Record<string, unknown>) => serial<{ result: T; scene?: Scene }>("run_op", { op }),
  undo: () => serial<Scene>("undo"),
  redo: () => serial<Scene>("redo"),
};
