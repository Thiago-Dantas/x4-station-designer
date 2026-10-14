// The 3D station view: module models (or boxes as a fallback), free-snap cones, picking.
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { TransformControls } from "three/addons/controls/TransformControls.js";
import type { Entry, Quat, Scene, Shape, Vec3 } from "./api";

// The game is left-handed (Y up), Three.js right-handed: mirror Z. A rotation
// q = (w, x, y, z) becomes S·R·S with S = diag(1, 1, -1), i.e. (w, -x, -y, z).
export const toVec = (v: Vec3) => new THREE.Vector3(v[0], v[1], -v[2]);
export const toQuat = (q: Quat) => new THREE.Quaternion(-q[1], -q[2], q[3], q[0]);
/** Back to the game's frame: the inverse of `toQuat`. */
const fromQuat = (q: THREE.Quaternion): Quat => [q.w, -q.x, -q.y, q.z];

export const CLASS_COLORS: Record<string, number> = {
  connectionmodule: 0x8a96aa,
  production: 0xe0892f,
  storage: 0xd8c247,
  habitation: 0x58b56f,
  dockarea: 0x4a8fd8,
  pier: 0x3cb8c4,
  defencemodule: 0xc9504a,
  buildmodule: 0x9b6ad6,
  welfaremodule: 0xd673b0,
  processingmodule: 0xa7774a,
  ventureplatform: 0x6fc2a0,
  radar: 0xbfbfbf,
};

/** Geometry from `module_model` bytes, mirrored into Three.js space; null when empty.
 * `userData.materials` lists the game material of each geometry group. */
export function modelGeometry(buffer: ArrayBuffer): THREE.BufferGeometry | null {
  const words = new DataView(buffer);
  const [vertices, indices, groupCount] = [0, 4, 8].map((o) => words.getUint32(o, true));
  if (indices === 0) return null;
  let at = 12;
  const groups: { start: number; count: number; name: string }[] = [];
  for (let g = 0; g < groupCount; g++) {
    const [start, count, length] = [0, 4, 8].map((o) => words.getUint32(at + o, true));
    const name = new TextDecoder().decode(new Uint8Array(buffer, at + 12, length));
    groups.push({ start, count, name });
    at += 12 + Math.ceil(length / 4) * 4;
  }
  const positions = new Float32Array(buffer, at, vertices * 3).slice();
  const normals = new Float32Array(buffer, at + vertices * 12, vertices * 3).slice();
  const uvs = new Float32Array(buffer, at + vertices * 24, vertices * 2).slice();
  for (let i = 2; i < positions.length; i += 3) {
    positions[i] = -positions[i];
    normals[i] = -normals[i];
  }
  // The mirror also turns the game's clockwise triangles counter-clockwise, as Three.js expects.
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
  // Game UVs follow Direct3D (v down) and so does the DDS data, uploaded unflipped: no change needed.
  geometry.setAttribute("uv", new THREE.BufferAttribute(uvs, 2));
  geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(buffer, at + vertices * 32, indices).slice(), 1));
  groups.forEach((g, i) => geometry.addGroup(g.start, g.count, i));
  geometry.computeBoundingSphere();
  geometry.userData = { shared: true, materials: groups.map((g) => g.name) };
  return geometry;
}

/** A material's diffuse texture; `singleChannel` for BC4 (grey value in red). */
export interface DiffuseTexture { texture: THREE.CompressedTexture; singleChannel: boolean; bytes: number }

export type Pick = { kind: "module"; index: number } | { kind: "snap"; index: number; snap: string } | null;

const SNAP_COLOR = 0x37d0c0;
const SNAP_HOVER = 0xffe066;

export class View3D {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private station = new THREE.Group();
  /** The plot box (not part of `station`, so Fit view frames the modules). */
  private plotBox = new THREE.Group();
  private snaps = new THREE.Group();
  private raycaster = new THREE.Raycaster();
  private hovered: THREE.Mesh | null = null;
  private snapGeometry = new THREE.ConeGeometry(55, 160, 16).translate(0, 80, 0);
  /** Navigation keys held down: WASD pan like a right-mouse drag, Q/E move
   * down/up, the arrow keys rotate the camera around its target. */
  private panKeys = new Set<string>();
  private clock = new THREE.Clock();
  /** Frame counter shown in the view's corner: frames and busy milliseconds since `fpsSince`. */
  private fps = { el: document.createElement("div"), frames: 0, busy: 0, since: performance.now() };
  /** Move or rotate gizmo for a selected free-standing module. */
  private gizmo: TransformControls;
  private gizmoTarget = new THREE.Object3D();
  private onGizmoMoved: ((position: Vec3, rotation: Quat) => void) | null = null;
  /** The joint an attached module turns about (Three.js frame); null: it turns about its origin. */
  private gizmoPivot: THREE.Vector3 | null = null;
  /** Modules that follow the gizmo while dragging: the selected one and everything on it. */
  private gizmoCarries = new Set<number>();
  /** While dragging: where the gizmo started, and the drawings it carries with their start matrices. */
  private drag: { position: THREE.Vector3; quaternion: THREE.Quaternion; parts: { matrix: THREE.Matrix4; set: (m: THREE.Matrix4) => void }[] } | null = null;

  constructor(private container: HTMLElement) {
    this.renderer = new THREE.WebGLRenderer({ antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);
    container.appendChild(this.renderer.domElement);
    this.scene.background = new THREE.Color(0x0b0f16);
    this.camera = new THREE.PerspectiveCamera(50, 1, 10, 500000);
    this.camera.position.set(4000, 3000, 4000);
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.scene.add(new THREE.HemisphereLight(0xdde6ff, 0x202430, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.6);
    sun.position.set(1, 2, 1.5);
    const fill = new THREE.DirectionalLight(0x9fb4ff, 0.6);
    fill.position.set(-1.5, -0.5, -1);
    this.scene.add(sun, fill);
    const grid = new THREE.GridHelper(40000, 50, 0x2b3850, 0x1a2232);
    (grid.material as THREE.Material).transparent = true;
    (grid.material as THREE.Material).opacity = 0.6;
    this.scene.add(grid, this.station, this.snaps, this.plotBox);
    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.gizmo = new TransformControls(this.camera, this.renderer.domElement);
    this.gizmo.setTranslationSnap(10);
    this.gizmo.addEventListener("dragging-changed", (ev) => {
      this.controls.enabled = !ev.value;
      this.drag = null;
      if (ev.value) this.startDrag();
      else if (this.onGizmoMoved) {
        const p = this.gizmoTarget.position;
        this.onGizmoMoved([p.x, p.y, -p.z], fromQuat(this.gizmoTarget.quaternion)); // back to the game's frame (mirror Z)
      }
    });
    this.gizmo.addEventListener("objectChange", () => this.followGizmo());
    this.scene.add(this.gizmoTarget, this.gizmo.getHelper());
    this.bindPanKeys();
    this.fps.el.className = "fps";
    container.append(this.fps.el);
    this.renderer.setAnimationLoop(() => {
      const start = performance.now();
      this.keyNavigate(this.clock.getDelta());
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
      this.countFrame(start);
    });
  }

  /** Updates the frame counter twice a second: frames per second, and the time a
   * frame takes to update and draw (the frame rate stops at the screen's refresh rate). */
  private countFrame(start: number) {
    const f = this.fps;
    const now = performance.now();
    f.frames++;
    f.busy += now - start;
    if (now - f.since >= 500) {
      f.el.textContent = `${Math.round((f.frames * 1000) / (now - f.since))} fps · ${(f.busy / f.frames).toFixed(1)} ms`;
      f.frames = 0;
      f.busy = 0;
      f.since = now;
    }
  }

  private bindPanKeys() {
    const codes = ["KeyW", "KeyA", "KeyS", "KeyD", "KeyQ", "KeyE", "KeyZ", "KeyX", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"];
    const typing = (ev: KeyboardEvent) =>
      ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement || ev.target instanceof HTMLTextAreaElement;
    window.addEventListener("keydown", (ev) => {
      if (codes.includes(ev.code) && !typing(ev) && !ev.ctrlKey && !ev.altKey) {
        this.panKeys.add(ev.code);
        if (ev.code.startsWith("Arrow")) ev.preventDefault(); // no list scrolling
      }
    });
    window.addEventListener("keyup", (ev) => this.panKeys.delete(ev.code));
    window.addEventListener("blur", () => this.panKeys.clear());
  }

  /** Keyboard navigation while keys are held, like the game's: W/S move forward/back
   * along the view direction kept level, A/D left/right, Z/X straight down/up
   * (speeds scaled to the zoom); Q/E (and the left/right arrows) turn the camera
   * around the target, the up/down arrows tilt it. */
  private keyNavigate(seconds: number) {
    if (this.panKeys.size === 0) return;
    const dt = Math.min(seconds, 0.1);
    const held = (code: string) => (this.panKeys.has(code) ? 1 : 0);
    const right = held("KeyD") - held("KeyA");
    const ahead = held("KeyW") - held("KeyS");
    const rise = held("KeyX") - held("KeyZ");
    if (right || ahead || rise) {
      const step = this.camera.position.distanceTo(this.controls.target) * 0.8 * dt;
      // Forward = where the camera looks, without the vertical part (looking
      // straight down: the screen's up instead).
      const forward = this.controls.target.clone().sub(this.camera.position).setY(0);
      if (forward.lengthSq() < 1e-6) forward.setFromMatrixColumn(this.camera.matrix, 1).setY(0);
      forward.normalize();
      const offset = new THREE.Vector3()
        .addScaledVector(new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 0), right * step)
        .addScaledVector(forward, ahead * step)
        .addScaledVector(new THREE.Vector3(0, 1, 0), rise * step);
      this.camera.position.add(offset);
      this.controls.target.add(offset);
    }
    const spin = held("ArrowRight") + held("KeyE") - held("ArrowLeft") - held("KeyQ");
    const tilt = held("ArrowUp") - held("ArrowDown");
    if (spin || tilt) {
      const offset = this.camera.position.clone().sub(this.controls.target);
      const s = new THREE.Spherical().setFromVector3(offset);
      s.theta += spin * 1.5 * dt;
      s.phi = THREE.MathUtils.clamp(s.phi - tilt * 1.5 * dt, 0.01, Math.PI - 0.01);
      this.camera.position.copy(this.controls.target).add(offset.setFromSpherical(s));
      this.camera.lookAt(this.controls.target);
    }
  }

  /** A WebGL compressed texture from `texture` bytes, or null if this GPU lacks the format. */
  diffuseTexture(buffer: ArrayBuffer): DiffuseTexture | null {
    const words = new DataView(buffer);
    const [code, mipCount] = [words.getUint32(0, true), words.getUint32(4, true)];
    const ext = (name: string) => this.renderer.extensions.has(name);
    const formats: Record<number, [THREE.CompressedPixelFormat, boolean]> = {
      1: [THREE.RGBA_S3TC_DXT1_Format, ext("WEBGL_compressed_texture_s3tc")],
      2: [THREE.RGBA_S3TC_DXT3_Format, ext("WEBGL_compressed_texture_s3tc")],
      3: [THREE.RGBA_S3TC_DXT5_Format, ext("WEBGL_compressed_texture_s3tc")],
      4: [THREE.RED_RGTC1_Format, ext("EXT_texture_compression_rgtc")],
      5: [THREE.RED_GREEN_RGTC2_Format, ext("EXT_texture_compression_rgtc")],
      7: [THREE.RGBA_BPTC_Format, ext("EXT_texture_compression_bptc")],
    };
    const [format, supported] = formats[code] ?? [THREE.RGBA_S3TC_DXT5_Format, false];
    if (!supported) return null;
    const mipmaps = [];
    let at = 8;
    for (let i = 0; i < mipCount; i++) {
      const [width, height, length] = [0, 4, 8].map((o) => words.getUint32(at + o, true));
      mipmaps.push({ data: new Uint8Array(buffer, at + 12, length), width, height });
      at += 12 + length;
    }
    const texture = new THREE.CompressedTexture(mipmaps, mipmaps[0].width, mipmaps[0].height, format);
    texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
    texture.minFilter = THREE.LinearMipmapLinearFilter;
    texture.magFilter = THREE.LinearFilter;
    texture.anisotropy = Math.min(8, this.renderer.capabilities.getMaxAnisotropy());
    // Colour textures are sRGB; only switch when the GPU can decode sRGB blocks.
    if (code <= 3 && ext("WEBGL_compressed_texture_s3tc_srgb")) texture.colorSpace = THREE.SRGBColorSpace;
    texture.needsUpdate = true;
    return { texture, singleChannel: code === 4, bytes: buffer.byteLength };
  }

  /** Redraws the station. `shapes` must hold every macro in the scene; `models`
   * holds real models where they are loaded (boxes are drawn otherwise);
   * `textures` maps game material names to diffuse textures (when shown);
   * with `upTo`, only modules up to that build step are drawn; snap cones of
   * modules in `noSnaps` are left out (a module being moved and what it carries). */
  render(
    scene: Scene,
    shapes: Map<string, Shape>,
    models: Map<string, THREE.BufferGeometry | null>,
    textures: Map<string, DiffuseTexture | null> | null,
    selected: number | null,
    showSnaps: boolean,
    upTo: number | null = null,
    noSnaps: Set<number> | null = null,
  ) {
    clear(this.station);
    clear(this.snaps);
    clear(this.plotBox);
    this.hovered = null;
    const plot = scene.plot?.plot;
    if (plot) {
      // Per side from the station origin, in km; centre mirrored into Three.js space like everything else.
      const size = plot.size.map((s) => s * 1000) as Vec3;
      const centre = [0, 1, 2].map((i) => ((plot.pos[i] - plot.neg[i]) / 2) * 1000) as Vec3;
      const box = new THREE.LineSegments(
        new THREE.EdgesGeometry(new THREE.BoxGeometry(...size)),
        new THREE.LineBasicMaterial({ color: scene.plot!.fits ? 0x4fbf7a : 0xe5534b }),
      );
      box.position.copy(toVec(centre));
      this.plotBox.add(box);
    }
    const bad = new Set(scene.issues.map((i) => i.index).filter((i) => i !== null));
    // Real models are drawn instanced: one mesh per module type, so a 300-module
    // station costs a draw call per module type and material, not per module.
    // The selected module, and modules without a model, are drawn on their own.
    const instanced = new Map<string, Entry[]>();
    for (const e of scene.entries.filter((x) => upTo === null || x.index <= upTo)) {
      const key = e.macro.toLowerCase();
      if (models.get(key) && e.index !== selected) instanced.set(key, [...(instanced.get(key) ?? []), e]);
      else this.station.add(single(e, shapes.get(key), models.get(key) ?? null, textures, bad.has(e.index), e.index === selected));
      if (showSnaps && !noSnaps?.has(e.index)) {
        for (const s of e.free_snaps) {
          const cone = new THREE.Mesh(this.snapGeometry, new THREE.MeshBasicMaterial({ color: SNAP_COLOR }));
          cone.position.copy(toVec(s.position));
          cone.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), toVec(s.facing).normalize());
          cone.userData = { index: e.index, snap: s.name };
          this.snaps.add(cone);
        }
      }
    }
    const shared = new Map<string, THREE.MeshStandardMaterial>();
    const [matrix, one, tint] = [new THREE.Matrix4(), new THREE.Vector3(1, 1, 1), new THREE.Color()];
    for (const [key, list] of instanced) {
      const model = models.get(key)!;
      const mesh = new THREE.InstancedMesh(model, sharedLook(model, textures, list[0].class, shared), list.length);
      list.forEach((e, i) => {
        mesh.setMatrixAt(i, matrix.compose(toVec(e.position), toQuat(e.rotation), one));
        // Instance colour multiplies the material: class colour without textures, red for issues.
        mesh.setColorAt(i, tint.set(bad.has(e.index) ? ISSUE_COLOR : textures ? 0xffffff : classColor(e.class)));
      });
      mesh.userData = { indices: list.map((e) => e.index) };
      mesh.computeBoundingBox();
      mesh.computeBoundingSphere();
      this.station.add(mesh);
    }
  }

  /** Shows the gizmo on a module at `at` (game frame), or hides it with null:
   * `translate` moves it in 10 m steps, `rotate` turns it around its own axes in
   * 5° steps. While dragging, the modules in `carries` follow it (an
   * attached module swings about its `joint`); `onMoved` gets the new position
   * and rotation when the drag ends. */
  setGizmo(
    at: { position: Vec3; rotation: Quat; joint: Vec3 | null; carries: Set<number> } | null,
    onMoved: ((position: Vec3, rotation: Quat) => void) | null = null,
    mode: "translate" | "rotate" = "translate",
  ) {
    this.onGizmoMoved = onMoved;
    if (!at) {
      this.gizmo.detach();
      return;
    }
    this.gizmoPivot = at.joint ? toVec(at.joint) : null;
    this.gizmoCarries = at.carries;
    this.gizmo.setMode(mode);
    this.gizmo.setSpace(mode === "rotate" ? "local" : "world");
    this.gizmo.setRotationSnap(THREE.MathUtils.degToRad(5));
    if (!this.gizmo.dragging) {
      this.gizmoTarget.position.copy(toVec(at.position));
      this.gizmoTarget.quaternion.copy(toQuat(at.rotation));
    }
    this.gizmo.attach(this.gizmoTarget);
  }

  /** Notes where the gizmo and the drawings it carries start a drag. */
  private startDrag() {
    const parts: NonNullable<typeof this.drag>["parts"] = [];
    for (const o of this.station.children) {
      if (o instanceof THREE.InstancedMesh) {
        (o.userData.indices as number[]).forEach((index, i) => {
          if (!this.gizmoCarries.has(index)) return;
          const set = (m: THREE.Matrix4) => {
            o.setMatrixAt(i, m);
            o.instanceMatrix.needsUpdate = true;
            o.boundingSphere = null; // recomputed when next needed (culling, picking)
          };
          parts.push({ matrix: o.getMatrixAt(i, new THREE.Matrix4()), set });
        });
      } else if (this.gizmoCarries.has(o.children[0]?.userData.index)) {
        parts.push({ matrix: o.matrix.clone(), set: (m) => m.decompose(o.position, o.quaternion, o.scale) });
      }
    }
    this.drag = { position: this.gizmoTarget.position.clone(), quaternion: this.gizmoTarget.quaternion.clone(), parts };
  }

  /** Moves the carried drawings with the gizmo while it is dragged; the design changes when the drag ends. */
  private followGizmo() {
    if (!this.drag) return;
    const { position, quaternion, parts } = this.drag;
    const one = new THREE.Vector3(1, 1, 1);
    const turn = this.gizmoTarget.quaternion.clone().multiply(quaternion.clone().invert());
    // An attached module swings about its joint, like the turn the design makes on release.
    const at = this.gizmoPivot
      ? position.clone().sub(this.gizmoPivot).applyQuaternion(turn).add(this.gizmoPivot)
      : this.gizmoTarget.position;
    const delta = new THREE.Matrix4()
      .compose(at, this.gizmoTarget.quaternion, one)
      .multiply(new THREE.Matrix4().compose(position, quaternion, one).invert());
    const m = new THREE.Matrix4();
    for (const p of parts) p.set(m.multiplyMatrices(delta, p.matrix));
  }

  /** Whether the pointer is on (or dragging) the gizmo: clicks there aren't picks. */
  gizmoBusy(): boolean {
    return this.gizmo.dragging || this.gizmo.axis !== null;
  }

  /** Where the cursor points on the ground plane (y = 0), in the game's frame;
   * null when it points above the horizon. */
  groundPoint(clientX: number, clientY: number): Vec3 | null {
    this.setRay(clientX, clientY);
    const hit = this.raycaster.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), new THREE.Vector3());
    return hit ? [hit.x, 0, -hit.z] : null; // back to the game's frame (mirror Z)
  }

  /** What is under the cursor: free snaps take priority over modules. */
  pick(clientX: number, clientY: number): Pick {
    this.setRay(clientX, clientY);
    const snap = this.raycaster.intersectObjects(this.snaps.children, false)[0];
    if (snap) return { kind: "snap", index: snap.object.userData.index, snap: snap.object.userData.snap };
    for (const hit of this.raycaster.intersectObjects(this.station.children, true)) {
      const index = hit.instanceId !== undefined ? hit.object.userData.indices?.[hit.instanceId] : hit.object.userData.index;
      if (index) return { kind: "module", index };
    }
    return null;
  }

  /** Highlights the free snap under the cursor and returns it. */
  hoverSnap(clientX: number, clientY: number): { index: number; snap: string } | null {
    this.setRay(clientX, clientY);
    const hit = this.raycaster.intersectObjects(this.snaps.children, false)[0]?.object as THREE.Mesh | undefined;
    if (this.hovered && this.hovered !== hit) (this.hovered.material as THREE.MeshBasicMaterial).color.set(SNAP_COLOR);
    this.hovered = hit ?? null;
    if (!hit) return null;
    (hit.material as THREE.MeshBasicMaterial).color.set(SNAP_HOVER);
    return { index: hit.userData.index, snap: hit.userData.snap };
  }

  /** Points the camera at the whole station. */
  frame() {
    const box = new THREE.Box3().setFromObject(this.station);
    if (box.isEmpty()) box.set(new THREE.Vector3(-2000, -2000, -2000), new THREE.Vector3(2000, 2000, 2000));
    const sphere = box.getBoundingSphere(new THREE.Sphere());
    const distance = sphere.radius / Math.sin(THREE.MathUtils.degToRad(this.camera.fov / 2)) * 1.1;
    const direction = new THREE.Vector3(1, 0.7, 1).normalize();
    this.controls.target.copy(sphere.center);
    this.camera.position.copy(sphere.center).addScaledVector(direction, distance);
  }

  private setRay(clientX: number, clientY: number) {
    const r = this.renderer.domElement.getBoundingClientRect();
    const ndc = new THREE.Vector2(((clientX - r.left) / r.width) * 2 - 1, -((clientY - r.top) / r.height) * 2 + 1);
    this.raycaster.setFromCamera(ndc, this.camera);
  }

  private resize() {
    const { clientWidth: w, clientHeight: h } = this.container;
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / Math.max(h, 1);
    this.camera.updateProjectionMatrix();
  }
}

const ISSUE_COLOR = 0xff3b30;

function classColor(cls: string | null) {
  return CLASS_COLORS[cls ?? ""] ?? 0xffffff;
}

/** Materials for an instanced model, shared between module types: white base
 * (instance colours tint), or per geometry group the game material's texture. */
function sharedLook(
  model: THREE.BufferGeometry,
  textures: Map<string, DiffuseTexture | null> | null,
  cls: string | null,
  shared: Map<string, THREE.MeshStandardMaterial>,
): THREE.MeshStandardMaterial | THREE.MeshStandardMaterial[] {
  const get = (key: string, make: () => THREE.MeshStandardMaterial) => {
    let m = shared.get(key);
    if (!m) shared.set(key, (m = make()));
    return m;
  };
  if (!textures) return get("plain", () => new THREE.MeshStandardMaterial({ color: 0xffffff, roughness: 0.6, metalness: 0.15 }));
  return (model.userData.materials as string[]).map((n) => {
    const t = textures.get(n.toLowerCase()) ?? null;
    // Untextured groups keep the class colour (their instance colour is white).
    return t ? get(n.toLowerCase(), () => textured(t, 0xffffff, false, 0)) : get(`plain:${cls}`, () => textured(null, classColor(cls), false, 0));
  });
}

/** One module drawn on its own: the selected one (glowing), or one without a real model (boxes). */
function single(
  e: Entry,
  shape: Shape | undefined,
  model: THREE.BufferGeometry | null,
  textures: Map<string, DiffuseTexture | null> | null,
  bad: boolean,
  selected: boolean,
): THREE.Group {
  const group = new THREE.Group();
  group.position.copy(toVec(e.position));
  group.quaternion.copy(toQuat(e.rotation));
  const color = bad ? ISSUE_COLOR : classColor(e.class);
  const material = new THREE.MeshStandardMaterial({
    color,
    roughness: 0.6,
    metalness: 0.15,
    emissive: selected ? 0x664000 : 0x000000,
  });
  if (model) {
    const names: string[] = model.userData.materials ?? [];
    const look = textures
      ? names.map((n) => textured(textures.get(n.toLowerCase()) ?? null, color, bad, material.emissive.getHex()))
      : material;
    const mesh = new THREE.Mesh(model, look);
    mesh.userData = { index: e.index };
    group.add(mesh);
  }
  for (const b of model ? [] : shape?.boxes ?? []) {
    const geometry = new THREE.BoxGeometry(b.half[0] * 2, b.half[1] * 2, b.half[2] * 2);
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.copy(toVec(b.position));
    mesh.quaternion.copy(toQuat(b.rotation));
    mesh.userData = { index: e.index };
    const edges = new THREE.LineSegments(
      new THREE.EdgesGeometry(geometry),
      new THREE.LineBasicMaterial({ color: selected ? 0xffd27a : 0x0b0f16 }),
    );
    mesh.add(edges);
    group.add(mesh);
  }
  return group;
}

function clear(group: THREE.Group) {
  group.traverse((o) => {
    if (o instanceof THREE.Mesh || o instanceof THREE.LineSegments) {
      // Cone and model geometries are shared and cached; only per-render ones are freed.
      if (!(o.geometry instanceof THREE.ConeGeometry) && !o.geometry.userData.shared) o.geometry.dispose();
      for (const m of Array.isArray(o.material) ? o.material : [o.material]) m.dispose();
      if (o instanceof THREE.InstancedMesh) o.dispose();
    }
  });
  group.clear();
}

/** A material showing a game diffuse texture. Without a texture it falls back to
 * `color`; `tinted` keeps `color` over the texture (issues are shown red). */
function textured(t: DiffuseTexture | null, color: number, tinted: boolean, emissive: number): THREE.MeshStandardMaterial {
  if (!t) return new THREE.MeshStandardMaterial({ color, roughness: 0.6, metalness: 0.15, emissive });
  const m = new THREE.MeshStandardMaterial({ map: t.texture, color: tinted ? color : 0xffffff, roughness: 0.55, metalness: 0.2, emissive });
  if (t.singleChannel) {
    // BC4 holds one grey channel in red: use it for all three colours.
    m.onBeforeCompile = (shader) => {
      shader.fragmentShader = shader.fragmentShader.replace(
        "#include <map_fragment>",
        "#ifdef USE_MAP\n  diffuseColor.rgb *= texture2D( map, vMapUv ).rrr;\n#endif",
      );
    };
    m.customProgramCacheKey = () => "x4sd-bc4";
  }
  return m;
}
