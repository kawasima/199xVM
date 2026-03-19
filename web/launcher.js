const JVM_MODULE_CANDIDATES = [
  "../jvm-core/pkg/jvm_core.js",
  "./pkg/jvm_core.js",
];

const WASM_CANDIDATES = [
  "../jvm-core/pkg/jvm_core_bg.wasm",
  "./pkg/jvm_core_bg.wasm",
];

const SHIM_BUNDLE_CANDIDATES = [
  "../jdk-shim/bundle.bin",
  "./bundle/shim.bin",
];

let jvmModulePromise = null;
let defaultShimBundlePromise = null;

function normalizeChunk(chunk) {
  if (chunk instanceof Uint8Array) {
    return chunk;
  }
  if (chunk instanceof ArrayBuffer) {
    return new Uint8Array(chunk);
  }
  if (ArrayBuffer.isView(chunk)) {
    return new Uint8Array(chunk.buffer, chunk.byteOffset, chunk.byteLength);
  }
  throw new TypeError("Expected Uint8Array-compatible chunk");
}

function frameClasspathJars(classpath) {
  const jars = (classpath ?? []).map(normalizeChunk);
  const total = jars.reduce((sum, jar) => sum + 4 + jar.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const jar of jars) {
    const size = jar.length >>> 0;
    out[offset++] = (size >>> 24) & 0xff;
    out[offset++] = (size >>> 16) & 0xff;
    out[offset++] = (size >>> 8) & 0xff;
    out[offset++] = size & 0xff;
    out.set(jar, offset);
    offset += jar.length;
  }
  return out;
}

async function readBytes(url) {
  if (url.protocol === "file:") {
    const { readFile } = await import("node:fs/promises");
    return new Uint8Array(await readFile(url));
  }
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

async function firstAvailableBytes(specs) {
  let lastError = null;
  for (const spec of specs) {
    const url = new URL(spec, import.meta.url);
    try {
      return await readBytes(url);
    } catch (error) {
      lastError = error;
    }
  }
  throw lastError ?? new Error("No candidate asset could be loaded");
}

async function loadJvmModule() {
  if (!jvmModulePromise) {
    jvmModulePromise = (async () => {
      let lastError = null;
      for (const spec of JVM_MODULE_CANDIDATES) {
        const url = new URL(spec, import.meta.url);
        try {
          const mod = await import(url.href);
          const wasmBytes = await firstAvailableBytes(WASM_CANDIDATES);
          await mod.default({ module_or_path: wasmBytes });
          return mod;
        } catch (error) {
          lastError = error;
        }
      }
      throw lastError ?? new Error("Failed to load jvm_core.js");
    })();
  }
  return jvmModulePromise;
}

async function loadShimBundle(explicitShimBundle) {
  if (explicitShimBundle) {
    return normalizeChunk(explicitShimBundle);
  }
  if (!defaultShimBundlePromise) {
    defaultShimBundlePromise = firstAvailableBytes(SHIM_BUNDLE_CANDIDATES);
  }
  return defaultShimBundlePromise;
}

function normalizeStdioMode(value, name) {
  if (value === "pipe" || value === "ignore" || value === "inherit") {
    return value;
  }
  throw new TypeError(`launchClasspathMain stdio.${name} must be "pipe", "ignore", or "inherit"`);
}

function resolveLauncherStdio(stdio = {}) {
  const publicStdio = {
    stdin: normalizeStdioMode(stdio.stdin ?? "pipe", "stdin"),
    stdout: normalizeStdioMode(stdio.stdout ?? "pipe", "stdout"),
    stderr: normalizeStdioMode(stdio.stderr ?? "pipe", "stderr"),
  };
  if (publicStdio.stdin === "inherit") {
    throw new Error(
      'launchClasspathMain does not support stdio.stdin="inherit" yet; use "pipe" or "ignore"',
    );
  }
  return {
    publicStdio,
    lowLevelStdio: {
      stdin: publicStdio.stdin,
      stdout: publicStdio.stdout === "inherit" ? "pipe" : publicStdio.stdout,
      stderr: publicStdio.stderr === "inherit" ? "pipe" : publicStdio.stderr,
    },
  };
}

class LauncherProcessHandle {
  #inner;
  #stdio;
  #stdoutDecoder = new TextDecoder();
  #stderrDecoder = new TextDecoder();
  #stdoutController;
  #stderrController;
  #waitPromise;
  #resolveWait;
  #pumpScheduled = false;
  #settled = false;

  constructor(inner, stdio) {
    this.#inner = inner;
    this.#stdio = stdio;
    this.stdin = new WritableStream({
      write: async (chunk) => {
        if (stdio.stdin === "ignore") {
          return;
        }
        this.#inner.write_stdin(normalizeChunk(chunk));
        this.#schedulePump();
      },
      close: async () => {
        if (stdio.stdin !== "ignore") {
          this.#inner.close_stdin();
          this.#schedulePump();
        }
      },
      abort: async () => {
        if (stdio.stdin !== "ignore") {
          this.#inner.close_stdin();
          this.#schedulePump();
        }
      },
    });
    this.stdout = new ReadableStream({
      start: (controller) => {
        this.#stdoutController = controller;
      },
    });
    this.stderr = new ReadableStream({
      start: (controller) => {
        this.#stderrController = controller;
      },
    });
    this.#waitPromise = new Promise((resolve) => {
      this.#resolveWait = resolve;
    });
    this.#schedulePump();
  }

  wait() {
    return this.#waitPromise;
  }

  kill() {
    if (this.#settled) {
      return;
    }
    this.#inner.kill();
    this.#schedulePump();
  }

  #schedulePump() {
    if (this.#pumpScheduled || this.#settled) {
      return;
    }
    this.#pumpScheduled = true;
    setTimeout(() => {
      this.#pumpScheduled = false;
      this.#pumpLoop();
    }, 0);
  }

  #pumpLoop() {
    if (this.#settled) {
      return;
    }

    let status = "running";
    for (let i = 0; i < 32 && status === "running"; i++) {
      status = this.#inner.pump(64);
    }

    this.#drainOutput();

    if (status === "running") {
      this.#schedulePump();
      return;
    }

    if (status === "waiting" && !this.#inner.is_exited()) {
      return;
    }

    this.#finalize();
  }

  #drainOutput() {
    const stdout = this.#inner.take_stdout();
    if (stdout.length > 0) {
      if (this.#stdio.stdout === "pipe") {
        this.#stdoutController.enqueue(stdout);
      } else if (this.#stdio.stdout === "inherit") {
        this.#forwardInheritedChunk(false, stdout);
      }
    }
    const stderr = this.#inner.take_stderr();
    if (stderr.length > 0) {
      if (this.#stdio.stderr === "pipe") {
        this.#stderrController.enqueue(stderr);
      } else if (this.#stdio.stderr === "inherit") {
        this.#forwardInheritedChunk(true, stderr);
      }
    }
  }

  #finalize() {
    if (this.#settled) {
      return;
    }
    this.#settled = true;
    this.#drainOutput();
    this.#flushInheritedDecoder(false);
    this.#flushInheritedDecoder(true);
    this.#stdoutController.close();
    this.#stderrController.close();
    this.#resolveWait({
      exitCode: this.#inner.exit_code(),
      uncaughtException: this.#inner.uncaught_exception() ?? undefined,
    });
  }

  #forwardInheritedChunk(isErr, chunk) {
    const decoder = isErr ? this.#stderrDecoder : this.#stdoutDecoder;
    const text = decoder.decode(chunk, { stream: true });
    if (text.length === 0) {
      return;
    }
    if (isErr) {
      console.error(text);
    } else {
      console.log(text);
    }
  }

  #flushInheritedDecoder(isErr) {
    const mode = isErr ? this.#stdio.stderr : this.#stdio.stdout;
    if (mode !== "inherit") {
      return;
    }
    const decoder = isErr ? this.#stderrDecoder : this.#stdoutDecoder;
    const text = decoder.decode();
    if (text.length === 0) {
      return;
    }
    if (isErr) {
      console.error(text);
    } else {
      console.log(text);
    }
  }
}

/**
 * Launch `main(String[])` from a classpath of JAR bytes.
 *
 * `classpath` currently accepts an array of JAR byte arrays. Exploded directory
 * classpath entries are out of scope for this first launcher slice.
 */
export async function launchClasspathMain({
  classpath,
  mainClass,
  args = [],
  stdio,
  jvmModule: explicitJvmModule,
  shimBundle,
} = {}) {
  if (!Array.isArray(classpath) || classpath.length === 0) {
    throw new TypeError("launchClasspathMain requires a non-empty classpath array");
  }
  if (!mainClass) {
    throw new TypeError("launchClasspathMain requires mainClass");
  }

  const [jvmModule, resolvedShimBundle] = await Promise.all([
    explicitJvmModule ? Promise.resolve(explicitJvmModule) : loadJvmModule(),
    loadShimBundle(shimBundle),
  ]);

  const { publicStdio, lowLevelStdio } = resolveLauncherStdio(stdio);
  const framedClasspath = frameClasspathJars(classpath);
  const inner = jvmModule.launchClasspathMainLowLevel(
    resolvedShimBundle,
    framedClasspath,
    mainClass,
    args,
    lowLevelStdio.stdin,
    lowLevelStdio.stdout,
    lowLevelStdio.stderr,
  );
  return new LauncherProcessHandle(inner, publicStdio);
}

export async function launchJar({ jar, args = [], stdio, shimBundle } = {}) {
  if (!jar) {
    throw new TypeError("launchJar requires jar");
  }
  throw new Error("launchJar is not implemented yet; use launchClasspathMain for now.");
}
