// A JSON Schema (2020-12) validator for the keywords our data schemas use, with no dependency
// (AGENTS.md: tooling without dependencies where possible). It knows type, const, enum, pattern,
// minLength, items, minItems, maxItems, contains with minContains and maxContains, required,
// properties, additionalProperties, oneOf, anyOf, allOf, not, if/then/else and local $ref; a schema
// using any other keyword is refused, so a schema never passes data a keyword it ignores would
// refuse. Errors name the field by its JSON pointer and say what it must be (D556). A oneOf
// reports the branch that came closest, as ajv's best-match does.

export type Fault = { at: string; says: string };

const ANNOTATIONS = new Set(["$schema", "$id", "title", "description", "$defs", "$comment"]);
const KNOWN = new Set([
  "type", "const", "enum", "pattern", "minLength", "items", "minItems", "maxItems", "contains", "minContains",
  "maxContains", "required", "properties", "additionalProperties", "oneOf", "anyOf", "allOf", "not", "if", "then",
  "else", "$ref",
]);

type Schema = Record<string, any> | boolean;

const typeOf = (v: unknown): string =>
  v === null ? "null" : Array.isArray(v) ? "array" : typeof v === "number" ? (Number.isInteger(v) ? "integer" : "number") : typeof v;

const typeMatches = (v: unknown, t: string) => {
  const got = typeOf(v);
  return got === t || (t === "number" && got === "integer");
};

const shown = (v: unknown) => JSON.stringify(v)?.slice(0, 60);

export function validate(root: Record<string, any>, data: unknown): Fault[] {
  const resolve = (ref: string): Schema => {
    if (!ref.startsWith("#/")) throw new Error(`schema: only local $ref is supported, not ${ref}`);
    return ref
      .slice(2)
      .split("/")
      .reduce((s: any, k) => {
        if (s?.[k] === undefined) throw new Error(`schema: $ref ${ref} names nothing`);
        return s[k];
      }, root);
  };

  const check = (schema: Schema, v: any, at: string): Fault[] => {
    if (schema === true) return [];
    if (schema === false) return [{ at, says: "is not allowed here" }];
    for (const k of Object.keys(schema))
      if (!KNOWN.has(k) && !ANNOTATIONS.has(k)) throw new Error(`schema: keyword ${k} is not supported by tools/items/schema.ts; add it there`);
    const out: Fault[] = [];
    if (schema.$ref) out.push(...check(resolve(schema.$ref), v, at));
    if (schema.type !== undefined) {
      const types: string[] = Array.isArray(schema.type) ? schema.type : [schema.type];
      if (!types.some((t) => typeMatches(v, t))) return [...out, { at, says: `must be ${types.join(" or ")}, not ${typeOf(v)}` }];
    }
    if ("const" in schema && JSON.stringify(v) !== JSON.stringify(schema.const)) out.push({ at, says: `must be ${shown(schema.const)}, not ${shown(v)}` });
    if (schema.enum && !schema.enum.some((e: unknown) => JSON.stringify(e) === JSON.stringify(v)))
      out.push({ at, says: `must be one of ${schema.enum.map(shown).join(", ")}, not ${shown(v)}` });
    if (typeof v === "string") {
      if (schema.minLength !== undefined && v.length < schema.minLength)
        out.push({ at, says: `is too short (${v.length} characters, at least ${schema.minLength}): say it in words, never a blank` });
      if (schema.pattern && !new RegExp(schema.pattern, "u").test(v)) out.push({ at, says: `must match ${schema.pattern}, not ${shown(v)}` });
    }
    if (Array.isArray(v)) {
      if (schema.minItems !== undefined && v.length < schema.minItems) out.push({ at, says: `needs at least ${schema.minItems} items, has ${v.length}` });
      if (schema.maxItems !== undefined && v.length > schema.maxItems) out.push({ at, says: `takes at most ${schema.maxItems} items, has ${v.length}` });
      if (schema.items !== undefined) v.forEach((x, i) => out.push(...check(schema.items, x, `${at}/${i}`)));
      if (schema.contains !== undefined) {
        const n = v.filter((x, i) => check(schema.contains, x, `${at}/${i}`).length === 0).length;
        const lo = schema.minContains ?? 1;
        if (n < lo) out.push({ at, says: `needs at least ${lo} items matching its contains, has ${n}` });
        if (schema.maxContains !== undefined && n > schema.maxContains)
          out.push({ at, says: `takes at most ${schema.maxContains} items matching its contains (${shown(schema.contains)}), has ${n}` });
      }
    }
    if (typeOf(v) === "object") {
      for (const k of schema.required ?? []) if (!(k in v)) out.push({ at: `${at}/${k}`, says: "is missing: every field is filled, a fact with its source or open with its owner and why" });
      const props = schema.properties ?? {};
      for (const [k, x] of Object.entries(v)) {
        if (k in props) out.push(...check(props[k], x, `${at}/${k}`));
        else if (schema.additionalProperties === false) out.push({ at: `${at}/${k}`, says: `is not a field here (allowed: ${Object.keys(props).join(", ")})` });
        else if (typeof schema.additionalProperties === "object") out.push(...check(schema.additionalProperties, x, `${at}/${k}`));
      }
    }
    for (const s of schema.allOf ?? []) out.push(...check(s, v, at));
    if (schema.anyOf) {
      const tries = schema.anyOf.map((s: Schema) => check(s, v, at));
      if (!tries.some((t: Fault[]) => t.length === 0)) out.push(...closest(tries));
    }
    if (schema.oneOf) {
      const tries: Fault[][] = schema.oneOf.map((s: Schema) => check(s, v, at));
      const passed = tries.filter((t) => t.length === 0).length;
      if (passed === 0) out.push(...closest(tries));
      else if (passed > 1) out.push({ at, says: `matches ${passed} of its oneOf's shapes; it must match exactly one` });
    }
    if (schema.not !== undefined && check(schema.not, v, at).length === 0) out.push({ at, says: `must not be ${shown(schema.not)}` });
    if (schema.if !== undefined) {
      const branch = check(schema.if, v, at).length === 0 ? schema.then : schema.else;
      if (branch !== undefined) out.push(...check(branch, v, at));
    }
    return out;
  };

  return check(root, data, "");
}

// The branch with the fewest faults, the deepest first on a tie: the shape the data meant.
function closest(tries: Fault[][]): Fault[] {
  const depth = (t: Fault[]) => Math.max(...t.map((f) => f.at.split("/").length));
  return [...tries].sort((a, b) => a.length - b.length || depth(b) - depth(a))[0];
}
