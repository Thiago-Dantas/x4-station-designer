"""Minimal Chrome DevTools Protocol client for driving the builder's WebView2,
standard library only. Used by ui_smoke.py and link_smoke.py."""
import base64
import json
import os
import socket
import struct
import subprocess
import tempfile
import time
import urllib.request
import zlib


class Cdp:
    """Minimal DevTools client over a WebSocket (RFC 6455, text frames only)."""

    def __init__(self, ws_url):
        host_port, path = ws_url[len("ws://"):].split("/", 1)
        host, port = host_port.split(":")
        self.sock = socket.create_connection((host, int(port)))
        key = base64.b64encode(os.urandom(16)).decode()
        self.sock.sendall((f"GET /{path} HTTP/1.1\r\nHost: {host_port}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n"
                           f"Sec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\n\r\n").encode())
        response = b""
        while b"\r\n\r\n" not in response:
            response += self.sock.recv(4096)
        assert b" 101 " in response.split(b"\r\n")[0], response
        self.buffer = response.split(b"\r\n\r\n", 1)[1]
        self.next_id = 0

    def _recv_exact(self, n):
        while len(self.buffer) < n:
            self.buffer += self.sock.recv(1 << 20)
        data, self.buffer = self.buffer[:n], self.buffer[n:]
        return data

    def _recv_message(self):
        payload = b""
        while True:
            b0, b1 = self._recv_exact(2)
            n = b1 & 0x7F
            if n == 126:
                n = struct.unpack(">H", self._recv_exact(2))[0]
            elif n == 127:
                n = struct.unpack(">Q", self._recv_exact(8))[0]
            payload += self._recv_exact(n)
            if b0 & 0x80:
                return json.loads(payload)

    def call(self, method, **params):
        self.next_id += 1
        data = json.dumps({"id": self.next_id, "method": method, "params": params}).encode()
        mask = os.urandom(4)
        header = bytes([0x81]) + (bytes([0x80 | len(data)]) if len(data) < 126 else bytes([0x80 | 126]) + struct.pack(">H", len(data)))
        self.sock.sendall(header + mask + bytes(b ^ mask[i % 4] for i, b in enumerate(data)))
        while True:
            msg = self._recv_message()
            if msg.get("id") == self.next_id:
                if "error" in msg:
                    raise RuntimeError(msg["error"])
                return msg["result"]

    def js(self, expression):
        r = self.call("Runtime.evaluate", expression=expression, awaitPromise=True, returnByValue=True)
        if "exceptionDetails" in r:
            raise RuntimeError(r["exceptionDetails"])
        return r["result"].get("value")

    def wait_for(self, expression, timeout=20):
        end = time.time() + timeout
        while time.time() < end:
            try:
                if self.js(expression):
                    return
            except RuntimeError:
                pass  # page still loading (element not there yet)
            time.sleep(0.25)
        raise TimeoutError(expression)

    def screenshot(self, name):
        png = base64.b64decode(self.call("Page.captureScreenshot", format="png")["data"])
        if name:
            open(name, "wb").write(png)
        return png

    def hold_key(self, code, key, seconds):
        """Presses a key (e.g. "KeyD", "d" or "ArrowLeft", "ArrowLeft"), holds it, releases it."""
        arrows = {"ArrowLeft": 37, "ArrowUp": 38, "ArrowRight": 39, "ArrowDown": 40}
        vk = arrows.get(key) or ord(key.upper())
        self.call("Input.dispatchKeyEvent", type="keyDown", code=code, key=key, windowsVirtualKeyCode=vk)
        time.sleep(seconds)
        self.call("Input.dispatchKeyEvent", type="keyUp", code=code, key=key, windowsVirtualKeyCode=vk)

    def click(self, x, y):
        for kind in ("mouseMoved", "mousePressed", "mouseReleased"):
            self.call("Input.dispatchMouseEvent", type=kind, x=x, y=y, button="left", clickCount=1)


def decode_png(png):
    """RGBA rows of an 8-bit RGB/RGBA PNG (enough for Chrome screenshots)."""
    pos, idat, width, height, channels = 8, b"", 0, 0, 4
    while pos < len(png):
        n, kind = struct.unpack(">I4s", png[pos:pos + 8])
        data = png[pos + 8:pos + 8 + n]
        if kind == b"IHDR":
            width, height, _, color = struct.unpack(">IIBB", data[:10])
            channels = 4 if color == 6 else 3
        elif kind == b"IDAT":
            idat += data
        pos += 12 + n
    raw, stride, rows, prev = zlib.decompress(idat), width * channels, [], bytearray(width * channels)
    for y in range(height):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - channels] if i >= channels else 0
            b, c = prev[i], prev[i - channels] if i >= channels else 0
            if f == 1: line[i] = (line[i] + a) & 255
            elif f == 2: line[i] = (line[i] + b) & 255
            elif f == 3: line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        rows.append(line)
        prev = line
    return width, height, channels, rows


def find_color(png, rgb, box):
    """Centre of the first blob of pixels close to `rgb` inside box = (x0, y0, x1, y1)."""
    w, h, ch, rows = decode_png(png)
    xs, ys = [], []
    for y in range(box[1], min(box[3], h), 2):
        row = rows[y]
        for x in range(box[0], min(box[2], w), 2):
            if all(abs(row[x * ch + k] - rgb[k]) < 30 for k in range(3)):
                xs.append(x)
                ys.append(y)
    if not xs:
        return None
    # One blob only (several cones share the colour): the pixels near the first match,
    # which is a cone's thin tip; their centre lies well inside the cone.
    near = [(x, y) for x, y in zip(xs, ys) if abs(x - xs[0]) < 60 and abs(y - ys[0]) < 60]
    return (sum(x for x, _ in near) // len(near), sum(y for _, y in near) // len(near))


# One port for every script: WebView2 refuses to start (and Tauri panics) when a
# previous instance's processes still hold the profile with different arguments.
PORT = 9333


def link_port_in_use():
    """Whether a builder already listens for agents (live link, 127.0.0.1:47823)."""
    try:
        socket.create_connection(("127.0.0.1", 47823), timeout=0.3).close()
        return True
    except OSError:
        return False


def start_builder(exe, *args):
    """Starts x4sd in builder mode with remote debugging; returns (process, Cdp)."""
    port = PORT
    # Own WebView2 profile: never collides with (or changes the settings of) a
    # builder the user has open.
    profile = os.path.join(tempfile.gettempdir(), "x4sd-ui-tests-webview2")
    env = dict(os.environ, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}",
               WEBVIEW2_USER_DATA_FOLDER=profile)
    app = subprocess.Popen([exe, *args], env=env)
    for _ in range(80):
        try:
            targets = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json"))
            page = next(t for t in targets if t.get("type") == "page")
            return app, Cdp(page["webSocketDebuggerUrl"])
        except Exception:
            time.sleep(0.25)
    app.terminate()
    raise TimeoutError("builder did not start")
