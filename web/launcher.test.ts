import { test } from "node:test";
import * as assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { launchClasspathMain } from "./launcher.js";

const decoder = new TextDecoder();
const encoder = new TextEncoder();

async function collect(stream: ReadableStream<Uint8Array>): Promise<string> {
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  while (true) {
    const { value, done } = await reader.read();
    if (done) {
      break;
    }
    chunks.push(value);
  }
  const total = chunks.reduce((sum, chunk) => sum + chunk.length, 0);
  const out = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    out.set(chunk, offset);
    offset += chunk.length;
  }
  return decoder.decode(out);
}

test("launchClasspathMain exposes process-style stdio", async () => {
  const [shimBundle, testJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../test-classes/test.jar", import.meta.url)),
  ]);

  const process = await launchClasspathMain({
    classpath: [new Uint8Array(testJar)],
    mainClass: "ProcessLauncherEchoMain",
    args: ["js"],
    stdio: {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    },
    shimBundle: new Uint8Array(shimBundle),
  });

  const stdoutPromise = collect(process.stdout);
  const stderrPromise = collect(process.stderr);
  const writer = process.stdin.getWriter();
  await writer.write(encoder.encode("abc!"));
  await writer.close();

  const [result, stdout, stderr] = await Promise.all([
    process.wait(),
    stdoutPromise,
    stderrPromise,
  ]);

  assert.equal(stdout, "js>abc");
  assert.equal(stderr, "bang");
  assert.equal(result.exitCode, 0);
  assert.equal(result.uncaughtException, undefined);
});

test("launchClasspathMain forwards inherited stdout/stderr chunks to console", async () => {
  const [shimBundle, testJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../test-classes/test.jar", import.meta.url)),
  ]);

  const stdoutChunks: string[] = [];
  const stderrChunks: string[] = [];
  const originalLog = console.log;
  const originalError = console.error;
  console.log = (chunk: unknown) => {
    stdoutChunks.push(String(chunk));
  };
  console.error = (chunk: unknown) => {
    stderrChunks.push(String(chunk));
  };

  try {
    const process = await launchClasspathMain({
      classpath: [new Uint8Array(testJar)],
      mainClass: "ProcessLauncherEchoMain",
      args: ["bytes"],
      stdio: {
        stdin: "ignore",
        stdout: "inherit",
        stderr: "inherit",
      },
      shimBundle: new Uint8Array(shimBundle),
    });

    const result = await process.wait();
    assert.equal(result.exitCode, 0);
    assert.equal(result.uncaughtException, undefined);
  } finally {
    console.log = originalLog;
    console.error = originalError;
  }

  assert.equal(stdoutChunks.join(""), "ABC");
  assert.equal(stderrChunks.join(""), "DEF");
});

test("launchClasspathMain rejects stdin inherit until host wiring exists", async () => {
  const [shimBundle, testJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../test-classes/test.jar", import.meta.url)),
  ]);

  await assert.rejects(
    launchClasspathMain({
      classpath: [new Uint8Array(testJar)],
      mainClass: "ProcessLauncherEchoMain",
      stdio: {
        stdin: "inherit",
        stdout: "pipe",
        stderr: "pipe",
      },
      shimBundle: new Uint8Array(shimBundle),
    }),
    /stdin="inherit" yet; use "pipe" or "ignore"/,
  );
});

test("launchClasspathMain tolerates closing System.out/System.err", async () => {
  const [shimBundle, testJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../test-classes/test.jar", import.meta.url)),
  ]);

  const process = await launchClasspathMain({
    classpath: [new Uint8Array(testJar)],
    mainClass: "ProcessLauncherEchoMain",
    args: ["close"],
    stdio: {
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    },
    shimBundle: new Uint8Array(shimBundle),
  });

  const [result, stdout, stderr] = await Promise.all([
    process.wait(),
    collect(process.stdout),
    collect(process.stderr),
  ]);

  assert.equal(stdout, "A");
  assert.equal(stderr, "B");
  assert.equal(result.exitCode, 0);
  assert.equal(result.uncaughtException, undefined);
});

test("launchClasspathMain does not emit writes after System.out/System.err close", async () => {
  const [shimBundle, testJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../test-classes/test.jar", import.meta.url)),
  ]);

  const process = await launchClasspathMain({
    classpath: [new Uint8Array(testJar)],
    mainClass: "ProcessLauncherEchoMain",
    args: ["write-after-close"],
    stdio: {
      stdin: "ignore",
      stdout: "pipe",
      stderr: "pipe",
    },
    shimBundle: new Uint8Array(shimBundle),
  });

  const [result, stdout, stderr] = await Promise.all([
    process.wait(),
    collect(process.stdout),
    collect(process.stderr),
  ]);

  assert.equal(stdout, "A");
  assert.equal(stderr, "B");
  assert.equal(result.exitCode, 0);
  assert.equal(result.uncaughtException, undefined);
});

test("launchClasspathMain rejects invalid classpath JARs", async () => {
  const shimBundle = await readFile(new URL("../jdk-shim/bundle.bin", import.meta.url));

  await assert.rejects(
    launchClasspathMain({
      classpath: [new Uint8Array([0x6e, 0x6f, 0x74, 0x2d, 0x61, 0x2d, 0x6a, 0x61, 0x72])],
      mainClass: "ProcessLauncherEchoMain",
      stdio: {
        stdin: "pipe",
        stdout: "pipe",
        stderr: "pipe",
      },
      shimBundle: new Uint8Array(shimBundle),
    }),
    /launchClasspathMain failed during classpath load: Failed to load classpath JAR #0/,
  );
});

test("launchClasspathMain flushes inherited marker streams on checkError", async () => {
  const [shimBundle, testJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../test-classes/test.jar", import.meta.url)),
  ]);

  const stdoutChunks: string[] = [];
  const stderrChunks: string[] = [];
  const originalLog = console.log;
  const originalError = console.error;
  console.log = (chunk: unknown) => {
    stdoutChunks.push(String(chunk));
  };
  console.error = (chunk: unknown) => {
    stderrChunks.push(String(chunk));
  };

  try {
    const process = await launchClasspathMain({
      classpath: [new Uint8Array(testJar)],
      mainClass: "ProcessLauncherEchoMain",
      args: ["check-error-flush"],
      stdio: {
        stdin: "pipe",
        stdout: "inherit",
        stderr: "inherit",
      },
      shimBundle: new Uint8Array(shimBundle),
    });

    await new Promise((resolve) => setTimeout(resolve, 20));
    assert.equal(stdoutChunks.join(""), "A");
    assert.equal(stderrChunks.join(""), "B");

    const writer = process.stdin.getWriter();
    await writer.close();
    const result = await process.wait();
    assert.equal(result.exitCode, 0);
    assert.equal(result.uncaughtException, undefined);
  } finally {
    console.log = originalLog;
    console.error = originalError;
  }
});

test("launchClasspathMain executes live Clojure source through clojure.main -e", async () => {
  const [shimBundle, clojureJar, specJar, coreSpecsJar] = await Promise.all([
    readFile(new URL("../jdk-shim/bundle.bin", import.meta.url)),
    readFile(new URL("../clj-smoke/jars/clojure-1.12.0.jar", import.meta.url)),
    readFile(new URL("../clj-smoke/jars/spec.alpha-0.5.238.jar", import.meta.url)),
    readFile(new URL("../clj-smoke/jars/core.specs.alpha-0.4.74.jar", import.meta.url)),
  ]);

  const process = await launchClasspathMain({
    classpath: [
      new Uint8Array(clojureJar),
      new Uint8Array(specJar),
      new Uint8Array(coreSpecsJar),
    ],
    mainClass: "clojure.main",
    args: [
      "-e",
      "(let [result (load-string (slurp *in*))] (when (some? result) (prn result)))",
    ],
    stdio: {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
    },
    shimBundle: new Uint8Array(shimBundle),
  });

  const stdoutPromise = collect(process.stdout);
  const stderrPromise = collect(process.stderr);
  const writer = process.stdin.getWriter();
  await writer.write(encoder.encode("(println \"Hello, Clojure!\")\n(+ 1 2 3)\n"));
  await writer.close();

  const [result, stdout, stderr] = await Promise.all([
    process.wait(),
    stdoutPromise,
    stderrPromise,
  ]);

  assert.equal(stdout, "Hello, Clojure!\n6\n");
  assert.equal(stderr, "");
  assert.equal(result.exitCode, 0);
  assert.equal(result.uncaughtException, undefined);
});
