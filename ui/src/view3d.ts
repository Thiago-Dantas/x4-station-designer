// The 3D station view: module models (or boxes as a fallback), free-snap cones, picking.
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import type { Entry, Quat, Scene, Shape, Vec3 } from "./api";

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
    this.scene.add(grid, this.station, this.snaps, this.plotBox);
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
   * `textures` maps game material names to diffuse textures (when shown). */
  render(
    scene: Scene,
    shapes: Map<string, Shape>,
    models: Map<string, THREE.BufferGeometry | null>,
    textures: Map<string, DiffuseTexture | null> | null,
    selected: number | null,
    showSnaps: boolean,
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
    for (const e of scene.entries) {
      const key = e.macro.toLowerCase();
      if (models.get(key) && e.index !== selected) instanced.set(key, [...(instanced.get(key) ?? []), e]);
      else this.station.add(single(e, shapes.get(key), models.get(key) ?? null, textures, bad.has(e.index), e.index === selected));
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
