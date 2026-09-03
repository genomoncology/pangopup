#!/usr/bin/env node
// Answer ratchet — the exact cost of declared CLI answers, checked at lint
// time. The declaration is optional, so repositories that have not adopted
// answer budgets stay quiet.

import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const REPO = dirname(dirname(dirname(fileURLToPath(import.meta.url))));
const CONFIG = join(REPO, "sdlc", "answer-ratchet.json");

function fail(message) {
  console.error(`answer-ratchet: ${message}`);
  process.exit(1);
}

function isObject(value) {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

if (!existsSync(CONFIG)) process.exit(0);

let declaration;
try {
  declaration = JSON.parse(readFileSync(CONFIG, "utf8"));
} catch (error) {
  fail(`cannot read ${CONFIG}: ${error.message}`);
}

if (!isObject(declaration)) fail("declaration must be an object");
if (typeof declaration.fixture !== "string" || declaration.fixture.length === 0) {
  fail("fixture must be a repository-relative directory");
}
if (!Array.isArray(declaration.questions)) fail("questions must be an array");

const fixture = resolve(REPO, declaration.fixture);
const fixtureRelative = relative(REPO, fixture);
if (isAbsolute(declaration.fixture) || fixtureRelative === ".." || fixtureRelative.startsWith(`..${sep}`)) {
  fail("fixture must be a repository-relative directory");
}
if (!existsSync(fixture) || !statSync(fixture).isDirectory()) {
  fail(`fixture ${declaration.fixture} is not a directory`);
}

for (const [index, value] of declaration.questions.entries()) {
  if (!isObject(value)) fail(`question ${index + 1} must be an object`);
  const { question, command, env = {}, exit, stdout, stderr } = value;
  if (typeof question !== "string" || question.length === 0) fail(`question ${index + 1} needs a question`);
  if (!Array.isArray(command) || command.length === 0 || command.some((part) => typeof part !== "string" || part.length === 0)) {
    fail(`question ${index + 1} command must be a non-empty argv array`);
  }
  if (!isObject(env) || Object.values(env).some((entry) => typeof entry !== "string")) {
    fail(`question ${index + 1} env must map names to strings`);
  }
  if (!Number.isInteger(exit) || exit < 0 || exit > 255) fail(`question ${index + 1} exit must be an integer from 0 to 255`);
  for (const [stream, budget] of [["stdout", stdout], ["stderr", stderr]]) {
    if (!Number.isSafeInteger(budget) || budget < 0) fail(`question ${index + 1} ${stream} must be a non-negative integer`);
  }

  const result = spawnSync(command[0], command.slice(1), {
    cwd: fixture,
    env: { ...process.env, ...env },
    maxBuffer: Number.MAX_SAFE_INTEGER,
  });
  if (result.error !== undefined) fail(`${question}: could not run command: ${result.error.message}`);

  const actualStdout = result.stdout ?? Buffer.alloc(0);
  const actualStderr = result.stderr ?? Buffer.alloc(0);
  const failures = [];
  if (actualStdout.length !== stdout) failures.push(`stdout was ${actualStdout.length} bytes; expected ${stdout}`);
  if (actualStderr.length !== stderr) failures.push(`stderr was ${actualStderr.length} bytes; expected ${stderr}`);
  if (result.status !== exit) {
    const actualExit = result.signal === null ? result.status : `signal ${result.signal}`;
    failures.push(`exit status was ${actualExit}; expected ${exit}`);
  }
  if (failures.length > 0) fail(`${question}: ${failures.join("; ")}`);
}
