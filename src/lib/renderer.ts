// WebGL2 nest viewport. Geometry is triangulated once into static GPU buffers; pan/zoom only
// changes two uniforms, so frames stay cheap at any part count (see bench/README.md).
import earcut from 'earcut'
import type { Polygon } from './api'

export type Rgba = [number, number, number, number]

export interface SceneShape {
  polygon: Polygon
  /** World offset (mm) added to every point. */
  dx: number
  dy: number
  fill: Rgba
}

export interface SceneRect {
  x: number
  y: number
  w: number
  h: number
}

export interface Scene {
  sheets: SceneRect[]
  shapes: SceneShape[]
}

export interface Theme {
  background: Rgba
  sheet: Rgba
  sheetEdge: Rgba
  outline: Rgba
}

const VERT = `#version 300 es
in vec2 a_pos;
in vec4 a_color;
uniform vec2 u_center;
uniform vec2 u_scale;
out vec4 v_color;
void main() {
  gl_Position = vec4((a_pos - u_center) * u_scale, 0.0, 1.0);
  v_color = a_color;
}`

const FRAG = `#version 300 es
precision mediump float;
in vec4 v_color;
out vec4 color;
void main() { color = v_color; }`

/** Interleaved x, y, r, g, b, a. */
const STRIDE = 6

class Batch {
  private vao: WebGLVertexArrayObject
  private buf: WebGLBuffer
  count = 0

  constructor(
    private gl: WebGL2RenderingContext,
    program: WebGLProgram,
    private mode: GLenum
  ) {
    this.vao = gl.createVertexArray()
    this.buf = gl.createBuffer()
    gl.bindVertexArray(this.vao)
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buf)
    const pos = gl.getAttribLocation(program, 'a_pos')
    const col = gl.getAttribLocation(program, 'a_color')
    gl.enableVertexAttribArray(pos)
    gl.vertexAttribPointer(pos, 2, gl.FLOAT, false, STRIDE * 4, 0)
    gl.enableVertexAttribArray(col)
    gl.vertexAttribPointer(col, 4, gl.FLOAT, false, STRIDE * 4, 8)
    gl.bindVertexArray(null)
  }

  upload(data: number[]) {
    const gl = this.gl
    gl.bindBuffer(gl.ARRAY_BUFFER, this.buf)
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(data), gl.STATIC_DRAW)
    this.count = data.length / STRIDE
  }

  draw() {
    if (!this.count) return
    this.gl.bindVertexArray(this.vao)
    this.gl.drawArrays(this.mode, 0, this.count)
  }

  dispose() {
    this.gl.deleteBuffer(this.buf)
    this.gl.deleteVertexArray(this.vao)
  }
}

export class NestRenderer {
  private gl: WebGL2RenderingContext
  private program: WebGLProgram
  private uCenter: WebGLUniformLocation | null
  private uScale: WebGLUniformLocation | null
  private fills: Batch
  private lines: Batch
  private frame = 0
  private bounds: SceneRect = { x: 0, y: 0, w: 1, h: 1 }
  /** Camera: world point at the canvas centre, and device px per mm. */
  private cx = 0
  private cy = 0
  private zoom = 1

  constructor(
    private canvas: HTMLCanvasElement,
    private theme: Theme
  ) {
    const gl = canvas.getContext('webgl2', { antialias: true, alpha: false })
    if (!gl) throw new Error('WebGL2 is not available')
    this.gl = gl
    this.program = link(gl, VERT, FRAG)
    this.uCenter = gl.getUniformLocation(this.program, 'u_center')
    this.uScale = gl.getUniformLocation(this.program, 'u_scale')
    this.fills = new Batch(gl, this.program, gl.TRIANGLES)
    this.lines = new Batch(gl, this.program, gl.LINES)
    gl.enable(gl.BLEND)
    gl.blendFunc(gl.SRC_ALPHA, gl.ONE_MINUS_SRC_ALPHA)
  }

  setTheme(theme: Theme, scene: Scene) {
    this.theme = theme
    this.setScene(scene, false)
  }

  /** Rebuild the GPU buffers. `fit` re-frames the camera on the new content. */
  setScene(scene: Scene, fit = true) {
    const fills: number[] = []
    const lines: number[] = []
    const t = this.theme
    let x0 = Infinity
    let y0 = Infinity
    let x1 = -Infinity
    let y1 = -Infinity
    const grow = (x: number, y: number) => {
      x0 = Math.min(x0, x)
      y0 = Math.min(y0, y)
      x1 = Math.max(x1, x)
      y1 = Math.max(y1, y)
    }

    for (const s of scene.sheets) {
      const corners = [
        [s.x, s.y],
        [s.x + s.w, s.y],
        [s.x + s.w, s.y + s.h],
        [s.x, s.y + s.h]
      ]
      for (const i of [0, 1, 2, 0, 2, 3])
        fills.push(corners[i][0], corners[i][1], ...t.sheet)
      for (let i = 0; i < 4; i++) {
        const [a, b] = [corners[i], corners[(i + 1) % 4]]
        lines.push(a[0], a[1], ...t.sheetEdge, b[0], b[1], ...t.sheetEdge)
      }
      grow(s.x, s.y)
      grow(s.x + s.w, s.y + s.h)
    }

    for (const shape of scene.shapes) {
      const rings = [shape.polygon.outer, ...shape.polygon.holes]
      const flat: number[] = []
      const holes: number[] = []
      for (const [k, ring] of rings.entries()) {
        if (k > 0) holes.push(flat.length / 2)
        for (const p of ring) {
          const x = p.x + shape.dx
          const y = p.y + shape.dy
          flat.push(x, y)
          grow(x, y)
        }
        for (let i = 0; i < ring.length; i++) {
          const a = ring[i]
          const b = ring[(i + 1) % ring.length]
          lines.push(
            a.x + shape.dx,
            a.y + shape.dy,
            ...t.outline,
            b.x + shape.dx,
            b.y + shape.dy,
            ...t.outline
          )
        }
      }
      for (const i of earcut(flat, holes, 2))
        fills.push(flat[2 * i], flat[2 * i + 1], ...shape.fill)
    }

    this.fills.upload(fills)
    this.lines.upload(lines)
    if (x0 <= x1) this.bounds = { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    if (fit) this.fit()
    else this.requestDraw()
  }

  /** Frame the whole scene with a small margin. */
  fit() {
    const { x, y, w, h } = this.bounds
    this.cx = x + w / 2
    this.cy = y + h / 2
    const cw = this.canvas.width || 1
    const ch = this.canvas.height || 1
    this.zoom = 0.92 * Math.min(cw / Math.max(w, 1e-6), ch / Math.max(h, 1e-6))
    this.requestDraw()
  }

  /** Pan by a drag delta in CSS px. */
  pan(dx: number, dy: number) {
    const dpr = window.devicePixelRatio || 1
    this.cx -= (dx * dpr) / this.zoom
    this.cy += (dy * dpr) / this.zoom
    this.requestDraw()
  }

  /** Zoom by `factor` keeping the world point under CSS position (px, py) fixed. */
  zoomAt(px: number, py: number, factor: number) {
    const dpr = window.devicePixelRatio || 1
    const sx = px * dpr - this.canvas.width / 2
    const sy = this.canvas.height / 2 - py * dpr
    const wx = this.cx + sx / this.zoom
    const wy = this.cy + sy / this.zoom
    this.zoom = Math.min(Math.max(this.zoom * factor, 1e-4), 1e4)
    this.cx = wx - sx / this.zoom
    this.cy = wy - sy / this.zoom
    this.requestDraw()
  }

  /** Match the drawing buffer to the element size (device px). */
  resize() {
    const dpr = window.devicePixelRatio || 1
    const w = Math.max(1, Math.round(this.canvas.clientWidth * dpr))
    const h = Math.max(1, Math.round(this.canvas.clientHeight * dpr))
    if (w === this.canvas.width && h === this.canvas.height) return
    const first = this.canvas.width <= 300 && this.canvas.height <= 150
    this.canvas.width = w
    this.canvas.height = h
    if (first) this.fit()
    else this.requestDraw()
  }

  requestDraw() {
    if (this.frame) return
    this.frame = requestAnimationFrame(() => {
      this.frame = 0
      this.draw()
    })
  }

  private draw() {
    const gl = this.gl
    gl.viewport(0, 0, this.canvas.width, this.canvas.height)
    gl.clearColor(...this.theme.background)
    gl.clear(gl.COLOR_BUFFER_BIT)
    gl.useProgram(this.program)
    gl.uniform2f(this.uCenter, this.cx, this.cy)
    gl.uniform2f(
      this.uScale,
      (2 * this.zoom) / this.canvas.width,
      (2 * this.zoom) / this.canvas.height
    )
    this.fills.draw()
    this.lines.draw()
  }

  dispose() {
    cancelAnimationFrame(this.frame)
    this.fills.dispose()
    this.lines.dispose()
    this.gl.deleteProgram(this.program)
  }
}

function link(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const compile = (type: GLenum, src: string) => {
    const s = gl.createShader(type)
    if (!s) throw new Error('createShader failed')
    gl.shaderSource(s, src)
    gl.compileShader(s)
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS))
      throw new Error(gl.getShaderInfoLog(s) ?? 'shader compile failed')
    return s
  }
  const p = gl.createProgram()
  gl.attachShader(p, compile(gl.VERTEX_SHADER, vs))
  gl.attachShader(p, compile(gl.FRAGMENT_SHADER, fs))
  gl.linkProgram(p)
  if (!gl.getProgramParameter(p, gl.LINK_STATUS))
    throw new Error(gl.getProgramInfoLog(p) ?? 'program link failed')
  return p
}
