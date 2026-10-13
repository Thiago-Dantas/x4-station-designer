// The 3D station view: module models (or boxes as a fallback), free-snap cones, picking.
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Quat, Scene, Shape, Vec3 } from "./api";

// The game is left-handed (Y up), Three.js right-handed: mirror Z. A rotation
// q = (w, x, y, z) becomes S·R·S with S = diag(1, 1, -1), i.e. (w, -x, -y, z).
export const toVec = (v: Vec3) => new THREE.Vector3(v[0], v[1], -v[2]);
export const toQuat = (q: Quat) => new THREE.Quaternion(-q[1], -q[2], q[3], q[0]);

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

/** Geometry from `module_model` bytes, mirrored into Three.js space; null when empty. */
export function modelGeometry(buffer: ArrayBuffer): THREE.BufferGeometry | null {
  const header = new Uint32Array(buffer, 0, 2);
  const [vertices, indices] = [header[0], header[1]];
  if (indices === 0) return null;
  const positions = new Float32Array(buffer, 8, vertices * 3).slice();
  const normals = new Float32Array(buffer, 8 + vertices * 12, vertices * 3).slice();
  for (let i = 2; i < positions.length; i += 3) {
    positions[i] = -positions[i];
    normals[i] = -normals[i];
  }
  // The mirror also turns the game's clockwise triangles counter-clockwise, as Three.js expects.
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geometry.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
  geometry.setIndex(new THREE.BufferAttribute(new Uint32Array(buffer, 8 + vertices * 24, indices).slice(), 1));
  geometry.computeBoundingSphere();
  geometry.userData.shared = true;
  return geometry;
}

export type Pick = { kind: "module"; index: number } | { kind: "snap"; index: number; snap: string } | null;

const SNAP_COLOR = 0x37d0c0;
const SNAP_HOVER = 0xffe066;

export class View3D {
  private renderer: THREE.WebGLRenderer;
  private scene = new THREE.Scene();
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private station = new THREE.Group();
  private snaps = new THREE.Group();
  private raycaster = new THREE.Raycaster();
  private hovered: THREE.Mesh | null = null;
  private snapGeometry = new THREE.ConeGeometry(55, 160, 16).translate(0, 80, 0);
  /** WASD keys held down; they pan like a right-mouse drag. */
  private panKeys = new Set<string>();
  private clock = new THREE.Clock();

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
    this.scene.add(grid, this.station, this.snaps);
    new ResizeObserver(() => this.resize()).observe(container);
    this.resize();
    this.bindPanKeys();
    this.renderer.setAnimationLoop(() => {
      this.keyPan(this.clock.getDelta());
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
    });
  }

  private bindPanKeys() {
    const codes = ["KeyW", "KeyA", "KeyS", "KeyD"];
    const typing = (ev: KeyboardEvent) =>
      ev.target instanceof HTMLInputElement || ev.target instanceof HTMLSelectElement || ev.target instanceof HTMLTextAreaElement;
    window.addEventListener("keydown", (ev) => {
      if (codes.includes(ev.code) && !typing(ev) && !ev.ctrlKey && !ev.altKey) this.panKeys.add(ev.code);
    });
    window.addEventListener("keyup", (ev) => this.panKeys.delete(ev.code));
    window.addEventListener("blur", () => this.panKeys.clear());
  }

  /** Pans in screen space, like OrbitControls' right-drag; speed scales with the zoom. */
  private keyPan(seconds: number) {
    if (this.panKeys.size === 0) return;
    const held = (code: string) => (this.panKeys.has(code) ? 1 : 0);
    const right = held("KeyD") - held("KeyA");
    const up = held("KeyW") - held("KeyS");
    if (right === 0 && up === 0) return;
    const step = this.camera.position.distanceTo(this.controls.target) * 0.8 * Math.min(seconds, 0.1);
    const offset = new THREE.Vector3()
      .addScaledVector(new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 0), right * step)
      .addScaledVector(new THREE.Vector3().setFromMatrixColumn(this.camera.matrix, 1), up * step);
    this.camera.position.add(offset);
    this.controls.target.add(offset);
  }

  /** Redraws the station. `shapes` must hold every macro in the scene; `models`
   * holds real models where they are loaded (boxes are drawn otherwise). */
  render(scene: Scene, shapes: Map<string, Shape>, models: Map<string, THREE.BufferGeometry | null>, selected: number | null, showSnaps: boolean) {
    clear(this.station);
    clear(this.snaps);
    this.hovered = null;
    const bad = new Set(scene.issues.map((i) => i.index).filter((i) => i !== null));
    for (const e of scene.entries) {
      const shape = shapes.get(e.macro.toLowerCase());
      const group = new THREE.Group();
      group.position.copy(toVec(e.position));
      group.quaternion.copy(toQuat(e.rotation));
      const color = bad.has(e.index) ? 0xff3b30 : CLASS_COLORS[e.class ?? ""] ?? 0xffffff;
      const material = new THREE.MeshStandardMaterial({
        color,
        roughness: 0.6,
        metalness: 0.15,
        emissive: e.index === selected ? 0x664000 : 0x000000,
      });
      const model = models.get(e.macro.toLowerCase());
      if (model) {
        const mesh = new THREE.Mesh(model, material);
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
          new THREE.LineBasicMaterial({ color: e.index === selected ? 0xffd27a : 0x0b0f16 }),
        );
        mesh.add(edges);
        group.add(mesh);
      }
      this.station.add(group);
      if (showSnaps) {
        for (const s of e.free_snaps) {
          const cone = new THREE.Mesh(this.snapGeometry, new THREE.MeshBasicMaterial({ color: SNAP_COLOR }));
          cone.position.copy(toVec(s.position));
          cone.quaternion.setFromUnitVectors(new THREE.Vector3(0, 1, 0), toVec(s.facing).normalize());
          cone.userData = { index: e.index, snap: s.name };
          this.snaps.add(cone);
        }
      }
    }
  }

  /** What is under the cursor: free snaps take priority over modules. */
  pick(clientX: number, clientY: number): Pick {
    this.setRay(clientX, clientY);
    const snap = this.raycaster.intersectObjects(this.snaps.children, false)[0];
    if (snap) return { kind: "snap", index: snap.object.userData.index, snap: snap.object.userData.snap };
    const module = this.raycaster.intersectObjects(this.station.children, true).find((h) => h.object.userData.index);
    return module ? { kind: "module", index: module.object.userData.index } : null;
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

function clear(group: THREE.Group) {
  group.traverse((o) => {
    if (o instanceof THREE.Mesh || o instanceof THREE.LineSegments) {
      // Cone and model geometries are shared and cached; only per-render ones are freed.
      if (!(o.geometry instanceof THREE.ConeGeometry) && !o.geometry.userData.shared) o.geometry.dispose();
      (o.material as THREE.Material).dispose();
    }
  });
  group.clear();
}
