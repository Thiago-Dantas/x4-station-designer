// Builder UI: palette, 3D view, inspector. All edits go to the Rust core
// through the Tauri commands in api.ts, which return the new scene.
import "./style.css";
import { listen } from "@tauri-apps/api/event";
import { ask, open, save } from "@tauri-apps/plugin-dialog";
import type { BufferGeometry } from "three";
import { api, type ModuleInfo, type PlanRef, type Quality, type Scene, type Shape } from "./api";
import { CLASS_COLORS, modelGeometry, View3D } from "./view3d";

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const view = new View3D($("viewport"));
const shapes = new Map<string, Shape>();
/** Real models per macro at the current quality (null: none, boxes are drawn). */
const models = new Map<string, BufferGeometry | null>();
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

/** Runs a command that returns the new scene, then redraws. Errors go to the status bar. */
async function apply(action: Promise<Scene>, done?: string, frame = false) {
  try {
    scene = await action;
    const macros = [...new Set(scene.entries.map((e) => e.macro))];
    if (macros.some((x) => !models.has(x.toLowerCase()))) status("Loading models…");
    await Promise.all(macros.flatMap((x) => [shapeOf(x), modelOf(x)]));
    if (selected !== null && !scene.entries.some((e) => e.index === selected)) selected = null;
    redraw();
    if (frame) view.frame();
    if (done) status(done, "good");
  } catch (e) {
    status(String(e), "error");
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
  view.render(scene, shapes, models, selected, armed !== null);
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
  }
  $("btn-place-root").hidden = scene.entries.length > 0;
}

function renderPalette() {
  const text = $<HTMLInputElement>("search").value.toLowerCase();
  const cls = $<HTMLSelectElement>("class-filter").value;
  const shown = modules.filter(
    (m) => (!cls || m.class === cls) && (!text || m.name.toLowerCase().includes(text) || m.macro.toLowerCase().includes(text)),
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

// ---------------------------------------------------------------- actions

async function arm(m: ModuleInfo | null) {
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
  await apply(api.attachModule(macro, chosen, to, toSnap, twist), `Attached ${moduleName(macro)} to #${to}`);
}

async function twistSelected(delta: number) {
  const sel = entry(selected);
  if (!sel?.attached) return;
  await apply(api.setTwist(sel.index, (sel.attached.twist ?? 0) + delta));
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
    status(`Exported to ${result.file}. In game: station build menu → Import.`, "good");
  } catch (e) {
    status(String(e), "error");
  }
}

// ---------------------------------------------------------------- wiring

$("search").oninput = renderPalette;
$("class-filter").onchange = renderPalette;
$("btn-new").onclick = () => {
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
    status(`Saved ${file}`, "good");
  } catch (e) {
    status(String(e), "error");
  }
};
$("btn-export").onclick = exportToGame;
$("btn-undo").onclick = () => apply(api.undo());
$("btn-redo").onclick = () => apply(api.redo());
$("btn-frame").onclick = () => view.frame();
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
$("btn-place-root").onclick = () => armed && apply(api.addModule(armed.macro, [0, 0, 0]), `Placed ${armed.name}`, true);
$<HTMLInputElement>("design-name").onchange = (ev) => apply(api.renameDesign((ev.target as HTMLInputElement).value));

// Click (not drag) in the view: attach to a snap, or select a module.
const viewport = $("viewport");
let down: { x: number; y: number } | null = null;
viewport.addEventListener("pointerdown", (ev) => (down = { x: ev.clientX, y: ev.clientY }));
viewport.addEventListener("pointerup", async (ev) => {
  if (!down || ev.button !== 0 || Math.hypot(ev.clientX - down.x, ev.clientY - down.y) > 5) return;
  const pick = view.pick(ev.clientX, ev.clientY);
  if (pick?.kind === "snap" && armed) await attachArmed(pick.index, pick.snap);
  else if (pick?.kind === "module") select(pick.index);
  else if (armed && scene?.entries.length === 0) await apply(api.addModule(armed.macro, [0, 0, 0]), `Placed ${armed.name}`, true);
  else select(null);
});
viewport.addEventListener("pointermove", (ev) => {
  if (armed && !ev.buttons) view.hoverSnap(ev.clientX, ev.clientY);
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
  if (scene?.entries.length === 0) await apply(api.addModule(macro, [0, 0, 0]), `Placed ${moduleName(macro)}`, true);
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
    void arm(null);
    select(null);
  }
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
    const classes = [...new Set(modules.map((m) => m.class))].sort();
    $("class-filter").append(...classes.map((c) => new Option(c, c)));
    renderPalette();
    await apply(api.scene(), `${modules.length} modules loaded · profile ${catalog.profile ?? "not found"}`, true);
  } catch (e) {
    status(String(e), "error");
  }
})();
