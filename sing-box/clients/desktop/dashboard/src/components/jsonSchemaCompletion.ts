import {
  CompletionContext,
  startCompletion,
  type Completion,
  type CompletionResult,
} from "@codemirror/autocomplete";
import { EditorState, Transaction, type Extension } from "@codemirror/state";
import type { EditorView } from "@codemirror/view";

export type JsonSchemaRoot = Record<string, unknown>;

type SchemaNode = Record<string, unknown>;

type PathStep = { key: string } | "item";

type MemberValue = string | number | boolean;

interface ObjectFrame {
  kind: "object";
  step: PathStep | null;
  openedAt: number;
  phase: "key" | "afterKey" | "value" | "afterValue";
  lastKey: string;
  currentKey: string | null;
  keys: Set<string>;
  memberValues: Map<string, MemberValue>;
}

interface ArrayFrame {
  kind: "array";
  step: PathStep | null;
  openedAt: number;
  phase: "value" | "afterValue";
}

type Frame = ObjectFrame | ArrayFrame;

interface DocumentTag {
  path: string;
  tag: string;
}

interface CursorState {
  stack: Frame[];
  slot: "key" | "value" | null;
  phase: ObjectFrame["phase"] | ArrayFrame["phase"] | null;
  structural: boolean;
  key: string | null;
  prefix: string;
  quoteOpen: boolean;
  from: number;
  ownKey: string | null;
  documentTags: DocumentTag[];
}

const referencePaths: Record<string, string[]> = {
  outbound: ["outbounds/[]", "endpoints/[]"],
  inbound: ["inbounds/[]"],
  dns_server: ["dns/servers/[]"],
  rule_set: ["route/rule_set/[]"],
  certificate_provider: ["certificate_providers/[]"],
  http_client: ["http_clients/[]"],
  network_namespace: ["network_namespaces/[]"],
};

const taggedEntryPaths = new Set(Object.values(referencePaths).flat());

function framePath(stack: Frame[], end: number): string | null {
  const segments: string[] = [];
  for (let i = 1; i <= end; i++) {
    const step = stack[i].step;
    if (step === null || (step !== "item" && step.key === "")) {
      return null;
    }
    segments.push(step === "item" ? "[]" : step.key);
  }
  return segments.join("/");
}

const bareTokenBreak = new Set([" ", "\t", "\r", "\n", "{", "}", "[", "]", ",", ":", '"', "/"]);

function parseBareValue(token: string): MemberValue | null {
  if (token === "true") {
    return true;
  }
  if (token === "false") {
    return false;
  }
  const numeric = Number(token);
  return token !== "" && !Number.isNaN(numeric) ? numeric : null;
}

function scanDocument(text: string, pos: number): CursorState | null {
  const stack: Frame[] = [];
  const documentTags: DocumentTag[] = [];
  let cursor: CursorState | null = null;
  let cursorSeen = false;
  let i = 0;

  const top = (): Frame | undefined => stack[stack.length - 1];

  const snapshot = (
    slot: "key" | "value",
    key: string | null,
    prefix: string,
    quoteOpen: boolean,
    from: number,
    ownKey: string | null,
  ): void => {
    cursorSeen = true;
    cursor = {
      stack: stack.slice(),
      slot,
      phase: null,
      structural: false,
      key,
      prefix,
      quoteOpen,
      from,
      ownKey,
      documentTags,
    };
  };

  const snapshotStructural = (): void => {
    cursorSeen = true;
    const frame = top();
    if (frame === undefined) {
      return;
    }
    let slot: "key" | "value" | null = null;
    let key: string | null = null;
    if (frame.kind === "object") {
      if (frame.phase === "key" || frame.phase === "afterValue") {
        slot = "key";
      } else if (frame.phase === "value") {
        slot = "value";
        key = frame.currentKey;
      }
    } else if (frame.phase === "value" || frame.phase === "afterValue") {
      slot = "value";
    }
    cursor = {
      stack: stack.slice(),
      slot,
      phase: frame.phase,
      structural: true,
      key,
      prefix: "",
      quoteOpen: false,
      from: pos,
      ownKey: null,
      documentTags,
    };
  };

  const openFrame = (kind: "object" | "array", openedAt: number): void => {
    const parent = top();
    let step: PathStep | null = null;
    if (parent !== undefined) {
      if (parent.kind === "object") {
        step = { key: parent.phase === "value" && parent.currentKey !== null ? parent.currentKey : "" };
        parent.phase = "afterValue";
      } else {
        step = "item";
        parent.phase = "afterValue";
      }
    }
    if (kind === "object") {
      stack.push({
        kind,
        step,
        openedAt,
        phase: "key",
        lastKey: "",
        currentKey: null,
        keys: new Set(),
        memberValues: new Map(),
      });
    } else {
      stack.push({ kind, step, openedAt, phase: "value" });
    }
  };

  const handleString = (content: string, quoteStart: number, contentEnd: number): void => {
    const frame = top();
    const cursorInside = !cursorSeen && pos > quoteStart && pos <= contentEnd;
    const prefix = cursorInside ? text.slice(quoteStart + 1, pos) : "";
    if (frame === undefined) {
      if (cursorInside) {
        cursorSeen = true;
      }
      return;
    }
    if (frame.kind === "object") {
      if (frame.phase === "value") {
        if (cursorInside) {
          snapshot("value", frame.currentKey, prefix, true, quoteStart + 1, null);
        }
        if (frame.currentKey !== null) {
          frame.memberValues.set(frame.currentKey, content);
          if (frame.currentKey === "tag" && content !== "") {
            const path = framePath(stack, stack.length - 1);
            if (path !== null && taggedEntryPaths.has(path)) {
              documentTags.push({ path, tag: content });
            }
          }
        }
        frame.phase = "afterValue";
      } else {
        if (cursorInside) {
          snapshot("key", null, prefix, true, quoteStart + 1, content);
        }
        frame.keys.add(content);
        frame.lastKey = content;
        frame.phase = "afterKey";
      }
    } else {
      if (cursorInside) {
        snapshot("value", null, prefix, true, quoteStart + 1, null);
      }
      frame.phase = "afterValue";
    }
  };

  const handleBare = (token: string, start: number, end: number): void => {
    const frame = top();
    const cursorInside = !cursorSeen && pos > start && pos <= end;
    const prefix = cursorInside ? text.slice(start, pos) : "";
    if (frame === undefined) {
      if (cursorInside) {
        cursorSeen = true;
      }
      return;
    }
    if (frame.kind === "object") {
      if (frame.phase === "value") {
        if (cursorInside) {
          snapshot("value", frame.currentKey, prefix, false, start, null);
        }
        if (frame.currentKey !== null) {
          const value = parseBareValue(token);
          if (value !== null) {
            frame.memberValues.set(frame.currentKey, value);
          }
        }
        frame.phase = "afterValue";
      } else if (frame.phase === "key" || frame.phase === "afterValue") {
        if (cursorInside) {
          snapshot("key", null, prefix, false, start, token);
        }
        frame.lastKey = token;
        frame.phase = "afterKey";
      } else if (cursorInside) {
        cursorSeen = true;
      }
    } else {
      if (cursorInside) {
        snapshot("value", null, prefix, false, start, null);
      }
      frame.phase = "afterValue";
    }
  };

  while (i < text.length) {
    const character = text[i];
    if (character === " " || character === "\t" || character === "\r" || character === "\n") {
      i++;
      continue;
    }
    if (!cursorSeen && pos <= i) {
      snapshotStructural();
    }
    if (character === "/" && (text[i + 1] === "/" || text[i + 1] === "*")) {
      const commentStart = i;
      let insideComment: boolean;
      if (text[i + 1] === "/") {
        i += 2;
        while (i < text.length && text[i] !== "\n") {
          i++;
        }
        insideComment = pos <= i;
      } else {
        i += 2;
        while (i < text.length && !(text[i] === "*" && text[i + 1] === "/")) {
          i++;
        }
        if (i < text.length) {
          i += 2;
          insideComment = pos < i;
        } else {
          insideComment = true;
        }
      }
      if (!cursorSeen && pos > commentStart && insideComment) {
        cursorSeen = true;
      }
    } else if (character === "/") {
      i++;
    } else if (character === "{") {
      openFrame("object", i);
      i++;
    } else if (character === "[") {
      openFrame("array", i);
      i++;
    } else if (character === "}" || character === "]") {
      stack.pop();
      i++;
    } else if (character === ":") {
      const frame = top();
      if (frame !== undefined && frame.kind === "object" && frame.phase === "afterKey") {
        frame.currentKey = frame.lastKey;
        frame.phase = "value";
      }
      i++;
    } else if (character === ",") {
      const frame = top();
      if (frame !== undefined) {
        frame.phase = frame.kind === "object" ? "key" : "value";
      }
      i++;
    } else if (character === '"') {
      const quoteStart = i;
      i++;
      while (i < text.length) {
        const inner = text[i];
        if (inner === "\\" && i + 1 < text.length) {
          i += 2;
          continue;
        }
        if (inner === '"' || inner === "\n") {
          break;
        }
        i++;
      }
      const contentEnd = i;
      handleString(text.slice(quoteStart + 1, contentEnd), quoteStart, contentEnd);
      if (text[i] === '"') {
        i++;
      }
    } else {
      const start = i;
      while (i < text.length && !bareTokenBreak.has(text[i])) {
        i++;
      }
      handleBare(text.slice(start, i), start, i);
    }
  }
  if (!cursorSeen) {
    snapshotStructural();
  }
  return cursor;
}

function asObject(value: unknown): SchemaNode | null {
  if (typeof value === "object" && value !== null && !Array.isArray(value)) {
    return value as SchemaNode;
  }
  return null;
}

function asArray(value: unknown): unknown[] | null {
  return Array.isArray(value) ? value : null;
}

function resolveNode(root: JsonSchemaRoot, node: unknown): SchemaNode | null {
  let current = asObject(node);
  for (let depth = 0; current !== null && typeof current.$ref === "string" && depth < 32; depth++) {
    const reference = current.$ref;
    if (!reference.startsWith("#/")) {
      return null;
    }
    let target: unknown = root;
    for (const segment of reference.slice(2).split("/")) {
      const container = asObject(target);
      if (container === null) {
        return null;
      }
      target = container[segment.replaceAll("~1", "/").replaceAll("~0", "~")];
    }
    current = asObject(target);
  }
  return current;
}

function referenceName(node: unknown): string | null {
  const container = asObject(node);
  if (container === null || typeof container.$ref !== "string") {
    return null;
  }
  const segments = container.$ref.split("/");
  return segments[segments.length - 1];
}

function collectScalarSchemas(root: JsonSchemaRoot, node: unknown, out: SchemaNode[], guard: SchemaNode[]): void {
  const resolved = resolveNode(root, node);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  if (resolved.const !== undefined || resolved.enum !== undefined || typeof resolved.type === "string") {
    out.push(resolved);
  }
  for (const unionKey of ["oneOf", "anyOf", "allOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches !== null) {
      for (const branch of branches) {
        collectScalarSchemas(root, branch, out, guard);
      }
    }
  }
  guard.pop();
}

function propertyAllowsValue(root: JsonSchemaRoot, property: unknown, value: MemberValue): boolean {
  const scalars: SchemaNode[] = [];
  collectScalarSchemas(root, property, scalars, []);
  for (const scalar of scalars) {
    if (scalar.const === value) {
      return true;
    }
    const values = asArray(scalar.enum);
    if (values !== null && values.includes(value)) {
      return true;
    }
  }
  return false;
}

function propertyHasConstant(root: JsonSchemaRoot, property: unknown): boolean {
  const scalars: SchemaNode[] = [];
  collectScalarSchemas(root, property, scalars, []);
  return scalars.some((scalar) => scalar.const !== undefined || scalar.enum !== undefined);
}

function branchMatches(
  root: JsonSchemaRoot,
  branch: unknown,
  siblings: ReadonlyMap<string, MemberValue>,
  guard: SchemaNode[],
): boolean {
  const resolved = resolveNode(root, branch);
  if (resolved === null || guard.includes(resolved)) {
    return false;
  }
  guard.push(resolved);
  try {
    const properties = asObject(resolved.properties);
    if (properties !== null) {
      for (const [key, value] of siblings) {
        if (key in properties && propertyAllowsValue(root, properties[key], value)) {
          return true;
        }
      }
    }
    for (const unionKey of ["allOf", "oneOf", "anyOf"]) {
      const branches = asArray(resolved[unionKey]);
      if (branches !== null && branches.some((nested) => branchMatches(root, nested, siblings, guard))) {
        return true;
      }
    }
    return false;
  } finally {
    guard.pop();
  }
}

function branchConflicts(
  root: JsonSchemaRoot,
  branch: unknown,
  siblings: ReadonlyMap<string, MemberValue>,
  guard: SchemaNode[],
): boolean {
  const resolved = resolveNode(root, branch);
  if (resolved === null || guard.includes(resolved)) {
    return false;
  }
  guard.push(resolved);
  try {
    const properties = asObject(resolved.properties);
    if (properties !== null) {
      for (const [key, value] of siblings) {
        if (
          key in properties &&
          propertyHasConstant(root, properties[key]) &&
          !propertyAllowsValue(root, properties[key], value)
        ) {
          return true;
        }
      }
    }
    const conjunction = asArray(resolved.allOf);
    if (conjunction !== null && conjunction.some((part) => branchConflicts(root, part, siblings, guard))) {
      return true;
    }
    for (const unionKey of ["oneOf", "anyOf"]) {
      const branches = asArray(resolved[unionKey]);
      if (
        branches !== null &&
        branches.length > 0 &&
        branches.every((nested) => branchConflicts(root, nested, siblings, guard))
      ) {
        return true;
      }
    }
    return false;
  } finally {
    guard.pop();
  }
}

function selectBranches(
  root: JsonSchemaRoot,
  branches: unknown[],
  siblings: ReadonlyMap<string, MemberValue>,
): { matched: unknown[]; viable: unknown[] } {
  const viable = branches.filter((branch) => !branchConflicts(root, branch, siblings, []));
  const pool = viable.length > 0 ? viable : branches;
  const matched = pool.filter((branch) => branchMatches(root, branch, siblings, []));
  return { matched, viable: pool };
}

interface KeyInfo {
  schemas: unknown[];
  constant: boolean;
}

function mergeConstantMaps(target: Map<string, unknown[]>, source: ReadonlyMap<string, unknown[]>): void {
  for (const [key, schemas] of source) {
    const existing = target.get(key);
    if (existing === undefined) {
      target.set(key, [...schemas]);
    } else {
      existing.push(...schemas);
    }
  }
}

function intersectConstantMaps(maps: Map<string, unknown[]>[]): Map<string, unknown[]> {
  const result = new Map<string, unknown[]>();
  if (maps.length === 0) {
    return result;
  }
  for (const [key, schemas] of maps[0]) {
    if (maps.every((map) => map.has(key))) {
      result.set(key, [...schemas]);
      for (const map of maps.slice(1)) {
        result.get(key)?.push(...(map.get(key) ?? []));
      }
    }
  }
  return result;
}

function collectRequiredConstants(
  root: JsonSchemaRoot,
  branch: unknown,
  out: Map<string, unknown[]>,
  guard: SchemaNode[],
): void {
  const resolved = resolveNode(root, branch);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  const required = asArray(resolved.required);
  const properties = asObject(resolved.properties);
  if (required !== null && properties !== null) {
    for (const key of required) {
      if (typeof key === "string" && key in properties && propertyHasConstant(root, properties[key])) {
        const schemas = out.get(key);
        if (schemas === undefined) {
          out.set(key, [properties[key]]);
        } else {
          schemas.push(properties[key]);
        }
      }
    }
  }
  const conjunction = asArray(resolved.allOf);
  if (conjunction !== null) {
    for (const part of conjunction) {
      collectRequiredConstants(root, part, out, guard);
    }
  }
  for (const unionKey of ["oneOf", "anyOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches === null || branches.length === 0) {
      continue;
    }
    const branchConstants = branches.map((nested) => {
      const constants = new Map<string, unknown[]>();
      collectRequiredConstants(root, nested, constants, guard);
      return constants;
    });
    if (branchConstants.every((constants) => constants.size > 0)) {
      mergeConstantMaps(out, intersectConstantMaps(branchConstants));
    }
  }
  guard.pop();
}

function collectKeys(
  root: JsonSchemaRoot,
  node: unknown,
  siblings: ReadonlyMap<string, MemberValue>,
  out: Map<string, KeyInfo>,
  guard: SchemaNode[],
): void {
  const resolved = resolveNode(root, node);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  const properties = asObject(resolved.properties);
  if (properties !== null) {
    for (const [key, property] of Object.entries(properties)) {
      let info = out.get(key);
      if (info === undefined) {
        info = { schemas: [], constant: propertyHasConstant(root, property) };
        out.set(key, info);
      }
      info.schemas.push(property);
    }
  }
  const conjunction = asArray(resolved.allOf);
  if (conjunction !== null) {
    for (const part of conjunction) {
      collectKeys(root, part, siblings, out, guard);
    }
  }
  for (const unionKey of ["oneOf", "anyOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches === null) {
      continue;
    }
    const { matched, viable } = selectBranches(root, branches, siblings);
    if (matched.length > 0 || viable.length === 1) {
      for (const branch of matched.length > 0 ? matched : viable) {
        collectKeys(root, branch, siblings, out, guard);
      }
      continue;
    }
    const requiredConstants = viable.map((branch) => {
      const constants = new Map<string, unknown[]>();
      collectRequiredConstants(root, branch, constants, []);
      return constants;
    });
    const admitting = viable.filter((_, index) => requiredConstants[index].size === 0);
    if (admitting.length > 0) {
      for (const branch of admitting) {
        collectKeys(root, branch, siblings, out, guard);
      }
      continue;
    }
    let discriminators = intersectConstantMaps(requiredConstants);
    if (discriminators.size === 0) {
      discriminators = new Map();
      for (const constants of requiredConstants) {
        mergeConstantMaps(discriminators, constants);
      }
    }
    for (const [key, schemas] of discriminators) {
      const existing = out.get(key);
      if (existing === undefined) {
        out.set(key, { schemas: [...schemas], constant: true });
      } else {
        existing.schemas.push(...schemas);
        existing.constant = true;
      }
    }
  }
  guard.pop();
}

function collectValueSchemas(
  root: JsonSchemaRoot,
  node: unknown,
  siblings: ReadonlyMap<string, MemberValue>,
  key: string,
  out: unknown[],
  guard: SchemaNode[],
): void {
  const resolved = resolveNode(root, node);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  const properties = asObject(resolved.properties);
  if (properties !== null && key in properties) {
    out.push(properties[key]);
  }
  const conjunction = asArray(resolved.allOf);
  if (conjunction !== null) {
    for (const part of conjunction) {
      collectValueSchemas(root, part, siblings, key, out, guard);
    }
  }
  for (const unionKey of ["oneOf", "anyOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches === null) {
      continue;
    }
    const { matched, viable } = selectBranches(root, branches, siblings);
    for (const branch of matched.length > 0 ? matched : viable) {
      collectValueSchemas(root, branch, siblings, key, out, guard);
    }
  }
  guard.pop();
}

function collectArrayExamples(root: JsonSchemaRoot, node: unknown, out: unknown[][], guard: SchemaNode[]): void {
  const resolved = resolveNode(root, node);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  const examples = asArray(resolved.examples);
  if (examples !== null) {
    for (const example of examples) {
      if (Array.isArray(example)) {
        out.push(example);
      }
    }
  }
  for (const unionKey of ["oneOf", "anyOf", "allOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches !== null) {
      for (const branch of branches) {
        collectArrayExamples(root, branch, out, guard);
      }
    }
  }
  guard.pop();
}

function collectItemSchemas(root: JsonSchemaRoot, node: unknown, out: unknown[], guard: SchemaNode[]): void {
  const resolved = resolveNode(root, node);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  if (resolved.items !== undefined) {
    out.push(resolved.items);
  }
  for (const unionKey of ["oneOf", "anyOf", "allOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches !== null) {
      for (const branch of branches) {
        collectItemSchemas(root, branch, out, guard);
      }
    }
  }
  guard.pop();
}

function describeSchema(schema: unknown, depth = 0): string | undefined {
  if (depth > 4) {
    return undefined;
  }
  const name = referenceName(schema);
  if (name !== null) {
    return name;
  }
  const container = asObject(schema);
  if (container === null) {
    return undefined;
  }
  if (container.type === "array") {
    const inner = container.items === undefined ? undefined : describeSchema(container.items, depth + 1);
    return inner === undefined ? "array" : `${inner}[]`;
  }
  if (typeof container.type === "string") {
    return container.type;
  }
  if (container.const !== undefined) {
    return JSON.stringify(container.const);
  }
  const union = asArray(container.anyOf) ?? asArray(container.oneOf);
  if (union !== null && union.length > 0) {
    return describeSchema(union[0], depth + 1);
  }
  return undefined;
}

type ScaffoldKind = "array" | "object" | "string" | "scalar" | "none";

function collectLeafSchemas(root: JsonSchemaRoot, node: unknown, out: SchemaNode[], guard: SchemaNode[]): void {
  const resolved = resolveNode(root, node);
  if (resolved === null || guard.includes(resolved)) {
    return;
  }
  guard.push(resolved);
  let hasUnion = false;
  for (const unionKey of ["oneOf", "anyOf", "allOf"]) {
    const branches = asArray(resolved[unionKey]);
    if (branches !== null) {
      hasUnion = true;
      for (const branch of branches) {
        collectLeafSchemas(root, branch, out, guard);
      }
    }
  }
  if (!hasUnion) {
    out.push(resolved);
  }
  guard.pop();
}

function leafScaffoldKind(leaf: SchemaNode): ScaffoldKind | null {
  if (leaf.type === "array" || leaf.items !== undefined) {
    return "array";
  }
  if (leaf.type === "object" || asObject(leaf.properties) !== null) {
    return "object";
  }
  if (leaf.type === "string") {
    return "string";
  }
  if (leaf.type === "boolean" || leaf.type === "integer" || leaf.type === "number") {
    return "scalar";
  }
  if (leaf.const !== undefined) {
    if (typeof leaf.const === "string") {
      return "string";
    }
    return typeof leaf.const === "number" || typeof leaf.const === "boolean" ? "scalar" : null;
  }
  const values = asArray(leaf.enum);
  if (values !== null && values.length > 0) {
    if (values.every((value) => typeof value === "string")) {
      return "string";
    }
    if (values.every((value) => typeof value === "number" || typeof value === "boolean")) {
      return "scalar";
    }
  }
  return null;
}

function scaffoldKindFor(root: JsonSchemaRoot, schemas: unknown[]): ScaffoldKind {
  const leaves: SchemaNode[] = [];
  for (const schema of schemas) {
    collectLeafSchemas(root, schema, leaves, []);
  }
  let kind: ScaffoldKind | null = null;
  for (const leaf of leaves) {
    const leafKind = leafScaffoldKind(leaf);
    if (leafKind === null || (kind !== null && kind !== leafKind)) {
      return "none";
    }
    kind = leafKind;
  }
  return kind ?? "none";
}

const objectSiblingStart = /^"$/;
const arraySiblingStart = /^["{[\w-]$/;

function nextSiblingStarts(doc: EditorView["state"]["doc"], position: number, container: "object" | "array"): boolean {
  const window = doc.sliceString(position, Math.min(doc.length, position + 200));
  let i = 0;
  while (i < window.length) {
    const character = window[i];
    if (character === " " || character === "\t" || character === "\r" || character === "\n") {
      i++;
    } else if (character === "/" && window[i + 1] === "/") {
      i += 2;
      while (i < window.length && window[i] !== "\n") {
        i++;
      }
    } else if (character === "/" && window[i + 1] === "*") {
      i += 2;
      while (i < window.length && !(window[i] === "*" && window[i + 1] === "/")) {
        i++;
      }
      i = i < window.length ? i + 2 : window.length;
    } else {
      return (container === "object" ? objectSiblingStart : arraySiblingStart).test(character);
    }
  }
  return false;
}

function previousValueEnd(doc: EditorView["state"]["doc"], position: number): number | null {
  const text = doc.sliceString(0, position);
  let last = -1;
  let i = 0;
  while (i < text.length) {
    const character = text[i];
    if (character === " " || character === "\t" || character === "\r" || character === "\n") {
      i++;
    } else if (character === "/" && text[i + 1] === "/") {
      i += 2;
      while (i < text.length && text[i] !== "\n") {
        i++;
      }
    } else if (character === "/" && text[i + 1] === "*") {
      i += 2;
      while (i < text.length && !(text[i] === "*" && text[i + 1] === "/")) {
        i++;
      }
      i = i < text.length ? i + 2 : text.length;
    } else if (character === '"') {
      i++;
      while (i < text.length) {
        const inner = text[i];
        if (inner === "\\" && i + 1 < text.length) {
          i += 2;
          continue;
        }
        if (inner === '"' || inner === "\n") {
          break;
        }
        i++;
      }
      if (text[i] === '"') {
        last = i;
        i++;
      } else {
        last = i - 1;
      }
    } else {
      last = i;
      i++;
    }
  }
  if (last < 0) {
    return null;
  }
  const endCharacter = text[last];
  if (
    endCharacter === '"' ||
    endCharacter === "}" ||
    endCharacter === "]" ||
    (endCharacter >= "0" && endCharacter <= "9")
  ) {
    return last + 1;
  }
  if (/[a-z]/.test(endCharacter)) {
    let wordStart = last;
    while (wordStart > 0 && /[a-z]/.test(text[wordStart - 1])) {
      wordStart--;
    }
    const word = text.slice(wordStart, last + 1);
    if (word === "true" || word === "false" || word === "null") {
      return last + 1;
    }
  }
  return null;
}

function lineIndent(doc: EditorView["state"]["doc"], position: number): string {
  const match = /^[ \t]*/.exec(doc.lineAt(position).text);
  return match === null ? "" : match[0];
}

function inlineCloserPosition(
  doc: EditorView["state"]["doc"],
  openedAt: number,
  opener: "{" | "[",
): number | null {
  const closer = opener === "{" ? "}" : "]";
  const line = doc.lineAt(openedAt);
  const text = line.text;
  let depth = 1;
  let inString = false;
  for (let i = openedAt - line.from + 1; i < text.length; i++) {
    const character = text[i];
    if (inString) {
      if (character === "\\") {
        i++;
      } else if (character === '"') {
        inString = false;
      }
      continue;
    }
    if (character === "/" && text[i + 1] === "/") {
      return null;
    }
    if (character === "/" && text[i + 1] === "*") {
      const commentClose = text.indexOf("*/", i + 2);
      if (commentClose === -1) {
        return null;
      }
      i = commentClose + 1;
      continue;
    }
    if (character === '"') {
      inString = true;
    } else if (character === opener) {
      depth++;
    } else if (character === closer) {
      depth--;
      if (depth === 0) {
        return line.from + i;
      }
    }
  }
  return null;
}

function inlineContainerEnd(
  doc: EditorView["state"]["doc"],
  pos: number,
  kind: "object" | "array",
  after: number,
): { closer: number; indent: string } | null {
  const cursor = scanDocument(doc.toString(), pos);
  if (cursor === null) {
    return null;
  }
  const frame = cursor.stack[cursor.stack.length - 1];
  if (frame === undefined || frame.kind !== kind) {
    return null;
  }
  const closer = inlineCloserPosition(doc, frame.openedAt, kind === "object" ? "{" : "[");
  if (closer === null || closer < after) {
    return null;
  }
  return { closer, indent: lineIndent(doc, frame.openedAt) };
}

function keyApply(label: string, quoteOpen: boolean, kind: ScaffoldKind) {
  return (view: EditorView, _completion: Completion, from: number, to: number): void => {
    const doc = view.state.doc;
    const start = quoteOpen ? from - 1 : from;
    let end = to;
    if (doc.sliceString(end, end + 1) === '"') {
      end++;
    }
    const keyText = JSON.stringify(label);
    let probe = end;
    while (probe < doc.length && (doc.sliceString(probe, probe + 1) === " " || doc.sliceString(probe, probe + 1) === "\t")) {
      probe++;
    }
    if (doc.sliceString(probe, probe + 1) === ":") {
      view.dispatch({
        changes: { from: start, to: end, insert: keyText },
        selection: { anchor: start + keyText.length },
        userEvent: "input.complete",
      });
      return;
    }
    const comma = nextSiblingStarts(doc, end, "object") ? "," : "";
    const leadingCommaAt = previousValueEnd(doc, start);
    const expansion = inlineContainerEnd(doc, to, "object", end);
    const baseIndent = expansion === null ? lineIndent(doc, start) : expansion.indent + "  ";
    const prefix = expansion === null ? "" : "\n" + baseIndent;
    let scaffold: string;
    let cursorBack: number;
    if (kind === "object" || kind === "array") {
      const openCharacter = kind === "object" ? "{" : "[";
      const closeCharacter = kind === "object" ? "}" : "]";
      scaffold = ": " + openCharacter + "\n" + baseIndent + "  \n" + baseIndent + closeCharacter;
      cursorBack = 2 + baseIndent.length;
    } else if (kind === "string") {
      scaffold = ': ""';
      cursorBack = 1;
    } else {
      scaffold = ": ";
      cursorBack = 0;
    }
    let replaceFrom = start;
    if (expansion !== null) {
      while (replaceFrom > 0) {
        const previous = doc.sliceString(replaceFrom - 1, replaceFrom);
        if (previous !== " " && previous !== "\t") {
          break;
        }
        replaceFrom--;
      }
    }
    const changes: { from: number; to?: number; insert: string }[] = [];
    let insert = prefix + keyText + scaffold + comma;
    let cursorOffset = prefix.length + keyText.length + scaffold.length - cursorBack;
    let shift = 0;
    if (leadingCommaAt !== null) {
      if (leadingCommaAt >= replaceFrom) {
        insert = "," + insert;
        cursorOffset++;
      } else {
        changes.push({ from: leadingCommaAt, insert: "," });
        shift = 1;
      }
    }
    if (expansion !== null && expansion.closer === end) {
      insert += "\n" + expansion.indent;
    }
    changes.push({ from: replaceFrom, to: end, insert });
    if (expansion !== null && expansion.closer > end) {
      changes.push({ from: expansion.closer, insert: "\n" + expansion.indent });
    }
    view.dispatch({
      changes,
      selection: { anchor: shift + replaceFrom + cursorOffset },
      userEvent: "input.complete",
    });
    startCompletion(view);
  };
}

function stringValueApply(value: string, quoteOpen: boolean, container: "object" | "array") {
  return (view: EditorView, _completion: Completion, from: number, to: number): void => {
    const doc = view.state.doc;
    const start = quoteOpen ? from - 1 : from;
    let end = to;
    if (doc.sliceString(end, end + 1) === '"') {
      end++;
    }
    const valueText = JSON.stringify(value);
    const comma = nextSiblingStarts(doc, end, container) ? "," : "";
    const leadingCommaAt = previousValueEnd(doc, start);
    const changes: { from: number; to?: number; insert: string }[] = [];
    if (leadingCommaAt !== null) {
      changes.push({ from: leadingCommaAt, insert: "," });
    }
    changes.push({ from: start, to: end, insert: valueText + comma });
    const shift = leadingCommaAt === null ? 0 : 1;
    view.dispatch({
      changes,
      selection: { anchor: shift + start + valueText.length },
      userEvent: "input.complete",
    });
  };
}

function arrayValueApply(values: unknown[]) {
  return (view: EditorView, _completion: Completion, from: number, to: number): void => {
    const doc = view.state.doc;
    const comma = nextSiblingStarts(doc, to, "object") ? "," : "";
    const leadingCommaAt = previousValueEnd(doc, from);
    const baseIndent = lineIndent(doc, from);
    let body = "[\n";
    for (const [index, value] of values.entries()) {
      body += baseIndent + "  " + JSON.stringify(value) + (index < values.length - 1 ? "," : "") + "\n";
    }
    body += baseIndent + "]";
    const changes: { from: number; to?: number; insert: string }[] = [];
    if (leadingCommaAt !== null) {
      changes.push({ from: leadingCommaAt, insert: "," });
    }
    changes.push({ from, to, insert: body + comma });
    const shift = leadingCommaAt === null ? 0 : 1;
    view.dispatch({
      changes,
      selection: { anchor: shift + from + body.length },
      userEvent: "input.complete",
    });
  };
}

const arrayExampleLabelLimit = 48;

function arrayExampleCompletions(root: JsonSchemaRoot, schemas: unknown[]): Completion[] {
  const examples: unknown[][] = [];
  for (const schema of schemas) {
    collectArrayExamples(root, schema, examples, []);
  }
  const options: Completion[] = [];
  const seen = new Set<string>();
  for (const example of examples) {
    const compact = JSON.stringify(example);
    if (seen.has(compact)) {
      continue;
    }
    seen.add(compact);
    options.push({
      label:
        compact.length > arrayExampleLabelLimit
          ? compact.slice(0, arrayExampleLabelLimit - 1) + "…"
          : compact,
      type: "constant",
      detail: "example",
      boost: 1,
      apply: arrayValueApply(example),
    });
  }
  return options;
}

function formApply(form: "[]" | "{}" | '""', container: "object" | "array") {
  return (view: EditorView, _completion: Completion, from: number, to: number): void => {
    const doc = view.state.doc;
    const comma = nextSiblingStarts(doc, to, container) ? "," : "";
    const leadingCommaAt = previousValueEnd(doc, from);
    const expansion =
      container === "array" && form !== '""' ? inlineContainerEnd(doc, to, "array", to) : null;
    const baseIndent = expansion === null ? lineIndent(doc, from) : expansion.indent + "  ";
    const prefix = expansion === null ? "" : "\n" + baseIndent;
    let body: string;
    let cursorBack: number;
    if (form === '""') {
      body = '""';
      cursorBack = 1;
    } else {
      const openCharacter = form === "{}" ? "{" : "[";
      const closeCharacter = form === "{}" ? "}" : "]";
      body = openCharacter + "\n" + baseIndent + "  \n" + baseIndent + closeCharacter;
      cursorBack = 2 + baseIndent.length;
    }
    let replaceFrom = from;
    if (expansion !== null) {
      while (replaceFrom > 0) {
        const previous = doc.sliceString(replaceFrom - 1, replaceFrom);
        if (previous !== " " && previous !== "\t") {
          break;
        }
        replaceFrom--;
      }
    }
    const changes: { from: number; to?: number; insert: string }[] = [];
    let insert = prefix + body + comma;
    let cursorOffset = prefix.length + body.length - cursorBack;
    let shift = 0;
    if (leadingCommaAt !== null) {
      if (leadingCommaAt >= replaceFrom) {
        insert = "," + insert;
        cursorOffset++;
      } else {
        changes.push({ from: leadingCommaAt, insert: "," });
        shift = 1;
      }
    }
    if (expansion !== null && expansion.closer === to) {
      insert += "\n" + expansion.indent;
    }
    changes.push({ from: replaceFrom, to, insert });
    if (expansion !== null && expansion.closer > to) {
      changes.push({ from: expansion.closer, insert: "\n" + expansion.indent });
    }
    view.dispatch({
      changes,
      selection: { anchor: shift + replaceFrom + cursorOffset },
      userEvent: "input.complete",
    });
    if (form !== '""') {
      startCompletion(view);
    }
  };
}

function formCompletions(
  root: JsonSchemaRoot,
  schemas: unknown[],
  container: "object" | "array",
  requireMultiple: boolean,
): Completion[] {
  const leaves: SchemaNode[] = [];
  for (const schema of schemas) {
    collectLeafSchemas(root, schema, leaves, []);
  }
  const kinds = new Set<ScaffoldKind>();
  let bareString = false;
  for (const leaf of leaves) {
    const kind = leafScaffoldKind(leaf);
    if (kind !== null) {
      kinds.add(kind);
    }
    if (
      kind === "string" &&
      leaf.const === undefined &&
      leaf.enum === undefined &&
      leaf.examples === undefined
    ) {
      bareString = true;
    }
  }
  if (requireMultiple && kinds.size < 2) {
    return [];
  }
  const options: Completion[] = [];
  if (kinds.has("array")) {
    options.push({
      label: "[]",
      type: "type",
      detail: "array",
      apply: formApply("[]", container),
    });
  }
  if (kinds.has("object")) {
    options.push({
      label: "{}",
      type: "type",
      detail: "object",
      apply: formApply("{}", container),
    });
  }
  if (bareString) {
    options.push({
      label: '""',
      type: "type",
      detail: "string",
      apply: formApply('""', container),
    });
  }
  return options;
}

function referenceCandidates(cursor: CursorState, kind: string): string[] {
  const paths = referencePaths[kind];
  if (paths === undefined) {
    return [];
  }
  const ownTags = new Set<string>();
  for (let i = 1; i < cursor.stack.length; i++) {
    const frame = cursor.stack[i];
    if (frame.kind !== "object") {
      continue;
    }
    const path = framePath(cursor.stack, i);
    if (path !== null && paths.includes(path)) {
      const tag = frame.memberValues.get("tag");
      if (typeof tag === "string") {
        ownTags.add(tag);
      }
    }
  }
  const candidates: string[] = [];
  for (const entry of cursor.documentTags) {
    if (paths.includes(entry.path) && !ownTags.has(entry.tag) && !candidates.includes(entry.tag)) {
      candidates.push(entry.tag);
    }
  }
  return candidates;
}

function scalarCompletions(
  root: JsonSchemaRoot,
  schemas: unknown[],
  quoteOpen: boolean,
  container: "object" | "array",
  references: (kind: string) => string[],
): Completion[] {
  const options = new Map<string, Completion>();
  const scalars: SchemaNode[] = [];
  for (const schema of schemas) {
    collectScalarSchemas(root, schema, scalars, []);
  }
  const addValue = (value: unknown, boost: number, detail?: string): void => {
    if (typeof value === "string") {
      if (value !== "" && !options.has(value)) {
        options.set(value, {
          label: value,
          type: "constant",
          detail,
          boost,
          apply: stringValueApply(value, quoteOpen, container),
        });
      }
    } else if ((typeof value === "number" || typeof value === "boolean") && !quoteOpen) {
      const label = String(value);
      if (!options.has(label)) {
        options.set(label, { label, type: "keyword", detail, boost });
      }
    }
  };
  const referencedKinds = new Set<string>();
  for (const scalar of scalars) {
    const kind = scalar["x-tag-reference"];
    if (typeof kind !== "string" || referencedKinds.has(kind)) {
      continue;
    }
    referencedKinds.add(kind);
    for (const tag of references(kind)) {
      if (!options.has(tag)) {
        options.set(tag, {
          label: tag,
          type: "variable",
          detail: kind,
          boost: 3,
          apply: stringValueApply(tag, quoteOpen, container),
        });
      }
    }
  }
  for (const scalar of scalars) {
    if (scalar.const !== undefined) {
      addValue(scalar.const, 2);
    }
    const values = asArray(scalar.enum);
    if (values !== null) {
      for (const value of values) {
        addValue(value, 2);
      }
    }
    if (scalar.type === "boolean") {
      addValue(true, 2);
      addValue(false, 2);
    }
  }
  for (const scalar of scalars) {
    const examples = asArray(scalar.examples);
    if (examples !== null) {
      for (const example of examples) {
        addValue(example, 1, "example");
      }
    }
  }
  return [...options.values()];
}

const typedValueStart = /^["{[\-0-9tfn]$/;

const closingPairs: Record<string, string> = { "{": "}", "[": "]", '"': '"' };

function hasOrphanCloser(text: string, opener: string): boolean {
  const closer = closingPairs[opener];
  let openCount = 0;
  let closeCount = 0;
  let quoteCount = 0;
  let inString = false;
  for (let i = 0; i < text.length; i++) {
    const character = text[i];
    if (inString) {
      if (character === "\\") {
        i++;
      } else if (character === '"') {
        inString = false;
        quoteCount++;
      } else if (character === "\n") {
        inString = false;
      }
      continue;
    }
    if (character === "/" && (text[i + 1] === "/" || text[i + 1] === "*")) {
      if (text[i + 1] === "/") {
        i++;
        while (i + 1 < text.length && text[i + 1] !== "\n") {
          i++;
        }
      } else {
        i += 2;
        while (i + 1 < text.length && !(text[i] === "*" && text[i + 1] === "/")) {
          i++;
        }
        i++;
      }
    } else if (character === '"') {
      inString = true;
      quoteCount++;
    } else if (character === opener) {
      openCount++;
    } else if (character === closer) {
      closeCount++;
    }
  }
  if (opener === '"') {
    return quoteCount % 2 === 1;
  }
  return closeCount > openCount;
}

export const jsonTypingAssist: Extension = EditorState.transactionFilter.of((tr) => {
  if (!tr.docChanged || !tr.isUserEvent("input.type") || tr.startState.selection.ranges.length !== 1) {
    return tr;
  }
  let insertion: { from: number; length: number; text: string } | null = null;
  let changeCount = 0;
  tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    changeCount++;
    insertion = { from: fromA, length: toA - fromA, text: inserted.toString() };
  });
  if (changeCount !== 1 || insertion === null) {
    return tr;
  }
  const { from, length, text } = insertion as { from: number; length: number; text: string };
  if (length !== 0 || text.length === 0 || text.length > 2) {
    return tr;
  }
  const typed = text[0];
  if (!typedValueStart.test(typed) || (text.length === 2 && text[1] !== closingPairs[typed])) {
    return tr;
  }
  const document = tr.startState.doc;
  const documentText = document.toString();
  const stripPair = text.length === 2 && hasOrphanCloser(documentText, typed);
  let commaAt: number | null = null;
  const cursor = scanDocument(documentText, from);
  if (cursor !== null && cursor.structural) {
    const frame = cursor.stack[cursor.stack.length - 1];
    let eligible = false;
    if (frame !== undefined) {
      if (frame.kind === "object") {
        eligible = typed === '"' && (cursor.phase === "key" || cursor.phase === "afterValue");
      } else {
        eligible = cursor.phase === "value" || cursor.phase === "afterValue";
      }
    }
    if (eligible) {
      commaAt = previousValueEnd(document, from);
    }
  }
  if (!stripPair) {
    if (commaAt === null) {
      return tr;
    }
    return [tr, { changes: { from: commaAt, insert: "," } }];
  }
  const changes: { from: number; insert: string }[] = [];
  if (commaAt !== null) {
    changes.push({ from: commaAt, insert: "," });
  }
  changes.push({ from, insert: typed });
  return {
    changes,
    selection: { anchor: from + (commaAt === null ? 1 : 2) },
    userEvent: "input.type",
    scrollIntoView: true,
  };
});

const newlineInsertion = /^\n[ \t]*(\n[ \t]*)?$/;

export function shouldAutoOpenCompletion(
  tr: Transaction,
  source: (context: CompletionContext) => CompletionResult | null,
): boolean {
  if (!tr.docChanged) {
    return false;
  }
  const event = tr.annotation(Transaction.userEvent);
  if (event !== "input" && event !== "input.type") {
    return false;
  }
  const selection = tr.state.selection;
  if (selection.ranges.length !== 1 || !selection.main.empty) {
    return false;
  }
  let insertion: { from: number; to: number; text: string } | null = null;
  let changeCount = 0;
  tr.changes.iterChanges((fromA, toA, _fromB, _toB, inserted) => {
    changeCount++;
    insertion = { from: fromA, to: toA, text: inserted.toString() };
  });
  if (changeCount !== 1 || insertion === null) {
    return false;
  }
  const { from, to, text } = insertion as { from: number; to: number; text: string };
  if (!newlineInsertion.test(text) || !/^[ \t]*$/.test(tr.startState.doc.sliceString(from, to))) {
    return false;
  }
  const head = selection.main.head;
  if (/\S/.test(tr.state.doc.lineAt(head).text)) {
    return false;
  }
  const cursor = scanDocument(tr.state.doc.toString(), head);
  if (cursor === null || !cursor.structural) {
    return false;
  }
  const frame = cursor.stack[cursor.stack.length - 1];
  if (frame === undefined) {
    return false;
  }
  if (frame.kind === "object" ? cursor.slot !== "key" : cursor.slot !== "value") {
    return false;
  }
  return source(new CompletionContext(tr.state, head, true)) !== null;
}

export function jsonSchemaCompletionSource(
  getSchema: () => JsonSchemaRoot | null,
): (context: CompletionContext) => CompletionResult | null {
  return (context) => {
    const root = getSchema();
    if (root === null) {
      return null;
    }
    const cursor = scanDocument(context.state.doc.toString(), context.pos);
    if (cursor === null || cursor.slot === null) {
      return null;
    }
    if (!context.explicit && !cursor.quoteOpen && cursor.prefix === "") {
      return null;
    }
    let nodes: unknown[] = [root];
    for (let i = 1; i < cursor.stack.length && nodes.length > 0; i++) {
      const frame = cursor.stack[i];
      const parent = cursor.stack[i - 1];
      const step = frame.step;
      if (step === null) {
        return null;
      }
      if (step === "item") {
        const items: unknown[] = [];
        for (const node of nodes) {
          collectItemSchemas(root, node, items, []);
        }
        nodes = items;
      } else {
        if (parent.kind !== "object" || step.key === "") {
          return null;
        }
        const next: unknown[] = [];
        for (const node of nodes) {
          collectValueSchemas(root, node, parent.memberValues, step.key, next, []);
        }
        nodes = next;
      }
    }
    const frame = cursor.stack[cursor.stack.length - 1];
    if (frame === undefined) {
      return null;
    }
    const references = (kind: string): string[] => referenceCandidates(cursor, kind);
    let options: Completion[];
    if (cursor.slot === "key") {
      if (frame.kind !== "object") {
        return null;
      }
      const keys = new Map<string, KeyInfo>();
      for (const node of nodes) {
        collectKeys(root, node, frame.memberValues, keys, []);
      }
      options = [];
      for (const [label, info] of keys) {
        if (frame.keys.has(label) && label !== cursor.ownKey) {
          continue;
        }
        options.push({
          label,
          type: "property",
          detail:
            info.constant && info.schemas.length > 1 ? "string" : describeSchema(info.schemas[0]),
          apply: keyApply(label, cursor.quoteOpen, scaffoldKindFor(root, info.schemas)),
        });
      }
    } else if (frame.kind === "object") {
      if (cursor.key === null) {
        return null;
      }
      const siblings = new Map(frame.memberValues);
      siblings.delete(cursor.key);
      const schemas: unknown[] = [];
      for (const node of nodes) {
        collectValueSchemas(root, node, siblings, cursor.key, schemas, []);
      }
      options = scalarCompletions(root, schemas, cursor.quoteOpen, "object", references);
      if (!cursor.quoteOpen) {
        options.push(...arrayExampleCompletions(root, schemas));
        options.push(...formCompletions(root, schemas, "object", true));
      }
    } else {
      const items: unknown[] = [];
      for (const node of nodes) {
        collectItemSchemas(root, node, items, []);
      }
      options = scalarCompletions(root, items, cursor.quoteOpen, "array", references);
      if (!cursor.quoteOpen) {
        options.push(...formCompletions(root, items, "array", false));
      }
    }
    if (options.length === 0) {
      return null;
    }
    for (const [index, option] of options.entries()) {
      option.sortText = String(index).padStart(4, "0");
    }
    return { from: cursor.from, options };
  };
}
