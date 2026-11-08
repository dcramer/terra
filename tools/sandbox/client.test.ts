// The client's own checks (D632): a shader the engine could not make is named from the run's
// log, and a log without one names none.
//
//   node --test tools/sandbox/client.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { shaderFaults } from "./client.ts";

test("a shader with an unresolved import is named with what was not found, once", () => {
  const log = [
    "INFO terra_world: playing",
    "WARN bevy_shader::shader_cache: Shader `shaders/portal_veil.wesl` has an unresolved import:",
    "error: module not found: `bevy_pbr::mesh_view_bindings` (no shader registered for this module path)",
    "WARN bevy_shader::shader_cache: Shader `shaders/portal_veil.wesl` has an unresolved import:",
    "error: module not found: `bevy_pbr::mesh_view_bindings` (no shader registered for this module path)",
  ].join("\n");
  assert.deepEqual(shaderFaults(log), ["shaders/portal_veil.wesl: module not found: `bevy_pbr::mesh_view_bindings` (no shader registered for this module path)"]);
});

test("a run whose shaders were all made names none", () => {
  assert.deepEqual(shaderFaults("INFO Loaded in 3.0 s: Preparing shaders 197 of 197\nWARN freeze: frame 5 took 117 ms"), []);
});
