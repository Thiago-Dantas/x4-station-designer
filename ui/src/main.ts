// Builder UI: palette, 3D view, inspector. All edits go to the Rust core
// through the Tauri commands in api.ts, which return the new scene.
import "./style.css";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type { BufferGeometry } from "three";
import { api, type Blueprints, type Entry, type EquipmentGroup, type EquipmentItem, type Issue, type ModuleInfo, type PlannedModule, type PlanRef, type PlotStatus, type ProductionPlan, type Quality, type Quat, type Scene, type Shape, type SnapInfo, type Vec3 } from "./api";
import { attachMatrix, CLASS_COLORS, type DiffuseTexture, modelGeometry, poseMatrix, View3D } from "./view3d";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const view = new View3D($("viewport"));
const shapes = new Map<string, Shape>();
/** Real models per macro at the current quality (null: none, boxes are drawn). */
const models = new Map<string, BufferGeometry | null>();
/** Diffuse textures per game material (null: none or unsupported). */
const textures = new Map<string, DiffuseTexture | null>();
// On by default: measured at about 0.2 s and 10 MB for an 84- or 332-module station,
// with no frame-rate cost (models are instanced). The toggle turns them off.
let showTextures = (() => {
  try {
    return localStorage.getItem("x4sd.textures") !== "off";
  } catch {
    return true;
  }
})();
let quality: Quality = (() => {
  try {
    return (localStorage.getItem("x4sd.quality") as Quality | null) ?? "normal";
  } catch {
    return "normal";
  }
})();
let modules: ModuleInfo[] = [];
let scene: Scene | null = null;
/** The player profile folder (from the catalog), for the plan file dialogs. */
let profileDir: string | null = null;
let selected: number | null = null;
let armed: ModuleInfo | null = null;
/** Module being moved onto another snap (with everything attached to it), if any. */
let moving: number | null = null;
/** While placing or moving: the free snap under the cursor, previewed there. */
let hoverTarget: { index: number; snap: string } | null = null;
/** Build-order preview: show modules up to this step (null: all). */
let buildStep: number | null = null;
/** Races shown in the palette; empty means all. Modules without a race count as "generic". */
const races = new Set<string>(
  (() => {
    try {
      return JSON.parse(localStorage.getItem("x4sd.races") ?? "[]") as string[];
    } catch {
      return [];
    }
  })(),
);
const raceOf = (m: ModuleInfo) => m.race ?? "generic";
/** The player's blueprints from a save, remembered (D-16). Off by default: every module. */
let blueprints: Blueprints = (() => {
  try {
    return (JSON.parse(localStorage.getItem("x4sd.blueprints") ?? "null") as Blueprints | null) ?? { on: false, save: null, read_at: null, modules: [] };
  } catch {
    return { on: false, save: null, read_at: null, modules: [] };
  }
})();
const ownedSet = () => (blueprints.on && blueprints.save ? new Set(blueprints.modules.map((m) => m.toLowerCase())) : null);
let owned = ownedSet();
/** The modules on offer: those with a blueprint while the limit is on, else all. */
const offered = () => (owned ? modules.filter((m) => owned!.has(m.macro.toLowerCase())) : modules);

/** The game's build-menu categories, in its order (`leftBar` in menu_station_configuration.lua),
 * with the macro classes each one holds (radar is filed under Other by the menu). */
const CATEGORIES = [
  { name: "Production Modules", classes: ["production"] },
  { name: "Build Modules", classes: ["buildmodule"] },
  { name: "Storage Modules", classes: ["storage"] },
  { name: "Habitation Modules", classes: ["habitation"] },
  { name: "Welfare Modules", classes: ["welfaremodule"] },
  { name: "Dock Modules", classes: ["dockarea", "pier"] },
  { name: "Defence Modules", classes: ["defencemodule"] },
  { name: "Processing Modules", classes: ["processingmodule"] },
  { name: "Other Modules", classes: ["connectionmodule", "radar"] },
  { name: "Venture Modules", classes: ["ventureplatform"] },
];
/** Position of a module's category in the game's order (unknown classes last). */
const categoryOf = (m: ModuleInfo) => {
  const i = CATEGORIES.findIndex((c) => c.classes.includes(m.class));
  return i < 0 ? CATEGORIES.length : i;
};

// ---------------------------------------------------------------- helpers

function status(text: string, kind: "" | "error" | "good" = "") {
  const el = $("status");
  el.textContent = text;
  el.className = kind;
}

async function shapeOf(macro: string): Promise<Shape> {
  const key = macro.toLowerCase();
  let shape = shapes.get(key);
  if (!shape) {
    shape = await api.moduleShape(macro);
    shapes.set(key, shape);
  }
  return shape;
}

async function modelOf(macro: string) {
  const key = macro.toLowerCase();
  if (models.has(key)) return;
  try {
    models.set(key, modelGeometry(await api.moduleModel(macro, quality)));
  } catch {
    models.set(key, null);
  }
}

/** Loads the diffuse textures of every material the scene's models use. */
async function loadTextures() {
  const names = new Set<string>();
  for (const e of scene?.entries ?? []) {
    for (const n of models.get(e.macro.toLowerCase())?.userData.materials ?? []) names.add(n.toLowerCase());
  }
  const missing = [...names].filter((n) => !textures.has(n));
  if (missing.length === 0) return;
  const started = performance.now();
  status(`Loading ${missing.length} textures…`);
  await Promise.all(
    missing.map(async (n) => {
      try {
        const maps = await api.materialMaps(n);
        textures.set(n, maps.diffuse ? view.diffuseTexture(await api.texture(maps.diffuse)) : null);
      } catch {
        textures.set(n, null);
      }
    }),
  );
  const loaded = [...textures.values()].filter((t) => t);
  const mb = loaded.reduce((sum, t) => sum + (t?.bytes ?? 0), 0) / 1048576;
  status(`Textures: ${loaded.length} loaded (${mb.toFixed(1)} MB GPU memory) in ${((performance.now() - started) / 1000).toFixed(1)} s`, "good");
}

/** Runs a command that returns the new scene, then redraws. Errors go to the status bar. */
async function apply(action: Promise<Scene>, done?: string, frame = false, userEdit = false) {
  try {
    const fitted = scene?.plot?.fits ?? true;
    scene = await action;
    const macros = [...new Set(scene.entries.map((e) => e.macro))];
    if (macros.some((x) => !models.has(x.toLowerCase()))) status("Loading models…");
    await Promise.all(macros.flatMap((x) => [shapeOf(x), modelOf(x)]));
    if (showTextures) await loadTextures();
    if (scene.focus) selected = scene.focus;
    if (selected !== null && !scene.entries.some((e) => e.index === selected)) selected = null;
    redraw();
    if (frame) view.frame();
    if (done) status(done, "good");
    if (userEdit && fitted && scene.plot && !scene.plot.fits) await offerBiggerPlot(scene.plot);
  } catch (e) {
    status(String(e), "error");
  }
}

/** The Checks list and the plot summary (also live while a gizmo drag is in progress). */
function renderChecks(list: Issue[], p: PlotStatus | null) {
  const issueCount = $("issue-count");
  issueCount.textContent = list.length ? `(${list.length})` : "";
  issueCount.className = list.length ? "bad" : "";
  const issues = $("issues");
  issues.replaceChildren();
  if (list.length === 0) {
    const li = document.createElement("li");
    li.className = "ok";
    li.textContent = scene?.entries.length ? "No problems found" : "Empty design";
    issues.append(li);
  }
  for (const issue of list) {
    const li = document.createElement("li");
    li.textContent = issue.text;
    if (issue.index !== null) li.onclick = () => select(issue.index);
    issues.append(li);
  }
  const summary = $("plot-summary");
  summary.textContent = p?.plot
    ? `Plot ${p.plot.size.join("×")} km · station needs ${p.required ? p.required.size.join("×") + " km" : "nothing yet"}` + (p.possible ? "" : " (more than any plot)")
    : "No plot";
  summary.className = "details" + (p && !p.fits ? " bad" : "");
}

// Live checks while the gizmo is dragged: one request at a time, the latest position wins.
let previewWant: { index: number; position: Vec3; rotation: Quat } | null = null;
let previewSent = "";
let previewBusy = false;
async function previewGizmo() {
  if (previewBusy) return;
  previewBusy = true;
  while (previewWant) {
    const want = previewWant;
    previewWant = null;
    try {
      const r = await api.previewGizmo(want.index, want.position, want.rotation);
      // A late answer after the drag ended would hide the real checks.
      if (view.gizmoDragging()) {
        renderChecks(r.issues, r.plot);
        view.setPlotFits(r.plot?.fits ?? true);
      }
    } catch {
      // The last checks shown stay; the real ones come when the drag ends.
    }
  }
  previewBusy = false;
}
view.onGizmoDrag = (position, rotation) => {
  const key = JSON.stringify([selected, position, rotation]);
  if (selected === null || key === previewSent) return;
  previewSent = key;
  previewWant = { index: selected, position, rotation };
  void previewGizmo();
};

/** The last edit pushed the station outside its plot: offer to enlarge it (or undo). */
async function offerBiggerPlot(p: NonNullable<Scene["plot"]>) {
  const need = p.required?.text ?? "?";
  if (!p.possible) {
    if (await ask(`The station no longer fits any plot: it needs ${need}, and plots are at most 20 km per axis.\n\nUndo the last change?`, { title: "Plot", kind: "warning" })) {
      await apply(api.undo(), "Undone: the station has to fit a 20 km plot");
    }
    return;
  }
  if (await ask(`The station no longer fits the plot (${p.plot?.text}).\nIt needs ${need}.\n\nEnlarge the plot?`, { title: "Plot", kind: "warning" })) {
    await apply(api.fitPlot(true), "Plot enlarged");
  }
}

function entry(index: number | null) {
  return scene?.entries.find((e) => e.index === index) ?? null;
}

function moduleName(macro: string) {
  return modules.find((m) => m.macro.toLowerCase() === macro.toLowerCase())?.name ?? macro;
}

// ---------------------------------------------------------------- drawing

function redraw() {
  if (!scene) return;
  if (buildStep !== null && buildStep >= scene.entries.length) buildStep = null;
  view.render(scene, shapes, models, showTextures ? textures : null, selected, armed !== null || moving !== null, buildStep, moving !== null ? subtree(moving) : null);
  hoverTarget = null; // the cones were redrawn
  refreshPlacing();
  renderOrder();
  void refreshCost();
  void refreshAnalysis();
  void refreshLoad();
  void refreshEquipment();
  renderEquipmentSummary();
  renderPlanner();
  $("dirty").hidden = !scene.dirty;
  const title = `X4 Station Designer - ${scene.name}${scene.dirty ? " *" : ""}`;
  if (title !== windowTitle) {
    windowTitle = title;
    void getCurrentWindow().setTitle(title).catch(() => undefined);
  }
  // The selected module's gizmo: a free-standing one moves or rotates; an attached
  // one rotates about its joint (where its snap meets the other's), staying attached.
  const root = entry(selected);
  const editable = root && !armed && moving === null && buildStep === null;
  // On the module itself, as in the game (an attached one still turns about its joint).
  const gizmoAt = !editable ? null : !root.attached || gizmoMode === "rotate" ? root.position : null;
  view.setGizmo(
    gizmoAt ? { position: gizmoAt, rotation: root!.rotation, joint: root!.attached?.joint ?? null, carries: subtree(root!.index) } : null,
    root
      ? (p, q) => {
          const name = root.name ?? root.macro;
          if (root.attached) void apply(api.rotateAttached(root.index, q), `Turned ${name} about its joint`, false, true);
          else void apply(api.moveRoot(root.index, p, q), `${gizmoMode === "rotate" ? "Rotated" : "Moved"} ${name}`, false, true);
        }
      : null,
    root?.attached ? "rotate" : gizmoMode,
  );
  // The button stays while placing: it drops the placement (see its onclick).
  $("gizmo-row").hidden = !(root && moving === null && buildStep === null);
  $("btn-gizmo-mode").textContent = !gizmoAt || (gizmoMode !== "rotate" && !root?.attached) ? "Rotate gizmo" : root?.attached ? "Hide gizmo" : "Move gizmo";
  $("btn-straighten").hidden = !(root?.attached && (root.attached.tilt ?? 0) > 0.05);
  $("btn-twist-left").textContent = `⟲ ${rotationStep}°`;
  $("btn-twist-right").textContent = `⟳ ${rotationStep}°`;
  $<HTMLInputElement>("design-name").value = scene.name;
  const count = `${scene.entries.length} module${scene.entries.length === 1 ? "" : "s"}`;
  $("patches").textContent = scene.patches.length ? `${count} · Needs: ${scene.patches.join(", ")}` : count;
  $<HTMLButtonElement>("btn-undo").disabled = !scene.can_undo;
  $<HTMLButtonElement>("btn-redo").disabled = !scene.can_redo;

  renderChecks(scene.issues, scene.plot);

  const sel = entry(selected);
  $("selection-panel").hidden = !sel;
  if (sel) {
    $("sel-name").textContent = sel.name ?? sel.macro;
    const a = sel.attached;
    $("sel-details").textContent =
      `#${sel.index} · ${sel.macro}\n` +
      (a ? `attached to #${a.to} (${a.to_snap}) by ${a.snap}, twist ${a.twist ?? "?"}°${a.tilt ? `, tilted ${a.tilt}°` : ""}` : "not attached (root)") +
      `\n${sel.free_snaps.length} free snap(s)`;
    $<HTMLButtonElement>("btn-twist-left").disabled = !a;
    $<HTMLButtonElement>("btn-twist-right").disabled = !a;
    $<HTMLButtonElement>("btn-detach").disabled = !a;
    $("sel-snap-row").hidden = !a;
    $("sel-position").hidden = !!a;
    document.querySelectorAll<HTMLInputElement>("#sel-position input").forEach((input) => {
      if (document.activeElement !== input) input.value = String(Math.round(sel.position[Number(input.dataset.axis)] * 100) / 100);
    });
    if (a) {
      $<HTMLSelectElement>("sel-snap").replaceChildren(...connectableSnaps(sel).map(snapOption));
      $<HTMLSelectElement>("sel-snap").value = a.snap;
    }
    $<HTMLButtonElement>("btn-move").disabled = (scene.entries.length ?? 0) < 2;
    $<HTMLInputElement>("stage-end").checked = sel.stage_end;
  }
  $("btn-place-root").hidden = scene.entries.length > 0;

  const p = scene.plot;
  document.querySelectorAll<HTMLInputElement>(".plot-grid input").forEach((input) => {
    const side = input.dataset.side as "pos" | "neg";
    if (document.activeElement !== input) input.value = String(p?.plot?.[side][Number(input.dataset.axis)] ?? "");
  });
}

/** Sends the six plot fields; the core rejects axes outside 1..20 km. */
function plotFromInputs(): Promise<Scene> {
  const pos = [0, 0, 0] as [number, number, number];
  const neg = [0, 0, 0] as [number, number, number];
  document.querySelectorAll<HTMLInputElement>(".plot-grid input").forEach((input) => {
    const target = input.dataset.side === "pos" ? pos : neg;
    target[Number(input.dataset.axis)] = Math.max(0, Math.round(Number(input.value) || 0));
  });
  return api.setPlot(pos, neg);
}

function renderPalette() {
  const text = $<HTMLInputElement>("search").value.toLowerCase();
  const category = $<HTMLSelectElement>("class-filter").value;
  const all = offered();
  const shown = all.filter(
    (m) =>
      (!category || categoryOf(m) === Number(category)) &&
      (races.size === 0 || races.has(raceOf(m))) &&
      (!text || m.name.toLowerCase().includes(text) || m.macro.toLowerCase().includes(text)),
  );
  const list = $("module-list");
  list.replaceChildren(
    ...shown.map((m) => {
      const div = document.createElement("div");
      div.className = "module" + (armed?.macro === m.macro ? " armed" : "");
      div.draggable = true;
      const color = (CLASS_COLORS[m.class] ?? 0xffffff).toString(16).padStart(6, "0");
      div.innerHTML = `<div class="n"><span class="swatch" style="background:#${color}"></span></div><div class="m"></div>`;
      div.querySelector(".n")!.append(m.name);
      div.querySelector(".m")!.textContent = `${m.macro} · ${m.class}${m.source ? " · " + m.source : ""}`;
      div.onclick = () => arm(armed?.macro === m.macro ? null : m);
      div.ondragstart = (ev) => {
        ev.dataTransfer!.setData("text/x4sd-macro", m.macro);
        void arm(m);
      };
      return div;
    }),
  );
  $("module-count").textContent = `${shown.length} of ${all.length} modules${owned ? " with blueprints" : ""}`;
}

/** The build order list: one row per module in build order, stage dividers, drag to reorder. */
function renderOrder() {
  if (!scene) return;
  const total = scene.entries.length;
  const slider = $<HTMLInputElement>("build-step");
  slider.max = String(total);
  slider.value = String(buildStep ?? total);
  $("build-step-label").textContent = `${buildStep ?? total} / ${total}`;
  const rows: HTMLElement[] = [];
  let stage = 1;
  for (const e of scene.entries) {
    const row = document.createElement("div");
    row.className = "order-row" + (e.index === selected ? " selected" : "") + (buildStep !== null && e.index > buildStep ? " later" : "");
    row.draggable = true;
    row.innerHTML = `<span class="num"></span><span class="n"></span>`;
    row.querySelector(".num")!.textContent = `${e.index}.`;
    row.querySelector(".n")!.textContent = e.name ?? e.macro;
    row.title = e.attached ? `attaches to #${e.attached.to}` : "not attached (root)";
    // Double click zooms to the module. No dblclick event: the first click redraws the rows.
    row.onclick = (ev) => {
      select(e.index);
      if (ev.detail === 2) view.frameModule(e.index);
    };
    row.ondragstart = (ev) => ev.dataTransfer!.setData("text/x4sd-entry", String(e.index));
    row.ondragover = (ev) => {
      if (ev.dataTransfer?.types.includes("text/x4sd-entry")) {
        ev.preventDefault();
        row.classList.add("drop");
      }
    };
    row.ondragleave = () => row.classList.remove("drop");
    row.ondrop = (ev) => {
      ev.preventDefault();
      row.classList.remove("drop");
      const moved = Number(ev.dataTransfer?.getData("text/x4sd-entry"));
      if (moved && moved !== e.index) void apply(api.moveInBuildOrder(moved, e.index), `Moved #${moved} to step ${e.index}`);
    };
    rows.push(row);
    if (e.stage_end) {
      const divider = document.createElement("div");
      divider.className = "stage-divider";
      divider.textContent = `End of stage ${stage++}`;
      rows.push(divider);
    }
  }
  $("order-list").replaceChildren(...rows);
}

function renderRaceFilter() {
  const counts = new Map<string, number>();
  offered().forEach((m) => counts.set(raceOf(m), (counts.get(raceOf(m)) ?? 0) + 1));
  const chip = (label: string, on: boolean, title: string, click: () => void) => {
    const b = document.createElement("button");
    b.textContent = label;
    b.title = title;
    b.className = on ? "on" : "";
    b.onclick = click;
    return b;
  };
  const toggle = (race: string | null) => {
    if (race === null) races.clear();
    else if (!races.delete(race)) races.add(race);
    try {
      localStorage.setItem("x4sd.races", JSON.stringify([...races]));
    } catch {
      // Remembering the choice is a convenience only.
    }
    renderRaceFilter();
    renderPalette();
  };
  $("race-filter").replaceChildren(
    chip("All", races.size === 0, "Show every race", () => toggle(null)),
    ...[...counts.keys()].sort().map((r) =>
      chip(r[0].toUpperCase() + r.slice(1), races.has(r), `${counts.get(r)} ${r} modules; click to add or remove`, () => toggle(r)),
    ),
  );
}

let windowTitle = "";

/** Build method for costs (the game's Preferred Build Method), remembered. */
let buildMethod = (() => {
  try {
    return localStorage.getItem("x4sd.method") ?? "default";
  } catch {
    return "default";
  }
})();
let costRequest = 0;

/** Right pane: what the selected module (with its equipment) or the whole station needs. */
async function refreshCost() {
  if (!scene) return;
  const request = ++costRequest;
  const sel = entry(selected);
  let cost;
  try {
    cost = await api.buildCost(sel?.index ?? null, buildMethod);
  } catch (e) {
    $("cost-total").textContent = String(e);
    return;
  }
  if (request !== costRequest) return; // a newer request is on its way
  $("cost-title").textContent = sel ? `Build cost: ${sel.name ?? sel.macro}` : "Build cost: whole station";
  const methodSelect = $<HTMLSelectElement>("build-method");
  if (methodSelect.options.length !== cost.methods.length) {
    methodSelect.replaceChildren(...cost.methods.map((m) => new Option(m.name, m.id)));
  }
  methodSelect.value = buildMethod;
  const n = (x: number) => Math.round(x).toLocaleString("en-US");
  $("cost-lines").replaceChildren(
    ...cost.lines.map((l) => {
      const row = document.createElement("tr");
      row.innerHTML = `<td></td><td class="n"></td><td class="v"></td>`;
      row.children[0].textContent = l.name;
      row.children[1].textContent = n(l.amount);
      row.children[2].textContent = l.value === null ? "" : `${n(l.value)} Cr`;
      return row;
    }),
  );
  $("cost-total").textContent = scene.entries.length
    ? `≈ ${n(cost.value)} Cr at average prices` + (cost.unknown.length ? ` (no recipe: ${cost.unknown.join(", ")})` : "")
    : "Nothing placed yet";
}

/** Sector sunlight for the production figures, in percent; remembered. */
let sunlight = (() => {
  try {
    return Number(localStorage.getItem("x4sd.sunlight") ?? "100") || 100;
  } catch {
    return 100;
  }
})();
let analyzed: { scene: Scene; sunlight: number } | null = null;

/** Right pane: what the whole station makes and uses per hour, its workforce, storage and docks. */
async function refreshAnalysis() {
  if (!scene || (analyzed?.scene === scene && analyzed.sunlight === sunlight)) return;
  const request = { scene, sunlight };
  analyzed = request;
  let a;
  try {
    a = await api.analyzeStation(sunlight);
  } catch (e) {
    $("economy-summary").textContent = String(e);
    return;
  }
  if (analyzed !== request) return; // a newer request is on its way
  const n = (x: number) => Math.round(x).toLocaleString("en-US");
  const w = a.workforce;
  const races = w.by_race.map((r) => `${r.race} ${n(r.capacity)}`).join(", ");
  $("economy-summary").textContent = scene.entries.length
    ? `Workforce: ${n(w.housed)} housed${races ? ` (${races})` : ""}, ${n(w.needed)} needed` +
      (w.needed ? ` · bonus ${Math.round(w.fill * 100)}%` : "") +
      `\nDocks: ${a.dock_areas} dock area${a.dock_areas === 1 ? "" : "s"}, ${a.piers} pier${a.piers === 1 ? "" : "s"}` +
      (a.flows.length ? "\nNet per hour:" : "")
    : "Nothing placed yet";
  $("economy-flows").replaceChildren(
    ...a.flows
      .filter((f) => Math.abs(f.net) >= 0.5)
      .map((f) => {
        const row = document.createElement("tr");
        row.innerHTML = `<td></td><td class="n"></td>`;
        row.title = `Made ${n(f.produced)}, used ${n(f.used)} per hour`;
        row.children[0].textContent = f.raw ? `${f.name} (raw)` : f.name;
        row.children[1].textContent = (f.net > 0 ? "+" : "") + n(f.net);
        row.children[1].className = `n ${f.net > 0 ? "up" : "down"}`;
        return row;
      }),
  );
  $("economy-storage").textContent = a.storage.length
    ? "Storage: " +
      a.storage
        .map((s) => `${s.transport} ${n(s.capacity)} m³` + (s.hours === null ? "" : ` (≈ ${s.hours < 10 ? s.hours.toFixed(1) : n(s.hours)} h)`))
        .join(" · ")
    : "";
  $("economy-notes").replaceChildren(
    ...a.notes.map((t) => {
      const li = document.createElement("li");
      li.textContent = t;
      return li;
    }),
  );
}

interface StationLoad { modules: number; connectors: number; turrets: number; shields: number; docks: number; size_km: [number, number, number] }
let loadRequest = 0;

/** Right pane: what likely costs the game frame rate (players' rules of thumb). */
async function refreshLoad() {
  if (!scene) return;
  const request = ++loadRequest;
  let l;
  try {
    l = (await api.runOp<StationLoad>({ op: "station_load" })).result;
  } catch (e) {
    $("load-summary").textContent = String(e);
    return;
  }
  if (request !== loadRequest) return; // a newer request is on its way
  const km = l.size_km.map((v) => v.toFixed(1)).join(" × ");
  $("load-summary").textContent = l.modules
    ? `Modules: ${l.modules} (${l.connectors} connection structure${l.connectors === 1 ? "" : "s"})
Turrets: ${l.turrets} · Shields: ${l.shields}
Docks and piers: ${l.docks}
Size: ${km} km`
    : "Nothing placed yet";
}

// ---------------------------------------------------------------- right pane

// A section's title folds it away or opens it; most start folded (see index.html).
for (const title of document.querySelectorAll<HTMLElement>("#inspector section > h3, #sel-equipment > h4")) {
  title.onclick = () => title.parentElement!.classList.toggle("collapsed");
}

// ---------------------------------------------------------------- equipment

let equipmentList: EquipmentItem[] = [];
let equipRequest = 0;
const SIZE = (s: string | null) => (s === "medium" ? "M" : s === "large" ? "L" : (s ?? "?"));
const KIND = { turrets: "turret", shields: "shield" } as const;
const equipmentName = (m: string) => equipmentList.find((e) => e.macro.toLowerCase() === m.toLowerCase())?.name ?? m;

/** Runs a shared operation that may change the design and redraws; returns its result (null after an error). */
async function applyOp<T>(op: { op: string } & Record<string, unknown>, done?: (result: T) => string): Promise<T | null> {
  let result: T | null = null;
  await apply(
    api.runOp<T>(op).then((r) => {
      result = r.result;
      return r.scene ?? scene!;
    }),
  );
  if (result !== null && done) status(done(result), "good");
  return result;
}

/** Selection panel: the selected module's groups, each kind with a choice of what fits and a count. */
async function refreshEquipment() {
  const sel = entry(selected);
  const box = $("sel-equipment");
  if (!sel || !sel.equipment[1]) {
    box.hidden = true;
    return;
  }
  const request = ++equipRequest;
  let groups: EquipmentGroup[];
  try {
    groups = (await api.runOp<{ groups: EquipmentGroup[] }>({ op: "module_equipment", index: sel.index })).result.groups;
  } catch {
    return;
  }
  if (request !== equipRequest) return; // a newer request is on its way
  box.hidden = false;
  const rows: HTMLElement[] = [];
  let last = "";
  for (const g of groups) {
    if (g.group !== last) {
      last = g.group;
      const title = document.createElement("div");
      title.className = "equip-group";
      const name = g.group.replace(/^group0*(\d+)$/, "Group $1").replace(/_/g, " ");
      title.textContent = name[0].toUpperCase() + name.slice(1);
      rows.push(title);
    }
    const row = document.createElement("div");
    row.className = "equip-row";
    const label = document.createElement("span");
    label.textContent = `${g.slots} × ${SIZE(g.size)} ${KIND[g.kind]}`;
    const choice = document.createElement("select");
    choice.append(new Option("—", ""), ...g.fits.map((m) => new Option(equipmentName(m), m)));
    choice.value = g.current?.macro ?? "";
    const count = document.createElement("input");
    count.type = "number";
    count.min = "1";
    count.max = String(g.slots);
    count.value = String(g.current?.count ?? g.slots);
    count.title = `How many of the ${g.slots} slots to fill`;
    count.hidden = g.slots < 2;
    const set = () =>
      void applyOp({ op: "set_equipment", index: sel.index, group: g.group, kind: g.kind, macro: choice.value || null, count: choice.value ? Number(count.value) : null });
    choice.onchange = set;
    count.onchange = () => choice.value && set();
    row.append(label, choice, count);
    rows.push(row);
  }
  $("sel-equipment-groups").replaceChildren(...rows);
}

/** Right pane: how many groups of the station are equipped. */
function renderEquipmentSummary() {
  const [done, total] = (scene?.entries ?? []).reduce(([a, b], e) => [a + e.equipment[0], b + e.equipment[1]], [0, 0]);
  $("equip-summary").textContent = total ? `${done} of ${total} turret and shield groups equipped` : "No module here has turret or shield slots";
}

/** What equip_modules reports, for the status bar. */
const equipped = (r: { filled: number; cleared: number; skipped: number }) =>
  [r.filled && `Equipped ${r.filled} groups`, r.cleared && `cleared ${r.cleared}`, r.skipped && `${r.skipped} left as they were (nothing fits)`].filter(Boolean).join(", ") ||
  "Nothing to equip";

async function loadEquipment() {
  equipmentList = (await api.runOp<EquipmentItem[]>({ op: "list_equipment" })).result;
  for (const [kind, size] of [["turrets", "medium"], ["shields", "medium"], ["turrets", "large"], ["shields", "large"]]) {
    const items = equipmentList.filter((e) => e.kind === kind && e.size === size); // sorted by race, name, macro
    // Two items with the same name (rare) show their macro too.
    const label = (e: EquipmentItem) => (items.filter((x) => x.name === e.name).length > 1 ? `${e.name} (${e.macro})` : e.name);
    $(`pick-${kind}-${size}`).replaceChildren(new Option("—", ""), ...items.map((e) => new Option(label(e), e.macro)));
  }
}

const defenceOnly = () => $<HTMLInputElement>("equip-defence-only").checked;
$("btn-equip-level").onclick = () =>
  void applyOp({ op: "equip_modules", level: $<HTMLSelectElement>("equip-level").value, defence_only: defenceOnly() }, equipped);
$("btn-equip-picks").onclick = () => {
  const picks = ["turrets-medium", "shields-medium", "turrets-large", "shields-large"].map((id) => $<HTMLSelectElement>(`pick-${id}`).value).filter(Boolean);
  if (!picks.length) {
    status("Pick at least one turret or shield", "error");
    return;
  }
  void applyOp({ op: "equip_modules", picks, defence_only: defenceOnly() }, equipped);
};
$("btn-equip-same").onclick = () => {
  const sel = entry(selected);
  if (sel) void applyOp<{ modules: number }>({ op: "copy_equipment", from: sel.index, scope: "same_type" }, (r) => `Copied to ${r.modules} more ${sel.name ?? sel.macro}`);
};
$("btn-equip-all").onclick = () => {
  const sel = entry(selected);
  if (sel) void applyOp({ op: "copy_equipment", from: sel.index, scope: "all" }, equipped);
};

// ---------------------------------------------------------------- planner

/** `extras`: other modules to add, with the sides of the station to put them on
 * (defence platforms); `storage`: m³ per cargo type the station has at least. */
interface PlannerState {
  targets: { ware: string; modules: number }[];
  extras: { macro: string; count: number; sides: string[] }[];
  race: string;
  workforce: boolean;
  hours: number;
  storage: Record<string, number>;
  buy: string[];
  fewerConnectors: boolean;
}
let planner: PlannerState = (() => {
  const fallback: PlannerState = { targets: [], extras: [], race: "", workforce: true, hours: 6, storage: {}, buy: [], fewerConnectors: false };
  try {
    return { ...fallback, ...JSON.parse(localStorage.getItem("x4sd.planner") ?? "{}") };
  } catch {
    return fallback;
  }
})();
let wareNames = new Map<string, string>();
/** The last plan, and how many of each macro the design had when it was made.
 * `optimize`: an Optimize plan (the shortfall fixes), applied with Apply plan;
 * `dropped` holds the macros of rows the user removed. */
let plan: { result: ProductionPlan; baseline: Map<string, number>; optimize?: { dropped: Set<string> } } | null = null;

/** An Optimize plan's modules, without the rows the user removed. */
function optimizeModules() {
  const dropped = plan?.optimize?.dropped;
  return (plan?.result.modules ?? []).filter((m) => !dropped?.has(m.macro.toLowerCase()));
}

function savePlanner() {
  try {
    localStorage.setItem("x4sd.planner", JSON.stringify(planner));
  } catch {
    // Remembering the planner is a convenience only.
  }
}

function macroCounts() {
  const counts = new Map<string, number>();
  for (const e of scene?.entries ?? []) counts.set(e.macro.toLowerCase(), (counts.get(e.macro.toLowerCase()) ?? 0) + 1);
  return counts;
}

/** How many of a planned module the design gained since the plan was made (`now`: `macroCounts()`). */
function placedOf(m: PlannedModule, now: Map<string, number>) {
  const key = m.macro.toLowerCase();
  return Math.min(m.count, Math.max(0, (now.get(key) ?? 0) - (plan?.baseline.get(key) ?? 0)));
}

/** Sides of the station (the game's axes): id, label, axis. */
const SIDES: [string, string, string][] = [
  ["front", "Front", "+Z"],
  ["back", "Back", "−Z"],
  ["left", "Left", "−X"],
  ["right", "Right", "+X"],
  ["top", "Top", "+Y"],
  ["bottom", "Bottom", "−Y"],
];

function renderPlanner() {
  const n = (x: number) => Math.round(x).toLocaleString("en-US");
  $("planner-targets").replaceChildren(
    ...planner.targets.map((t, i) => {
      const li = document.createElement("li");
      li.innerHTML = `<span></span><input type="number" min="1" /><button title="Remove">✕</button>`;
      li.querySelector("span")!.textContent = wareNames.get(t.ware) ?? t.ware;
      const count = li.querySelector("input")!;
      count.value = String(t.modules);
      count.onchange = () => {
        t.modules = Math.max(1, Math.round(Number(count.value)) || 1);
        savePlanner();
      };
      li.querySelector("button")!.onclick = () => {
        planner.targets.splice(i, 1);
        savePlanner();
        renderPlanner();
      };
      return li;
    }),
  );
  $("planner-extras").replaceChildren(
    ...planner.extras.map((x, i) => {
      const li = document.createElement("li");
      li.innerHTML = `<div><span></span><input type="number" min="1" /><button title="Remove">✕</button></div>`;
      li.querySelector("span")!.textContent = moduleName(x.macro);
      const count = li.querySelector("input")!;
      count.value = String(x.count);
      count.onchange = () => {
        x.count = Math.max(1, Math.round(Number(count.value)) || 1);
        savePlanner();
      };
      li.querySelector("button")!.onclick = () => {
        planner.extras.splice(i, 1);
        savePlanner();
        renderPlanner();
      };
      if (modules.find((m) => m.macro.toLowerCase() === x.macro.toLowerCase())?.class === "defencemodule") {
        const chips = document.createElement("div");
        chips.className = "chips";
        for (const [side, label, axis] of SIDES) {
          const b = document.createElement("button");
          b.textContent = label;
          b.title = `On the outside of the station on its ${label.toLowerCase()} (${axis}) side, nothing in front of it. The count is spread evenly over the sides picked; none: anywhere`;
          b.classList.toggle("on", x.sides.includes(side));
          b.onclick = () => {
            const on = x.sides.includes(side) ? x.sides.filter((s) => s !== side) : [...x.sides, side];
            x.sides = SIDES.map((s) => s[0]).filter((s) => on.includes(s));
            savePlanner();
            renderPlanner();
          };
          chips.append(b);
        }
        li.append(chips);
      }
      return li;
    }),
  );
  $("planner-buy").replaceChildren(
    ...planner.buy.map((w) => {
      const b = document.createElement("button");
      b.textContent = `Buy ${wareNames.get(w) ?? w} ✕`;
      b.title = "Bought instead of made; click to make it again";
      b.onclick = () => {
        planner.buy = planner.buy.filter((x) => x !== w);
        savePlanner();
        void generatePlan(false);
      };
      return b;
    }),
  );
  if (!plan) return;
  const { result } = plan;
  if (plan.optimize) {
    renderOptimizePlan();
    return;
  }
  $("planner-apply").hidden = true;
  const now = macroCounts();
  const groups: [PlannedModule["role"], string][] = [["target", "Targets"], ["extra", "Other modules"], ["chain", "Production chain"], ["habitat", "Habitats"], ["storage", "Storage"], ["dock", "Docks"]];
  let left = 0;
  const rows: HTMLElement[] = [];
  for (const [role, title] of groups) {
    const members = result.modules.filter((m) => m.role === role);
    if (!members.length) continue;
    const h = document.createElement("h4");
    h.textContent = title;
    rows.push(h);
    for (const m of members) {
      const placed = placedOf(m, now);
      left += m.count - placed;
      const row = document.createElement("div");
      row.className = "planned-row" + (placed >= m.count ? " done" : "");
      row.title = `${m.macro}: click to place one`;
      row.innerHTML = `<span class="name"></span><span class="n"></span>`;
      row.querySelector(".name")!.textContent = m.name;
      row.querySelector(".n")!.textContent = `${placed}/${m.count}`;
      if (role === "chain" && m.ware) {
        const buy = document.createElement("button");
        buy.textContent = "Buy";
        buy.title = `Buy ${wareNames.get(m.ware) ?? m.ware} instead of making it`;
        buy.onclick = (ev) => {
          ev.stopPropagation();
          planner.buy.push(m.ware!);
          savePlanner();
          void generatePlan(false);
        };
        row.append(buy);
      }
      row.onclick = () => {
        const info = modules.find((x) => x.macro.toLowerCase() === m.macro.toLowerCase());
        if (info) void arm(info);
      };
      rows.push(row);
    }
  }
  $("planner-result").replaceChildren(...rows);
  $("planner-place").hidden = left === 0;
  $("planner-place").textContent = `Place all (${left})`;
  const total = result.modules.reduce((s, m) => s + m.count, 0);
  const short = (raw: boolean) =>
    result.analysis.flows
      .filter((f) => f.net < -0.5 && f.raw === raw)
      .map((f) => `${f.name} ${n(-f.net)}`)
      .join(", ");
  $("planner-summary").textContent =
    (total ? `${total} modules to add, ${left} still to place · ≈ ${n(result.cost.value)} Cr` : "Nothing to add: the station covers its needs") +
    (result.analysis.self_sufficient ? " · self-sufficient" : "") +
    (short(true) ? `\nRaw materials to bring in per hour: ${short(true)}` : "") +
    (short(false) ? `\nTo buy per hour: ${short(false)}` : "") +
    (result.unknown_targets.length ? `\nNo module makes: ${result.unknown_targets.join(", ")}` : "") +
    (result.no_blueprint.length ? `\nNo blueprint for a module that makes: ${result.no_blueprint.map((w) => wareNames.get(w) ?? w).join(", ")}` : "");
}

/** An Optimize plan: the modules it adds (chain rows can be bought instead, the
 * others removed) and an Apply plan button. */
function renderOptimizePlan() {
  const n = (x: number) => Math.round(x).toLocaleString("en-US");
  const { result, optimize } = plan!;
  const kept = optimizeModules();
  const groups: [PlannedModule["role"], string][] = [["chain", "Production chain"], ["habitat", "Habitats"], ["storage", "Storage"], ["dock", "Docks"]];
  const rows: HTMLElement[] = [];
  for (const [role, title] of groups) {
    const members = kept.filter((m) => m.role === role);
    if (!members.length) continue;
    const h = document.createElement("h4");
    h.textContent = title;
    rows.push(h);
    for (const m of members) {
      const row = document.createElement("div");
      row.className = "planned-row";
      row.title = m.macro;
      row.innerHTML = `<span class="name"></span><span class="n"></span><button></button>`;
      row.querySelector(".name")!.textContent = m.name;
      row.querySelector(".n")!.textContent = `+${m.count}`;
      const b = row.querySelector("button")!;
      if (role === "chain" && m.ware) {
        b.textContent = "Buy";
        b.title = `Buy ${wareNames.get(m.ware) ?? m.ware} instead of making it (modules that only fed it drop out too)`;
        b.onclick = () => {
          planner.buy.push(m.ware!);
          savePlanner();
          void generatePlan(false);
        };
      } else {
        b.textContent = "✕";
        b.title = "Don't add these";
        b.onclick = () => {
          optimize!.dropped.add(m.macro.toLowerCase());
          renderPlanner();
        };
      }
      rows.push(row);
    }
  }
  $("planner-result").replaceChildren(...rows);
  $("planner-place").hidden = true;
  $("planner-apply").hidden = false;
  const total = kept.reduce((s, m) => s + m.count, 0);
  const rebuild = $<HTMLInputElement>("optimize-rebuild").checked;
  $<HTMLButtonElement>("planner-apply").disabled = !total && !rebuild;
  $("planner-apply").textContent = total ? `Apply plan (+${total})` : rebuild ? "Apply plan (re-layout only)" : "Apply plan";
  const short = result.analysis.flows.filter((f) => f.net < -0.5 && !f.raw).map((f) => `${f.name} ${n(-f.net)}`).join(", ");
  $("planner-summary").textContent =
    `Optimize: ${total ? `${total} modules to add · ≈ ${n(result.cost.value)} Cr` : "nothing to add, the station covers its needs"}` +
    (short ? `\nTo buy per hour: ${short}` : "") +
    (optimize!.dropped.size ? "\nRemoved rows aren't counted in these figures" : "") +
    (rebuild ? "\nApply plan adds them and rebuilds the station in layers." : "\nApply plan adds them at free snaps; the station's modules stay where they are.");
}

/** Plans the targets. `fresh` (Generate plan): a new plan for the design as it is
 * now. Otherwise (buying a ware, or making it again) the current plan is redone
 * for the station it was made for, so what is placed keeps counting and its
 * modules are never added twice. */
async function generatePlan(fresh: boolean, optimize = false) {
  if (!scene) return;
  const baseline = fresh || !plan ? macroCounts() : plan.baseline;
  // A re-plan (buying a ware) keeps an Optimize plan's mode and removed rows.
  const mode = fresh ? (optimize ? { dropped: new Set<string>() } : undefined) : plan?.optimize;
  let result;
  try {
    result = await api.planProduction(
      {
        targets: mode ? [] : planner.targets,
        extra_modules: mode ? [] : planner.extras.map(({ macro, count }) => ({ macro, count })),
        race: planner.race || null,
        workforce: planner.workforce,
        storage_hours: planner.hours,
        storage: planner.storage,
        buy: planner.buy,
        sunlight,
        method: buildMethod,
      },
      [...baseline].map(([macro, count]) => ({ macro, count })),
    );
  } catch (e) {
    $("planner-summary").textContent = String(e);
    return;
  }
  if (fresh || !plan) $("planner-placed").textContent = "";
  plan = { result, baseline, optimize: mode };
  renderPlanner();
}

/** Places every planned module still to place (auto-placement; the plot may grow). */
async function placeAll() {
  if (!plan) return;
  const now = macroCounts();
  // Sides as picked now, so they can change without planning again.
  const sides = (macro: string) => planner.extras.find((x) => x.macro.toLowerCase() === macro.toLowerCase())?.sides ?? [];
  const todo = plan.result.modules.map((m) => ({ macro: m.macro, count: m.count - placedOf(m, now), sides: sides(m.macro) })).filter((m) => m.count > 0);
  if (todo.length) await placeModules(todo, true);
}

/** Auto-places `todo` (one undo step; the station's modules stay where they are)
 * and reports it. `offerPlot`: offer a bigger plot if the station outgrew its own.
 * False if it failed. */
async function placeModules(todo: { macro: string; count: number; sides?: string[] }[], offerPlot: boolean): Promise<boolean> {
  status(`Placing ${todo.reduce((s, m) => s + m.count, 0)} modules…`);
  let note = "";
  await apply(
    api.autoPlace(todo, planner.race || null, planner.fewerConnectors).then((s) => {
      const p = s.placement!;
      const left = p.unplaced.map((u) => `${u.count} × ${moduleName(u.macro)}`).join(", ");
      note =
        `Placed ${p.placed.length} modules` +
        (p.connectors.length ? ` and ${p.connectors.length} connection structures` : "") +
        (p.docks_blocked ? `. ${p.docks_blocked} docks or bays have no clear space` : "") +
        (p.sides_blocked ? `. ${p.sides_blocked} modules couldn't go on their side (no free snaps there with a clear view out): move them by hand` : "") +
        (left ? `. No room for ${left}` : "");
      return s;
    }),
    undefined,
    true,
    offerPlot,
  );
  $("planner-placed").textContent = note;
  if (note) status(note, note.includes("No room") || note.includes("no clear approach") || note.includes("by hand") ? "error" : "good");
  return note !== "";
}

async function loadPlanner() {
  const wares = await api.listWares();
  wareNames = new Map(wares.map((w) => [w.ware, w.name]));
  const races = [...new Set(modules.filter((m) => m.class === "habitation" && m.race).map((m) => m.race!))].sort();
  $("planner-race").replaceChildren(new Option("Any", ""), ...races.map((r) => new Option(r[0].toUpperCase() + r.slice(1), r)));
  $<HTMLSelectElement>("planner-race").value = planner.race;
  $<HTMLInputElement>("planner-workforce").checked = planner.workforce;
  $<HTMLInputElement>("planner-hours").value = String(planner.hours);
  for (const t of ["container", "solid", "liquid"]) $<HTMLInputElement>(`planner-store-${t}`).value = planner.storage[t] ? amountText(planner.storage[t]) : "";
  $<HTMLInputElement>("planner-fewer").checked = planner.fewerConnectors;
  renderPlanner();
}

$("planner-add").onclick = () => {
  const text = $<HTMLInputElement>("planner-ware").value.trim().toLowerCase();
  const ware = [...wareNames].find(([id, name]) => name.toLowerCase() === text || id === text)?.[0];
  if (!ware) {
    status(`No module makes "${$<HTMLInputElement>("planner-ware").value}"`);
    return;
  }
  const modulesFor = Math.max(1, Math.round(Number($<HTMLInputElement>("planner-count").value)) || 1);
  const existing = planner.targets.find((t) => t.ware === ware);
  if (existing) existing.modules += modulesFor;
  else planner.targets.push({ ware, modules: modulesFor });
  $<HTMLInputElement>("planner-ware").value = "";
  wareSearch.close();
  savePlanner();
  renderPlanner();
};

/** A search box with a list of matches under it: each typed word must be in the
 * name or id, names starting with the first word first. Arrow keys and Enter, or a
 * click, pick one; Enter with the list closed calls `enter`. */
function searchBox(box: HTMLInputElement, list: HTMLElement, entries: () => [string, string][], pick: (id: string, name: string) => void, enter: () => void) {
  const s = { open: false, matches: [] as [string, string][], active: 0 };
  const render = () => {
    const words = box.value.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const starts = (name: string) => (words.length && name.toLowerCase().startsWith(words[0]) ? 0 : 1);
    s.matches = entries()
      .filter(([id, name]) => words.every((w) => name.toLowerCase().includes(w) || id.includes(w)))
      .sort(([, a], [, b]) => starts(a) - starts(b) || a.localeCompare(b));
    s.active = Math.min(s.active, Math.max(0, s.matches.length - 1));
    list.replaceChildren(
      ...s.matches.map(([id, name], i) => {
        const row = document.createElement("div");
        row.textContent = name;
        if (i === s.active) row.className = "active";
        // mousedown, not click: the box loses focus (and closes the list) before a click lands.
        row.onmousedown = (ev) => {
          ev.preventDefault();
          pick(id, name);
        };
        return row;
      }),
    );
    list.hidden = !s.open || s.matches.length === 0;
    list.children[s.active]?.scrollIntoView({ block: "nearest" });
  };
  box.oninput = () => {
    s.open = true;
    s.active = 0;
    render();
  };
  box.onfocus = () => {
    s.open = true;
    render();
  };
  box.onblur = () => {
    s.open = false;
    render();
  };
  box.onkeydown = (ev) => {
    const n = s.matches.length;
    if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
      ev.preventDefault();
      if (n) s.active = (s.active + (ev.key === "ArrowDown" ? 1 : n - 1)) % n;
      s.open = true;
      render();
    } else if (ev.key === "Enter") {
      if (s.open && n) pick(...s.matches[s.active]);
      else enter();
    } else if (ev.key === "Escape") {
      s.open = false;
      render();
    }
  };
  return {
    close() {
      s.open = false;
      render();
    },
  };
}

const wareSearch = searchBox(
  $<HTMLInputElement>("planner-ware"),
  $("planner-ware-list"),
  () => [...wareNames],
  (_, name) => {
    $<HTMLInputElement>("planner-ware").value = name;
    $("planner-add").click();
  },
  () => $("planner-add").click(),
);

/** Classes the planner's Other modules box offers: end uses, not production (wares go in the ware box). */
const EXTRA_CLASSES = ["defencemodule", "storage", "buildmodule"];
/** Other modules the planner can add: defence, storage and ship-building modules, by macro. */
const moduleSearch = searchBox(
  $<HTMLInputElement>("planner-module"),
  $("planner-module-list"),
  () => offered().filter((m) => EXTRA_CLASSES.includes(m.class)).map((m) => [m.macro, m.name]),
  (id) => addExtra(id),
  () => {
    const text = $<HTMLInputElement>("planner-module").value.trim().toLowerCase();
    const found = offered().find((m) => EXTRA_CLASSES.includes(m.class) && (m.name.toLowerCase() === text || m.macro.toLowerCase() === text));
    if (found) addExtra(found.macro);
    else status(`No defence, storage or ship-building module "${$<HTMLInputElement>("planner-module").value}"`);
  },
);
$("planner-module-add").onclick = () => $<HTMLInputElement>("planner-module").dispatchEvent(new KeyboardEvent("keydown", { key: "Enter" }));

function addExtra(macro: string) {
  const count = Math.max(1, Math.round(Number($<HTMLInputElement>("planner-module-count").value)) || 1);
  const existing = planner.extras.find((x) => x.macro.toLowerCase() === macro.toLowerCase());
  if (existing) existing.count += count;
  else planner.extras.push({ macro, count, sides: [] });
  $<HTMLInputElement>("planner-module").value = "";
  moduleSearch.close();
  savePlanner();
  renderPlanner();
}

/** "1M", "500k", "250,000" as m³; null if it isn't a number. */
function parseAmount(text: string) {
  const m = text.trim().toLowerCase().replace(/[, ]/g, "").match(/^(\d+(?:\.\d+)?)([km]?)$/);
  return m ? Number(m[1]) * (m[2] === "m" ? 1e6 : m[2] === "k" ? 1e3 : 1) : null;
}
const amountText = (n: number) => (n >= 1e6 ? `${n / 1e6}M` : n >= 1e3 ? `${n / 1e3}k` : String(n));

for (const t of ["container", "solid", "liquid"]) {
  const box = $<HTMLInputElement>(`planner-store-${t}`);
  box.onchange = () => {
    const v = parseAmount(box.value);
    if (box.value.trim() && !v) status(`Storage: give m³, e.g. 1M, 500k or 250000`, "error");
    if (v) planner.storage[t] = v;
    else delete planner.storage[t];
    box.value = v ? amountText(v) : "";
    savePlanner();
  };
}

$<HTMLSelectElement>("planner-race").onchange = (ev) => {
  planner.race = (ev.target as HTMLSelectElement).value;
  savePlanner();
};
$<HTMLInputElement>("planner-workforce").onchange = (ev) => {
  planner.workforce = (ev.target as HTMLInputElement).checked;
  savePlanner();
};
$<HTMLInputElement>("planner-fewer").onchange = (ev) => {
  planner.fewerConnectors = (ev.target as HTMLInputElement).checked;
  savePlanner();
};
$<HTMLInputElement>("planner-hours").onchange = (ev) => {
  planner.hours = Math.max(0, Number((ev.target as HTMLInputElement).value) || 0);
  savePlanner();
};
$("planner-generate").onclick = () => void generatePlan(true);
$("optimize-rebuild").onchange = () => plan?.optimize && renderPlanner();

interface Optimized { added: { macro: string; count: number }[]; moved: number; connectors_removed: number; connectors_added: number; docks_blocked: number; plot: { plot: { text: string } | null } }

/** Optimize: plan the open station's shortfall fixes (the planner's settings) for the user to trim. */
$("planner-optimize").onclick = () => {
  if (!scene?.entries.length) {
    status("Nothing to optimize: the design is empty", "error");
    return;
  }
  void generatePlan(true, true);
};

/** Apply plan: add the Optimize plan's modules and fit the plot. With Rebuild
 * connection structures, the station is re-laid out in layers too. */
$("planner-apply").onclick = async () => {
  if (!plan?.optimize) return;
  const modules = optimizeModules().map((m) => ({ macro: m.macro, count: m.count }));
  if (!$<HTMLInputElement>("optimize-rebuild").checked) {
    if (!modules.length || !(await placeModules(modules, false))) return;
    await apply(api.fitPlot(false));
  } else {
    status("Optimizing…");
    let note = "";
    await apply(
      api
        .runOp<Optimized>({
          op: "optimize_station",
          race: planner.race || null,
          workforce: planner.workforce,
          storage_hours: planner.hours,
          buy: planner.buy,
          sunlight,
          method: buildMethod,
          keep_shape: false,
          fewer_connectors: planner.fewerConnectors,
          modules,
        })
        .then((r) => {
          const o = r.result;
          const added = o.added.reduce((s, a) => s + a.count, 0);
          note =
            `Optimized: re-placed ${o.moved} modules` +
            (added ? `, added ${added} to cover shortfalls` : ", no shortfalls to cover") +
            `; connection structures −${o.connectors_removed} +${o.connectors_added}` +
            (o.docks_blocked ? `. ${o.docks_blocked} docks or bays have no clear space` : "") +
            (o.plot.plot ? `; plot fitted: ${o.plot.plot.text}` : "");
          return r.scene!;
        }),
      undefined,
      true,
    );
    if (!note) return;
    status(note, note.includes("no clear approach") ? "error" : "good");
  }
  // The plan (if any) was for the station before: start over.
  plan = null;
  $("planner-result").replaceChildren();
  $("planner-summary").textContent = "";
  $("planner-placed").textContent = "";
  $("planner-place").hidden = true;
  $("planner-apply").hidden = true;
};
$("planner-place").onclick = () => void placeAll();

/** `index` and every module attached to it, directly or not. */
function subtree(index: number): Set<number> {
  const found = new Set([index]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const e of scene?.entries ?? []) {
      if (e.attached && found.has(e.attached.to) && !found.has(e.index)) {
        found.add(e.index);
        grew = true;
      }
    }
  }
  return found;
}

/** Before replacing the design: true when there is nothing unsaved or the user agrees. */
async function confirmDiscard(): Promise<boolean> {
  if (!scene?.dirty) return true;
  return ask(`"${scene.name}" has unsaved changes. Discard them?`, { title: "Unsaved changes", kind: "warning" });
}

/** Snaps a placed module can connect by: those not holding one of its own attached modules (its shape must be loaded). */
function connectableSnaps(e: Entry): SnapInfo[] {
  const held = new Set(scene!.entries.filter((x) => x.attached?.to === e.index).map((x) => x.attached!.to_snap.toLowerCase()));
  return (shapes.get(e.macro.toLowerCase())?.snaps ?? []).filter((s) => !held.has(s.name.toLowerCase()));
}

/** Re-attaches the selected module by its snap `snap`, on the same spot with the same twist. */
async function changeSnap(snap: string) {
  const sel = entry(selected);
  const a = sel?.attached;
  if (!sel || !a || snap === a.snap) return;
  const label = [...$<HTMLSelectElement>("sel-snap").options].find((o) => o.value === snap)?.text ?? snap;
  await apply(api.reattachModule(sel.index, snap, a.to, a.to_snap, a.twist ?? 0), `${sel.name ?? sel.macro} now connects by ${label}`, false, true);
}

$<HTMLSelectElement>("sel-snap").addEventListener("change", (ev) => {
  const list = ev.target as HTMLSelectElement;
  list.blur(); // give the keys back
  void changeSnap(list.value);
});

/** Move mode: the selected module (with everything on it) goes onto the next free snap clicked. */
async function startMove() {
  const e = entry(selected);
  if (!e) return;
  armed = null;
  moving = e.index;
  $("armed-panel").hidden = false;
  $("armed-title").textContent = "Moving";
  $("armed-name").textContent = e.name ?? e.macro;
  $("btn-place-root").hidden = true;
  await shapeOf(e.macro);
  $<HTMLSelectElement>("armed-snap").replaceChildren(...connectableSnaps(e).map(snapOption));
  if (e.attached) $<HTMLSelectElement>("armed-snap").value = e.attached.snap;
  status("Pick the snap it connects by (orange cones; C cycles, T twists), then click a free snap (cone) to move it there. Esc cancels.");
  renderPalette();
  redraw();
}

function stopMove() {
  moving = null;
  $("armed-panel").hidden = !armed;
  $("armed-title").textContent = "Placing";
}

async function moveOnto(to: number, toSnap: string) {
  const index = moving!;
  const snap = $<HTMLSelectElement>("armed-snap").value;
  const twist = Number($<HTMLSelectElement>("armed-twist").value);
  stopMove();
  await apply(api.reattachModule(index, snap, to, toSnap, twist), `Moved onto #${to}`, false, true);
}

// ---------------------------------------------------------------- actions

async function arm(m: ModuleInfo | null) {
  if (m) stopMove();
  armed = m;
  $("armed-panel").hidden = !m;
  if (m) {
    $("armed-name").textContent = m.name;
    const shape = await shapeOf(m.macro);
    $<HTMLSelectElement>("armed-snap").replaceChildren(...shape.snaps.map(snapOption));
    void modelOf(m.macro).then(refreshPlacing); // for the preview
    status(scene?.entries.length ? "Click or drop onto a free snap (cone) to attach. C cycles the connecting snap, T the twist." : "Click in the view to place it there, or Place as first module (at the origin).");
  }
  renderPalette();
  redraw();
}

/** Which way a snap faces in its module's own frame (game: +X right, +Y up, +Z front). */
function snapDirection(s: SnapInfo) {
  const size = s.facing.map(Math.abs);
  const axis = size.indexOf(Math.max(...size));
  return [["right", "left"], ["top", "bottom"], ["front", "back"]][axis][s.facing[axis] >= 0 ? 0 : 1];
}

const snapOption = (s: SnapInfo) => new Option(`${snapDirection(s)} · ${s.name}`, s.name);

/** While placing or moving: the moving module's own snaps as cones (the chosen
 * one brighter), and a see-through preview on the hovered free snap. */
function refreshPlacing() {
  const picker = $<HTMLSelectElement>("armed-snap");
  const m = entry(moving);
  const macro = m?.macro ?? armed?.macro;
  const shape = macro ? shapes.get(macro.toLowerCase()) : undefined;
  const offered = new Set([...picker.options].map((o) => o.value));
  view.setOwnSnaps(m, shape?.snaps.filter((s) => offered.has(s.name)) ?? [], picker.value);
  const target = hoverTarget && entry(hoverTarget.index);
  const toSnap = target && shapes.get(target.macro.toLowerCase())?.snaps.find((s) => s.name.toLowerCase() === hoverTarget!.snap.toLowerCase());
  const own = shape?.snaps.find((s) => s.name === picker.value);
  if (!target || !toSnap || !own || !macro) return view.setGhost(null);
  const at = attachMatrix(target, toSnap, own, Number($<HTMLSelectElement>("armed-twist").value));
  if (!m) return view.setGhost([{ model: models.get(macro.toLowerCase()) ?? null, shape, matrix: at }]);
  // Moving: everything on it goes along.
  const delta = at.multiply(poseMatrix(m.position, m.rotation).invert());
  view.setGhost(
    [...subtree(m.index)].map((i) => {
      const e = entry(i)!;
      const key = e.macro.toLowerCase();
      return { model: models.get(key) ?? null, shape: shapes.get(key), matrix: delta.clone().multiply(poseMatrix(e.position, e.rotation)) };
    }),
  );
}

function setHover(h: { index: number; snap: string } | null) {
  if (h?.index === hoverTarget?.index && h?.snap === hoverTarget?.snap) return;
  hoverTarget = h;
  refreshPlacing();
}

/** Steps a placing dropdown (snap or twist) by `by` and updates the preview. */
function cycle(id: string, by: 1 | -1) {
  const list = $<HTMLSelectElement>(id);
  const n = list.options.length;
  if (n === 0) return;
  list.selectedIndex = (list.selectedIndex + by + n) % n;
  refreshPlacing();
  status(id === "armed-snap" ? `Connects by ${list.selectedOptions[0].text}` : `Twist ${list.value}°`);
}

for (const id of ["armed-snap", "armed-twist"]) {
  $<HTMLSelectElement>(id).addEventListener("change", (ev) => {
    (ev.target as HTMLSelectElement).blur(); // give the keys back
    refreshPlacing();
  });
}

function select(index: number | null) {
  selected = index;
  redraw();
}

async function attachArmed(to: number, toSnap: string, macro = armed?.macro) {
  if (!macro) return;
  const shape = await shapeOf(macro);
  const chosen = armed?.macro === macro ? $<HTMLSelectElement>("armed-snap").value : shape.snaps[0]?.name;
  const twist = Number($<HTMLSelectElement>("armed-twist").value);
  await apply(api.attachModule(macro, chosen, to, toSnap, twist), `Attached ${moduleName(macro)} to #${to}`, false, true);
}

async function twistSelected(direction: 1 | -1) {
  const sel = entry(selected);
  if (!sel?.attached) return;
  await apply(api.setTwist(sel.index, (sel.attached.twist ?? 0) + direction * rotationStep), undefined, false, true);
}

// ---------------------------------------------------------------- rotation

/** Step for twists, in degrees; remembered. */
let rotationStep = (() => {
  try {
    return Number(localStorage.getItem("x4sd.rotationStep")) || 90;
  } catch {
    return 90;
  }
})();
let gizmoMode: "translate" | "rotate" = "translate";

/** Shows the rotation step everywhere and offers its multiples as attach twists. */
function renderRotationStep() {
  document.querySelectorAll<HTMLSelectElement>(".rot-step").forEach((s) => (s.value = String(rotationStep)));
  const twist = $<HTMLSelectElement>("armed-twist");
  const keep = Number(twist.value) || 0;
  const angles = Array.from({ length: 360 / rotationStep }, (_, i) => i * rotationStep);
  twist.replaceChildren(...angles.map((a) => new Option(String(a), String(a))));
  twist.value = String(angles.includes(keep) ? keep : 0);
}
document.querySelectorAll<HTMLSelectElement>(".rot-step").forEach(
  (s) =>
    (s.onchange = () => {
      rotationStep = Number(s.value);
      s.blur(); // give the keys back (R / Shift+R ignore a focused dropdown)
      try {
        localStorage.setItem("x4sd.rotationStep", String(rotationStep));
      } catch {
        // Remembering the step is a convenience only.
      }
      renderRotationStep();
      redraw();
    }),
);
renderRotationStep();
$("btn-straighten").onclick = () => {
  const sel = entry(selected);
  if (sel?.attached) void apply(api.straighten(sel.index), `Straightened ${sel.name ?? sel.macro}`, false, true);
};
$("btn-gizmo-mode").onclick = () => {
  if (selected === null) return;
  if (armed) {
    // Placing a module: stop that and rotate the selected one instead.
    gizmoMode = "rotate";
    void arm(null);
    return;
  }
  gizmoMode = gizmoMode === "rotate" ? "translate" : "rotate";
  redraw();
};

async function removeSelected() {
  if (selected === null) return;
  const index = selected;
  await apply(api.removeModule(index), `Removed #${index} and everything attached to it`);
}

function choosePlan(title: string, plans: PlanRef[]): Promise<PlanRef | null> {
  return new Promise((resolve) => {
    $("modal-title").textContent = title;
    const body = $("modal-body");
    body.replaceChildren(
      ...plans.map((p) => {
        const row = document.createElement("div");
        row.className = "plan-row";
        row.innerHTML = `<div class="n"></div><div class="m"></div>`;
        row.querySelector(".n")!.textContent = p.name;
        row.querySelector(".m")!.textContent = `${p.modules} modules · ${p.source ?? ""} · ${p.id}`;
        row.onclick = () => close(p);
        return row;
      }),
    );
    const close = (p: PlanRef | null) => {
      $("modal").hidden = true;
      resolve(p);
    };
    $("modal-close").onclick = () => close(null);
    $("modal").hidden = false;
  });
}

async function openPlans(title: string, plans: PlanRef[]) {
  if (plans.length === 0) return status("No plans found", "error");
  if (!(await confirmDiscard())) return;
  const plan = plans.length === 1 ? plans[0] : await choosePlan(title, plans);
  if (!plan) return;
  selected = null;
  await apply(api.openDesign(plan.file, plan.id), `Opened "${plan.name}"`, true);
}

async function exportToGame() {
  try {
    let result;
    try {
      result = await api.exportDesign(false, false);
    } catch (e) {
      if (!String(e).startsWith("exists:")) throw e;
      if (!(await ask(`${String(e).slice(8)}\n\nReplace it?`, { title: "Export to game", kind: "warning" }))) return;
      result = await api.exportDesign(false, true);
    }
    await apply(api.scene());
    const renamed = result.new_plan ? " as a new plan (it was renamed, so the original stays)" : "";
    status(`Exported${renamed} to ${result.file}. In game: station build menu → Import.`, "good");
  } catch (e) {
    status(String(e), "error");
  }
}

// ---------------------------------------------------------------- wiring

$("search").oninput = renderPalette;
$("class-filter").onchange = renderPalette;
$("btn-new").onclick = async () => {
  if (!(await confirmDiscard())) return;
  selected = null;
  void apply(api.newDesign("New station"), "New design", true);
};
$("btn-open-game").onclick = async () => {
  try {
    await openPlans("Open game plan", await api.listGamePlans());
  } catch (e) {
    status(String(e), "error");
  }
};
/** The game's plan folder, `<profile>\constructionplan` (its Import and Export files). */
const planFolder = () => (profileDir ? `${profileDir}\\constructionplan` : null);
/** The file name the game gives a plan: the name with characters outside `[A-Za-z0-9_-() ]` as `_` (as `import::file_name`). */
const planFileName = (name: string) => `${name.replace(/[^A-Za-z0-9_\-() ]/g, "_")}.xml`;

$("btn-open-file").onclick = async () => {
  const file = await open({ defaultPath: planFolder() ?? undefined, filters: [{ name: "Plan files", extensions: ["xml"] }] });
  if (typeof file !== "string") return;
  try {
    await openPlans("Open plan", await api.listPlansInFile(file));
  } catch (e) {
    status(String(e), "error");
  }
};
$("btn-save").onclick = async () => {
  const name = planFileName(scene?.name ?? "station");
  const folder = planFolder();
  const file = await save({ defaultPath: folder ? `${folder}\\${name}` : name, filters: [{ name: "Plan file", extensions: ["xml"] }] });
  if (!file) return;
  try {
    const result = await api.saveDesign(file);
    await apply(api.scene());
    status(`Saved ${file}${result.new_plan ? " as a new plan (it was renamed, so the original stays)" : ""}`, "good");
  } catch (e) {
    status(String(e), "error");
  }
};
$("btn-export").onclick = exportToGame;

/** Sets the blueprint limit, remembers it, and redraws the palette. */
function setBlueprints(b: Blueprints, tellCore = true) {
  blueprints = b;
  owned = ownedSet();
  try {
    localStorage.setItem("x4sd.blueprints", JSON.stringify(b));
  } catch {
    // Remembering it is a convenience only.
  }
  if (tellCore) void api.setBlueprints(b);
  renderBlueprints();
  renderRaceFilter();
  renderPalette();
}

function renderBlueprints() {
  $<HTMLInputElement>("bp-on").checked = blueprints.on;
  $("bp-info").hidden = !blueprints.on || !blueprints.save;
  if (!blueprints.save) return;
  const read = blueprints.read_at ? new Date(blueprints.read_at * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "";
  $("bp-save").textContent = `${blueprints.save.split(/[\\/]/).pop()} · ${blueprints.modules.length} modules · read ${read}`;
  $("bp-save").title = blueprints.save;
}

/** Reads the blueprints from a save; on failure the limit stays as it was. */
async function readSave(file: string) {
  const name = file.split(/[\\/]/).pop();
  status(`Reading the blueprints in ${name}…`);
  document.querySelectorAll<HTMLInputElement | HTMLButtonElement>("#bp-toggle input, #bp-info button").forEach((e) => (e.disabled = true));
  try {
    setBlueprints(await api.readBlueprints(file), false);
    status(`${blueprints.modules.length} of ${modules.length} modules have blueprints in ${name}`, "good");
  } catch (e) {
    status(String(e), "error");
    renderBlueprints();
  } finally {
    document.querySelectorAll<HTMLInputElement | HTMLButtonElement>("#bp-toggle input, #bp-info button").forEach((e) => (e.disabled = false));
  }
}

/** A save picked in the profile's save folder, read. */
async function pickSave() {
  const file = await open({ defaultPath: profileDir ? `${profileDir}\\save` : undefined, filters: [{ name: "X4 saves", extensions: ["gz", "xml"] }] });
  if (typeof file === "string") await readSave(file);
  else renderBlueprints();
}

$<HTMLInputElement>("bp-on").onchange = (ev) => {
  const on = (ev.target as HTMLInputElement).checked;
  // Turning it on again uses the save read last; only the first time asks for one.
  if (on && !blueprints.save) void pickSave();
  else setBlueprints({ ...blueprints, on });
};
$("bp-other").onclick = () => void pickSave();
$("bp-reread").onclick = () => {
  if (blueprints.save) void readSave(blueprints.save);
};
$("btn-undo").onclick = () => apply(api.undo());
$("btn-redo").onclick = () => apply(api.redo());
$("btn-frame").onclick = () => view.frame();
$<HTMLInputElement>("textures").checked = showTextures;
$<HTMLInputElement>("textures").onchange = async (ev) => {
  showTextures = (ev.target as HTMLInputElement).checked;
  try {
    localStorage.setItem("x4sd.textures", showTextures ? "on" : "off");
  } catch {
    // Remembering the choice is a convenience only.
  }
  if (showTextures) await loadTextures();
  redraw();
};
$<HTMLSelectElement>("quality").value = quality;
$<HTMLSelectElement>("quality").onchange = async (ev) => {
  quality = (ev.target as HTMLSelectElement).value as Quality;
  try {
    localStorage.setItem("x4sd.quality", quality);
  } catch {
    // Remembering the choice is a convenience only.
  }
  models.forEach((g) => g?.dispose());
  models.clear();
  if (scene) await apply(Promise.resolve(scene), `Model detail: ${quality}`);
};
$("btn-twist-left").onclick = () => twistSelected(-1);
$("btn-twist-right").onclick = () => twistSelected(1);
$("btn-remove").onclick = removeSelected;
$<HTMLSelectElement>("build-method").onchange = (ev) => {
  buildMethod = (ev.target as HTMLSelectElement).value;
  try {
    localStorage.setItem("x4sd.method", buildMethod);
  } catch {
    // Remembering the choice is a convenience only.
  }
  void refreshCost();
};
$<HTMLInputElement>("sunlight").onchange = (ev) => {
  const value = Number((ev.target as HTMLInputElement).value);
  if (!Number.isFinite(value) || value < 0) return;
  sunlight = value;
  try {
    localStorage.setItem("x4sd.sunlight", String(sunlight));
  } catch {
    // Remembering the choice is a convenience only.
  }
  void refreshAnalysis();
};
$<HTMLInputElement>("sunlight").value = String(sunlight);
$("btn-move").onclick = startMove;
$("btn-detach").onclick = () => selected !== null && apply(api.detachModule(selected), "Detached: it now stands on its own");
$("btn-build-first").onclick = () => selected !== null && apply(api.buildFirst(selected), "Moved to the front of the build order");
$("btn-build-last").onclick = () => selected !== null && apply(api.buildLast(selected), "Moved to the end of the build order");
$<HTMLInputElement>("stage-end").onchange = (ev) =>
  selected !== null && apply(api.setStageEnd(selected, (ev.target as HTMLInputElement).checked));
$<HTMLInputElement>("build-step").oninput = (ev) => {
  const step = Number((ev.target as HTMLInputElement).value);
  buildStep = scene && step < scene.entries.length ? step : null;
  redraw();
};
const tabs = [["tab-modules", "modules-tab"], ["tab-order", "order-tab"], ["tab-planner", "planner-tab"]];
for (const [tab, panel] of tabs) {
  $(tab).onclick = () => {
    for (const [t, p] of tabs) {
      $(t).classList.toggle("on", t === tab);
      $(p).hidden = p !== panel;
    }
  };
}
document.querySelectorAll<HTMLInputElement>(".plot-grid input").forEach((input) => (input.onchange = () => apply(plotFromInputs())));
// Typed position of a free-standing module (metres, game frame): it moves with everything on it.
document.querySelectorAll<HTMLInputElement>("#sel-position input").forEach(
  (input) =>
    (input.onchange = () => {
      const sel = entry(selected);
      const pos = [...document.querySelectorAll<HTMLInputElement>("#sel-position input")].map((i) => i.valueAsNumber) as Vec3;
      if (!sel || sel.attached || pos.some((v) => !Number.isFinite(v))) return;
      void apply(api.moveRoot(sel.index, pos), `Moved ${sel.name ?? sel.macro} to ${pos.join(", ")} m`, false, true).then(() => view.framePlot());
    }),
);
$("btn-fit-plot").onclick = () => apply(api.fitPlot(false), "Plot fitted to the station");
$("btn-place-root").onclick = () => armed && placeFirst(armed.macro, [0, 0, 0]);
$<HTMLInputElement>("design-name").onchange = (ev) => apply(api.renameDesign((ev.target as HTMLInputElement).value));

/** Where a first module goes: the ground-plane point under the cursor, in 10 m steps (else the origin). */
function firstAt(ev: MouseEvent): Vec3 {
  const p = view.groundPoint(ev.clientX, ev.clientY);
  return p ? (p.map((v) => Math.round(v / 10) * 10 + 0) as Vec3) : [0, 0, 0];
}

/** Places the first module, then selects it and stops placing, so its position fields and gizmo show.
 * The view backs off 5 km from a tight fit (user request), leaving room for what gets attached next. */
async function placeFirst(macro: string, at: Vec3) {
  await apply(api.addModule(macro, at), `Placed ${moduleName(macro)}`, false, true);
  view.frame(5000);
  const first = scene?.entries.length === 1 ? scene.entries[0].index : null;
  if (first === null) return;
  await arm(null);
  select(first);
}

// Click (not drag) in the view: attach to a snap, or select a module. A middle
// click picks a module type to place, like the palette: the one under the cursor,
// else the selected one (middle drag still zooms).
const viewport = $("viewport");
let down: { x: number; y: number } | null = null;
viewport.addEventListener("pointerdown", (ev) => (down = { x: ev.clientX, y: ev.clientY }));
viewport.addEventListener("mousedown", (ev) => ev.button === 1 && ev.preventDefault()); // no autoscroll
viewport.addEventListener("pointerup", async (ev) => {
  if (!down || Math.hypot(ev.clientX - down.x, ev.clientY - down.y) > 5) return;
  if (ev.button === 1) {
    const pick = view.pick(ev.clientX, ev.clientY);
    const source = entry(pick?.kind === "module" ? pick.index : selected);
    const info = source && modules.find((m) => m.macro.toLowerCase() === source.macro.toLowerCase());
    if (info) await arm(info);
    return;
  }
  if (ev.button !== 0) return;
  const pick = view.pick(ev.clientX, ev.clientY);
  if (view.gizmoBusy()) return;
  if (pick?.kind === "own") {
    $<HTMLSelectElement>("armed-snap").value = pick.snap;
    refreshPlacing();
    status(`Connects by ${$<HTMLSelectElement>("armed-snap").selectedOptions[0]?.text}. Now click a free snap (cone).`);
  } else if (pick?.kind === "snap" && moving !== null) await moveOnto(pick.index, pick.snap);
  else if (pick?.kind === "snap" && armed) await attachArmed(pick.index, pick.snap);
  else if (pick?.kind === "module") select(pick.index);
  else if (armed && scene?.entries.length === 0) await placeFirst(armed.macro, firstAt(ev));
  else select(null);
});
viewport.addEventListener("pointermove", (ev) => {
  if ((armed || moving !== null) && !ev.buttons) setHover(view.hoverSnap(ev.clientX, ev.clientY));
});
viewport.addEventListener("pointerleave", () => setHover(null));

// Drag from the palette onto a snap cone (or anywhere, for the first module).
viewport.addEventListener("dragover", (ev) => {
  ev.preventDefault();
  viewport.classList.add("drop-target");
  view.hoverSnap(ev.clientX, ev.clientY);
});
viewport.addEventListener("dragleave", () => viewport.classList.remove("drop-target"));
viewport.addEventListener("drop", async (ev) => {
  ev.preventDefault();
  viewport.classList.remove("drop-target");
  const macro = ev.dataTransfer?.getData("text/x4sd-macro");
  if (!macro) return;
  const snap = view.hoverSnap(ev.clientX, ev.clientY);
  if (scene?.entries.length === 0) await placeFirst(macro, firstAt(ev));
  else if (snap) await attachArmed(snap.index, snap.snap, macro);
  else status("Drop the module onto a free snap (cone).", "error");
});

window.addEventListener("keydown", (ev) => {
  if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement) return;
  const key = ev.key.toLowerCase();
  if (ev.ctrlKey && key === "z") void apply(ev.shiftKey ? api.redo() : api.undo());
  else if (ev.ctrlKey && key === "y") void apply(api.redo());
  else if (key === "delete") void removeSelected();
  else if (key === "c" && (armed || moving !== null)) cycle("armed-snap", ev.shiftKey ? -1 : 1);
  else if (key === "t" && (armed || moving !== null)) cycle("armed-twist", ev.shiftKey ? -1 : 1);
  else if (key === "c" && entry(selected)?.attached) {
    const list = $<HTMLSelectElement>("sel-snap");
    const n = list.options.length;
    if (n > 1) void changeSnap(list.options[(list.selectedIndex + (ev.shiftKey ? -1 : 1) + n) % n].value);
  }
  else if (key === "r") void twistSelected(ev.shiftKey ? -1 : 1);
  else if (key === "f") view.frame();
  else if (key === "escape") {
    if (moving !== null) {
      stopMove();
      redraw();
      status("Move cancelled");
      return;
    }
    void arm(null);
    select(null);
  }
});

// Closing with unsaved changes asks first.
void getCurrentWindow().onCloseRequested(async (ev) => {
  if (scene?.dirty && !(await confirmDiscard())) ev.preventDefault();
});

// An AI agent edited the design through the live link (x4sd mcp).
void listen<{ scene: Scene; op: string; replaced: boolean }>("scene-changed", (ev) =>
  apply(Promise.resolve(ev.payload.scene), `Agent: ${ev.payload.op.replace(/_/g, " ")}`, ev.payload.replaced),
);

// An agent read a save or turned the blueprint limit on or off.
void listen<Blueprints>("blueprints-changed", (ev) => {
  setBlueprints(ev.payload, false);
  status(`Agent: ${blueprints.on ? `only modules with blueprints (${blueprints.modules.length})` : "every module"}`);
});

// ---------------------------------------------------------------- start

(async () => {
  try {
    if (blueprints.save) await api.setBlueprints(blueprints);
    renderBlueprints();
    const catalog = await api.loadCatalog();
    modules = catalog.modules;
    profileDir = catalog.profile;
    void loadPlanner().catch((e) => status(`Planner: ${e}`));
    void loadEquipment().catch((e) => status(`Equipment: ${e}`));
    // Game order: by build-menu category, then by name.
    modules.sort((a, b) => categoryOf(a) - categoryOf(b) || a.name.localeCompare(b.name));
    const used = new Set(modules.map(categoryOf));
    $("class-filter").append(...CATEGORIES.flatMap((c, i) => (used.has(i) ? [new Option(c.name, String(i))] : [])));
    renderRaceFilter();
    renderPalette();
    await apply(api.scene(), `${modules.length} modules loaded · profile ${catalog.profile ?? "not found"}`, true);
  } catch (e) {
    status(String(e), "error");
  }
})();
