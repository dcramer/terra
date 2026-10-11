import assert from "node:assert/strict";
import { test } from "node:test";
import { presignPut } from "./studio.ts";
import { chargesIn, projectIdOf, slug } from "./tripo.ts";

test("signs an upload as the client that worked against Tripo's bucket did", () => {
  const url = presignPut("tripo-data", "tripo-studio/20261012/5e0c347f-eaa0/input.png", "us-west-2", "ASIAEXAMPLE", "secret/key+x", "tok/en+==", new Date("2026-10-11T22:45:56Z"));
  assert.equal(url, "https://tripo-data.s3.us-west-2.amazonaws.com/tripo-studio/20261012/5e0c347f-eaa0/input.png?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Credential=ASIAEXAMPLE%2F20261011%2Fus-west-2%2Fs3%2Faws4_request&X-Amz-Date=20261011T224556Z&X-Amz-Expires=900&X-Amz-Security-Token=tok%2Fen%2B%3D%3D&X-Amz-SignedHeaders=host&X-Amz-Signature=9446ed7050f3412913b1f0c32fe44292164974c92ca7e22809a7d81f4d482b11");
});

test("counts fulfilled charges only, an operation billed in parts once per record", () => {
  const charge = (operator_id: string, amount: number, status: string, txn_type = "charge") =>
    ({ operator_id, amount, status, txn_type, create_time: "2026-10-11T22:17:58Z" });
  const got = chargesIn([
    charge("02a7ec14-a07f-4445-aa4c-a9d743574f4f", 5, "fulfilled"),
    charge("02a7ec14-a07f-4445-aa4c-a9d743574f4f", 30, "fulfilled"),
    charge("6e7d9e9c-2a91-4289-a725-34ae7e5ccb35", 10, "pending"),
    charge("3c2bce2b-c710-4d71-89fe-2437d43ea455", 25, "cancelled"),
    charge("00000000-0000-0000-0000-000000000000", 3000, "fulfilled", "payment"),
  ]);
  assert.deepEqual(got, [
    { operation: "02a7ec14-a07f-4445-aa4c-a9d743574f4f", credits: 5, day: "2026-10-11" },
    { operation: "02a7ec14-a07f-4445-aa4c-a9d743574f4f", credits: 30, day: "2026-10-11" },
  ]);
});

test("takes a project from its id or any Studio URL", () => {
  const id = "758cdffb-92b7-4818-a89e-f54ed4fc664b";
  assert.equal(projectIdOf(id), id);
  assert.equal(projectIdOf(`https://studio.tripo3d.ai/workspace/rigging/${id}`), id);
  assert.throws(() => projectIdOf("crate.glb"), /not a project id/);
  assert.throws(() => projectIdOf(undefined), /give a project/);
});

test("names a file after the first words of a prompt", () => {
  assert.equal(slug("A weathered supply crate, plain grey background!"), "a-weathered-supply-crate-plain-grey");
  assert.equal(slug("!!!"), "tripo");
});
