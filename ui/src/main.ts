// Builder UI: palette, 3D view, inspector. All edits go to the Rust core
// through the Tauri commands in api.ts, which return the new scene.
import "./style.css";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type { BufferGeometry } from "three";
import { api, type ModuleInfo, type PlannedModule, type PlanRef, type ProductionPlan, type Quality, type Scene, type Shape } from "./api";
import { CLASS_COLORS, type DiffuseTexture, modelGeometry, View3D } from "./view3d";

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
let selected: number | null = null;
let armed: ModuleInfo | null = null;
/** Module being moved onto another snap (with everything attached to it), if any. */
let moving: number | null = null;
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
  renderOrder();
  void refreshCost();
  void refreshAnalysis();
  renderPlanner();
  $("dirty").hidden = !scene.dirty;
  const title = `X4 Station Designer - ${scene.name}${scene.dirty ? " *" : ""}`;
  if (title !== windowTitle) {
    windowTitle = title;
    void getCurrentWindow().setTitle(title).catch(() => undefined);
  }
  // A selected free-standing module can be dragged with the gizmo.
  const root = entry(selected);
  view.setGizmo(
    root && !root.attached && !armed && moving === null && buildStep === null ? root.position : null,
    root ? (p) => void apply(api.moveRoot(root.index, p), `Moved ${root.name ?? root.macro}`, false, true) : null,
  );
  $<HTMLInputElement>("design-name").value = scene.name;
  const count = `${scene.entries.length} module${scene.entries.length === 1 ? "" : "s"}`;
  $("patches").textContent = scene.patches.length ? `${count} · Needs: ${scene.patches.join(", ")}` : count;
  $<HTMLButtonElement>("btn-undo").disabled = !scene.can_undo;
  $<HTMLButtonElement>("btn-redo").disabled = !scene.can_redo;

  const issues = $("issues");
  issues.replaceChildren();
  if (scene.issues.length === 0) {
    const li = document.createElement("li");
    li.className = "ok";
    li.textContent = scene.entries.length ? "No problems found" : "Empty design";
    issues.append(li);
  }
  for (const issue of scene.issues) {
    const li = document.createElement("li");
    li.textContent = issue.text;
    if (issue.index !== null) li.onclick = () => select(issue.index);
    issues.append(li);
  }

  const sel = entry(selected);
  $("selection-panel").hidden = !sel;
  if (sel) {
    $("sel-name").textContent = sel.name ?? sel.macro;
    const a = sel.attached;
    $("sel-details").textContent =
      `#${sel.index} · ${sel.macro}\n` +
      (a ? `attached to #${a.to} (${a.to_snap}) by ${a.snap}, twist ${a.twist ?? "?"}°` : "not attached (root)") +
      `\n${sel.free_snaps.length} free snap(s)`;
    $<HTMLButtonElement>("btn-twist-left").disabled = !a;
    $<HTMLButtonElement>("btn-twist-right").disabled = !a;
    $<HTMLButtonElement>("btn-detach").disabled = !a;
    $<HTMLButtonElement>("btn-move").disabled = (scene.entries.length ?? 0) < 2;
    $<HTMLInputElement>("stage-end").checked = sel.stage_end;
  }
  $("btn-place-root").hidden = scene.entries.length > 0;

  const p = scene.plot;
  const summary = $("plot-summary");
  summary.textContent = p?.plot
    ? `Plot ${p.plot.size.join("×")} km · station needs ${p.required ? p.required.size.join("×") + " km" : "nothing yet"}` + (p.possible ? "" : " (more than any plot)")
    : "No plot";
  summary.className = "details" + (p && !p.fits ? " bad" : "");
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
  const shown = modules.filter(
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
  $("module-count").textContent = `${shown.length} of ${modules.length} modules`;
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
    row.onclick = () => select(e.index);
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
  modules.forEach((m) => counts.set(raceOf(m), (counts.get(raceOf(m)) ?? 0) + 1));
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
        row.children[0].textContent = f.name;
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

// ---------------------------------------------------------------- planner

interface PlannerState { targets: { ware: string; modules: number }[]; race: string; workforce: boolean; hours: number; buy: string[] }
let planner: PlannerState = (() => {
  const fallback: PlannerState = { targets: [], race: "", workforce: true, hours: 6, buy: [] };
  try {
    return { ...fallback, ...JSON.parse(localStorage.getItem("x4sd.planner") ?? "{}") };
  } catch {
    return fallback;
  }
})();
let wareNames = new Map<string, string>();
/** The last plan, and how many of each macro the design had when it was made. */
let plan: { result: ProductionPlan; baseline: Map<string, number> } | null = null;

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
  $("planner-buy").replaceChildren(
    ...planner.buy.map((w) => {
      const b = document.createElement("button");
      b.textContent = `Buy ${wareNames.get(w) ?? w} ✕`;
      b.title = "Bought instead of made; click to make it again";
      b.onclick = () => {
        planner.buy = planner.buy.filter((x) => x !== w);
        savePlanner();
        void generatePlan();
      };
      return b;
    }),
  );
  if (!plan) return;
  const { result, baseline } = plan;
  const now = macroCounts();
  const groups: [PlannedModule["role"], string][] = [["target", "Targets"], ["chain", "Production chain"], ["habitat", "Habitats"], ["storage", "Storage"], ["dock", "Docks"]];
  let left = 0;
  const rows: HTMLElement[] = [];
  for (const [role, title] of groups) {
    const members = result.modules.filter((m) => m.role === role);
    if (!members.length) continue;
    const h = document.createElement("h4");
    h.textContent = title;
    rows.push(h);
    for (const m of members) {
      const placed = Math.min(m.count, Math.max(0, (now.get(m.macro.toLowerCase()) ?? 0) - (baseline.get(m.macro.toLowerCase()) ?? 0)));
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
          void generatePlan();
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
  const total = result.modules.reduce((s, m) => s + m.count, 0);
  const supply = result.analysis.flows.filter((f) => f.net < -0.5).map((f) => `${f.name} ${n(-f.net)}`);
  $("planner-summary").textContent =
    (total ? `${total} modules to add, ${left} still to place · ≈ ${n(result.cost.value)} Cr` : "Nothing to add: the station covers its needs") +
    (supply.length ? `\nTo supply per hour: ${supply.join(", ")}` : "") +
    (result.unknown_targets.length ? `\nNo module makes: ${result.unknown_targets.join(", ")}` : "");
}

async function generatePlan() {
  if (!scene) return;
  let result;
  try {
    result = await api.planProduction({
      targets: planner.targets,
      race: planner.race || null,
      workforce: planner.workforce,
      storage_hours: planner.hours,
      buy: planner.buy,
      sunlight,
      method: buildMethod,
    });
  } catch (e) {
    $("planner-summary").textContent = String(e);
    return;
  }
  plan = { result, baseline: macroCounts() };
  renderPlanner();
}

async function loadPlanner() {
  const wares = await api.listWares();
  wareNames = new Map(wares.map((w) => [w.ware, w.name]));
  $("planner-wares").replaceChildren(...wares.map((w) => new Option(w.name, w.name)));
  const races = [...new Set(modules.filter((m) => m.class === "habitation" && m.race).map((m) => m.race!))].sort();
  $("planner-race").replaceChildren(new Option("Any", ""), ...races.map((r) => new Option(r[0].toUpperCase() + r.slice(1), r)));
  $<HTMLSelectElement>("planner-race").value = planner.race;
  $<HTMLInputElement>("planner-workforce").checked = planner.workforce;
  $<HTMLInputElement>("planner-hours").value = String(planner.hours);
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
  savePlanner();
  renderPlanner();
};
$<HTMLInputElement>("planner-ware").onkeydown = (ev) => ev.key === "Enter" && $("planner-add").click();
$<HTMLSelectElement>("planner-race").onchange = (ev) => {
  planner.race = (ev.target as HTMLSelectElement).value;
  savePlanner();
};
$<HTMLInputElement>("planner-workforce").onchange = (ev) => {
  planner.workforce = (ev.target as HTMLInputElement).checked;
  savePlanner();
};
$<HTMLInputElement>("planner-hours").onchange = (ev) => {
  planner.hours = Math.max(0, Number((ev.target as HTMLInputElement).value) || 0);
  savePlanner();
};
$("planner-generate").onclick = () => void generatePlan();

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
  // Its snaps that don't hold one of its own attached modules.
  const held = new Set(scene!.entries.filter((x) => x.attached?.to === e.index).map((x) => x.attached!.to_snap.toLowerCase()));
  const snaps = (await shapeOf(e.macro)).snaps.filter((s) => !held.has(s.name.toLowerCase()));
  $<HTMLSelectElement>("armed-snap").replaceChildren(...snaps.map((s) => new Option(s.name, s.name)));
  if (e.attached) $<HTMLSelectElement>("armed-snap").value = e.attached.snap;
  status("Click a free snap (cone) to move the module there. Esc cancels.");
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
    $<HTMLSelectElement>("armed-snap").replaceChildren(
      ...shape.snaps.map((s) => new Option(s.name, s.name)),
    );
    status(scene?.entries.length ? "Click or drop onto a free snap (cone) to attach." : "Place it as the first module.");
  }
  renderPalette();
  redraw();
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

async function twistSelected(delta: number) {
  const sel = entry(selected);
  if (!sel?.attached) return;
  await apply(api.setTwist(sel.index, (sel.attached.twist ?? 0) + delta), undefined, false, true);
}

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
    status(`Exported to ${result.file}. In game: station build menu → Import.`, "good");
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
$("btn-open-file").onclick = async () => {
  const file = await open({ filters: [{ name: "Plan files", extensions: ["xml"] }] });
  if (typeof file !== "string") return;
  try {
    await openPlans("Open plan", await api.listPlansInFile(file));
  } catch (e) {
    status(String(e), "error");
  }
};
$("btn-save").onclick = async () => {
  const file = await save({ defaultPath: `${scene?.name ?? "station"}.xml`, filters: [{ name: "Plan file", extensions: ["xml"] }] });
  if (!file) return;
  try {
    await api.saveDesign(file);
    await apply(api.scene());
    status(`Saved ${file}`, "good");
  } catch (e) {
    status(String(e), "error");
  }
};
$("btn-export").onclick = exportToGame;
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
$("btn-twist-left").onclick = () => twistSelected(-90);
$("btn-twist-right").onclick = () => twistSelected(90);
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
$("btn-fit-plot").onclick = () => apply(api.fitPlot(false), "Plot fitted to the station");
$("btn-place-root").onclick = () => armed && apply(api.addModule(armed.macro, [0, 0, 0]), `Placed ${armed.name}`, true, true);
$<HTMLInputElement>("design-name").onchange = (ev) => apply(api.renameDesign((ev.target as HTMLInputElement).value));

// Click (not drag) in the view: attach to a snap, or select a module.
const viewport = $("viewport");
let down: { x: number; y: number } | null = null;
viewport.addEventListener("pointerdown", (ev) => (down = { x: ev.clientX, y: ev.clientY }));
viewport.addEventListener("pointerup", async (ev) => {
  if (!down || ev.button !== 0 || Math.hypot(ev.clientX - down.x, ev.clientY - down.y) > 5) return;
  const pick = view.pick(ev.clientX, ev.clientY);
  if (view.gizmoBusy()) return;
  if (pick?.kind === "snap" && moving !== null) await moveOnto(pick.index, pick.snap);
  else if (pick?.kind === "snap" && armed) await attachArmed(pick.index, pick.snap);
  else if (pick?.kind === "module") select(pick.index);
  else if (armed && scene?.entries.length === 0) await apply(api.addModule(armed.macro, [0, 0, 0]), `Placed ${armed.name}`, true, true);
  else select(null);
});
viewport.addEventListener("pointermove", (ev) => {
  if ((armed || moving !== null) && !ev.buttons) view.hoverSnap(ev.clientX, ev.clientY);
});

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
  if (scene?.entries.length === 0) await apply(api.addModule(macro, [0, 0, 0]), `Placed ${moduleName(macro)}`, true, true);
  else if (snap) await attachArmed(snap.index, snap.snap, macro);
  else status("Drop the module onto a free snap (cone).", "error");
});

window.addEventListener("keydown", (ev) => {
  if (ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement) return;
  const key = ev.key.toLowerCase();
  if (ev.ctrlKey && key === "z") void apply(ev.shiftKey ? api.redo() : api.undo());
  else if (ev.ctrlKey && key === "y") void apply(api.redo());
  else if (key === "delete") void removeSelected();
  else if (key === "r") void twistSelected(ev.shiftKey ? -90 : 90);
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

// ---------------------------------------------------------------- start

(async () => {
  try {
    const catalog = await api.loadCatalog();
    modules = catalog.modules;
    void loadPlanner().catch((e) => status(`Planner: ${e}`));
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
