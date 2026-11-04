import init, { Viewer } from "./wasm/viewer_web.js";
import { fetchVerified } from "./content";

const canvas = document.querySelector<HTMLCanvasElement>("#view")!;
const statusLine = document.querySelector<HTMLDivElement>("#status")!;
const SPEED = 20; // blocks per second; Shift triples it
const LOOK = 0.0025; // radians per pixel of mouse movement

const show = (text: string) => (statusLine.textContent = text);

function fitCanvas(viewer: Viewer | null) {
  const ratio = window.devicePixelRatio || 1;
  const width = Math.max(1, Math.floor(canvas.clientWidth * ratio));
  const height = Math.max(1, Math.floor(canvas.clientHeight * ratio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
    viewer?.resize(width, height);
  }
}

async function start() {
  if (!("gpu" in navigator)) throw new Error("此浏览器不支持 WebGPU");
  await init();
  fitCanvas(null);
  const viewer = await Viewer.create(canvas);
  window.addEventListener("resize", () => fitCanvas(viewer));

  const socket = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/viewer`);
  socket.binaryType = "arraybuffer";
  let ready = false;
  const flush = () => viewer.take_outgoing().forEach((bytes) => socket.send(bytes));

  socket.onmessage = async (event) => {
    let result: string;
    try {
      result = viewer.handle_frame(new Uint8Array(event.data as ArrayBuffer));
    } catch (error) {
      show(`协议错误：${error}`);
      socket.close(1002, "protocol error");
      return;
    }
    flush();
    if (result !== "session") return;
    ready = false;
    show("正在下载字典与资源…");
    try {
      const [dictionary, assets] = await Promise.all([
        fetchVerified(viewer.dictionary_hash()!),
        fetchVerified(viewer.assets_hash()!, (loaded) => show(`正在下载资源… ${(loaded / 1048576).toFixed(1)} MiB`)),
      ]);
      show("正在烘焙方块模型…");
      viewer.load_content(dictionary, assets);
    } catch (error) {
      show(`资源加载失败：${error instanceof Error ? error.message : error}`);
      socket.close(1000, "content unavailable");
      return;
    }
    ready = true;
    socket.send(viewer.subscribe());
  };
  socket.onclose = (event) => show(`连接已关闭（${event.code}${event.reason ? `：${event.reason}` : ""}）`);

  // Click to capture the mouse; WASD/Space/Ctrl move, the mouse turns.
  const held = new Set<string>();
  let turn = { yaw: 0, pitch: 0 };
  canvas.addEventListener("click", () => canvas.requestPointerLock());
  window.addEventListener("keydown", (event) => held.add(event.code));
  window.addEventListener("keyup", (event) => held.delete(event.code));
  window.addEventListener("blur", () => held.clear());
  document.addEventListener("mousemove", (event) => {
    if (document.pointerLockElement !== canvas) return;
    turn.yaw -= event.movementX * LOOK;
    turn.pitch -= event.movementY * LOOK;
  });

  let last = performance.now();
  const loop = (now: number) => {
    const step = Math.min(0.1, (now - last) / 1000) * SPEED * (held.has("ShiftLeft") ? 3 : 1);
    last = now;
    const axis = (plus: string, minus: string) => (held.has(plus) ? 1 : 0) - (held.has(minus) ? 1 : 0);
    const crossed = viewer.move_camera(
      axis("KeyW", "KeyS") * step,
      axis("KeyD", "KeyA") * step,
      axis("Space", "ControlLeft") * step,
      turn.yaw,
      turn.pitch,
    );
    turn = { yaw: 0, pitch: 0 };
    if (crossed && ready && socket.readyState === WebSocket.OPEN) socket.send(viewer.subscribe());
    viewer.frame();
    if (ready) show(viewer.status());
    requestAnimationFrame(loop);
  };
  requestAnimationFrame(loop);
}

start().catch((error) => show(`启动失败：${error instanceof Error ? error.message : error}`));
