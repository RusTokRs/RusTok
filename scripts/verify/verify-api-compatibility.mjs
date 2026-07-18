#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const HTTP_METHODS = [
  "get",
  "put",
  "post",
  "delete",
  "options",
  "head",
  "patch",
  "trace",
];

function parseArguments(argv) {
  const options = { selfTest: false };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--self-test") {
      options.selfTest = true;
      continue;
    }
    if (["--base-dir", "--head-dir", "--exceptions"].includes(argument)) {
      const value = argv[index + 1];
      if (!value) throw new Error(`${argument} requires a value`);
      options[argument.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase())] = value;
      index += 1;
      continue;
    }
    throw new Error(`unknown argument: ${argument}`);
  }
  return options;
}

function readJson(file) {
  return JSON.parse(fs.readFileSync(file, "utf8"));
}

function readText(file) {
  return fs.readFileSync(file, "utf8").replaceAll("\r\n", "\n");
}

function stableJson(value) {
  if (Array.isArray(value)) return `[${value.map(stableJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableJson(value[key])}`)
      .join(",")}}`;
  }
  return JSON.stringify(value);
}

function addChange(changes, id, message) {
  if (!changes.has(id)) changes.set(id, { id, message });
}

function pointerGet(document, pointer) {
  if (!pointer.startsWith("#/")) return undefined;
  return pointer
    .slice(2)
    .split("/")
    .map((segment) => segment.replaceAll("~1", "/").replaceAll("~0", "~"))
    .reduce((value, segment) => value?.[segment], document);
}

function resolveObject(document, value) {
  if (!value || typeof value !== "object") return value;
  return typeof value.$ref === "string" ? pointerGet(document, value.$ref) : value;
}

function schemaKind(schema) {
  if (!schema || typeof schema !== "object") return "unknown";
  if (Array.isArray(schema.type)) return schema.type.slice().sort().join("|");
  if (schema.type) return schema.type;
  if (schema.properties || schema.additionalProperties) return "object";
  if (schema.items) return "array";
  if (schema.oneOf) return "oneOf";
  if (schema.anyOf) return "anyOf";
  if (schema.allOf) return "allOf";
  return "unknown";
}

function compareSchema({
  baseDocument,
  headDocument,
  baseSchema,
  headSchema,
  direction,
  location,
  changes,
  seen,
}) {
  if (!baseSchema) return;
  if (!headSchema) {
    addChange(changes, `openapi:schema-removed:${location}`, `${location}: schema was removed`);
    return;
  }

  const baseRef = baseSchema.$ref;
  const headRef = headSchema.$ref;
  if (baseRef && headRef && baseRef !== headRef) {
    addChange(
      changes,
      `openapi:schema-ref:${location}`,
      `${location}: schema reference changed from ${baseRef} to ${headRef}`,
    );
  }

  const resolvedBase = resolveObject(baseDocument, baseSchema);
  const resolvedHead = resolveObject(headDocument, headSchema);
  if (!resolvedBase || !resolvedHead) {
    addChange(
      changes,
      `openapi:schema-resolution:${location}`,
      `${location}: schema reference cannot be resolved in the head contract`,
    );
    return;
  }

  const seenKey = `${direction}:${baseRef || location}:${headRef || location}`;
  if (seen.has(seenKey)) return;
  seen.add(seenKey);

  const baseKind = schemaKind(resolvedBase);
  const headKind = schemaKind(resolvedHead);
  if (baseKind !== headKind) {
    addChange(
      changes,
      `openapi:schema-type:${location}`,
      `${location}: schema type changed from ${baseKind} to ${headKind}`,
    );
    return;
  }

  if ((resolvedBase.format || null) !== (resolvedHead.format || null)) {
    addChange(
      changes,
      `openapi:schema-format:${location}`,
      `${location}: schema format changed from ${resolvedBase.format || "none"} to ${resolvedHead.format || "none"}`,
    );
  }

  const baseEnum = new Set(resolvedBase.enum || []);
  const headEnum = new Set(resolvedHead.enum || []);
  for (const value of baseEnum) {
    if (!headEnum.has(value)) {
      addChange(
        changes,
        `openapi:enum-value:${location}:${JSON.stringify(value)}`,
        `${location}: enum value ${JSON.stringify(value)} was removed`,
      );
    }
  }

  if (baseKind === "array") {
    compareSchema({
      baseDocument,
      headDocument,
      baseSchema: resolvedBase.items,
      headSchema: resolvedHead.items,
      direction,
      location: `${location}[]`,
      changes,
      seen,
    });
  }

  if (baseKind === "object") {
    const baseProperties = resolvedBase.properties || {};
    const headProperties = resolvedHead.properties || {};
    for (const [name, property] of Object.entries(baseProperties)) {
      if (!(name in headProperties)) {
        addChange(
          changes,
          `openapi:property-removed:${location}.${name}`,
          `${location}: property ${name} was removed`,
        );
        continue;
      }
      compareSchema({
        baseDocument,
        headDocument,
        baseSchema: property,
        headSchema: headProperties[name],
        direction,
        location: `${location}.${name}`,
        changes,
        seen,
      });
    }

    const baseRequired = new Set(resolvedBase.required || []);
    const headRequired = new Set(resolvedHead.required || []);
    if (direction === "input") {
      for (const name of headRequired) {
        if (!baseRequired.has(name)) {
          addChange(
            changes,
            `openapi:required-input-added:${location}.${name}`,
            `${location}: input property ${name} became required`,
          );
        }
      }
    } else {
      for (const name of baseRequired) {
        if (!headRequired.has(name)) {
          addChange(
            changes,
            `openapi:required-output-removed:${location}.${name}`,
            `${location}: response property ${name} is no longer guaranteed`,
          );
        }
      }
    }
  }

  for (const keyword of ["oneOf", "anyOf", "allOf", "not"]) {
    if (stableJson(resolvedBase[keyword] || null) !== stableJson(resolvedHead[keyword] || null)) {
      addChange(
        changes,
        `openapi:composition:${location}:${keyword}`,
        `${location}: ${keyword} contract changed`,
      );
    }
  }
}

function parameterKey(parameter) {
  return `${parameter.in || "unknown"}:${parameter.name || "unknown"}`;
}

function collectParameters(document, pathItem, operation) {
  const result = new Map();
  for (const raw of [...(pathItem.parameters || []), ...(operation.parameters || [])]) {
    const parameter = resolveObject(document, raw);
    if (parameter) result.set(parameterKey(parameter), parameter);
  }
  return result;
}

function compareContent({
  baseDocument,
  headDocument,
  baseContent,
  headContent,
  direction,
  location,
  changes,
}) {
  for (const [mediaType, baseMedia] of Object.entries(baseContent || {})) {
    const headMedia = headContent?.[mediaType];
    if (!headMedia) {
      addChange(
        changes,
        `openapi:media-type-removed:${location}:${mediaType}`,
        `${location}: media type ${mediaType} was removed`,
      );
      continue;
    }
    compareSchema({
      baseDocument,
      headDocument,
      baseSchema: baseMedia.schema,
      headSchema: headMedia.schema,
      direction,
      location: `${location}:${mediaType}`,
      changes,
      seen: new Set(),
    });
  }
}

function compareOperation(baseDocument, headDocument, route, method, basePath, headPath, changes) {
  const baseOperation = basePath[method];
  const headOperation = headPath[method];
  const operationLocation = `${method.toUpperCase()} ${route}`;

  if (!headOperation) {
    addChange(
      changes,
      `openapi:operation-removed:${method}:${route}`,
      `${operationLocation}: operation was removed`,
    );
    return;
  }

  if (
    baseOperation.operationId &&
    headOperation.operationId &&
    baseOperation.operationId !== headOperation.operationId
  ) {
    addChange(
      changes,
      `openapi:operation-id:${method}:${route}`,
      `${operationLocation}: operationId changed from ${baseOperation.operationId} to ${headOperation.operationId}`,
    );
  }

  const baseParameters = collectParameters(baseDocument, basePath, baseOperation);
  const headParameters = collectParameters(headDocument, headPath, headOperation);
  for (const [key, baseParameter] of baseParameters) {
    const headParameter = headParameters.get(key);
    if (!headParameter) {
      addChange(
        changes,
        `openapi:parameter-removed:${method}:${route}:${key}`,
        `${operationLocation}: parameter ${key} was removed`,
      );
      continue;
    }
    if (!baseParameter.required && headParameter.required) {
      addChange(
        changes,
        `openapi:parameter-required:${method}:${route}:${key}`,
        `${operationLocation}: parameter ${key} became required`,
      );
    }
    compareSchema({
      baseDocument,
      headDocument,
      baseSchema: baseParameter.schema,
      headSchema: headParameter.schema,
      direction: "input",
      location: `${operationLocation} parameter ${key}`,
      changes,
      seen: new Set(),
    });
  }
  for (const [key, headParameter] of headParameters) {
    if (!baseParameters.has(key) && headParameter.required) {
      addChange(
        changes,
        `openapi:required-parameter-added:${method}:${route}:${key}`,
        `${operationLocation}: new required parameter ${key} was added`,
      );
    }
  }

  const baseRequestBody = resolveObject(baseDocument, baseOperation.requestBody);
  const headRequestBody = resolveObject(headDocument, headOperation.requestBody);
  if (baseRequestBody && !headRequestBody) {
    addChange(
      changes,
      `openapi:request-body-removed:${method}:${route}`,
      `${operationLocation}: request body contract was removed`,
    );
  } else if (!baseRequestBody && headRequestBody?.required) {
    addChange(
      changes,
      `openapi:required-request-body-added:${method}:${route}`,
      `${operationLocation}: a required request body was added`,
    );
  } else if (baseRequestBody && headRequestBody) {
    if (!baseRequestBody.required && headRequestBody.required) {
      addChange(
        changes,
        `openapi:request-body-required:${method}:${route}`,
        `${operationLocation}: request body became required`,
      );
    }
    compareContent({
      baseDocument,
      headDocument,
      baseContent: baseRequestBody.content,
      headContent: headRequestBody.content,
      direction: "input",
      location: `${operationLocation} request`,
      changes,
    });
  }

  const baseResponses = baseOperation.responses || {};
  const headResponses = headOperation.responses || {};
  for (const [status, rawBaseResponse] of Object.entries(baseResponses)) {
    const rawHeadResponse = headResponses[status];
    if (!rawHeadResponse) {
      addChange(
        changes,
        `openapi:response-removed:${method}:${route}:${status}`,
        `${operationLocation}: response ${status} was removed`,
      );
      continue;
    }
    const baseResponse = resolveObject(baseDocument, rawBaseResponse);
    const headResponse = resolveObject(headDocument, rawHeadResponse);
    compareContent({
      baseDocument,
      headDocument,
      baseContent: baseResponse?.content,
      headContent: headResponse?.content,
      direction: "output",
      location: `${operationLocation} response ${status}`,
      changes,
    });
  }

  const baseSecurity = baseOperation.security ?? baseDocument.security ?? [];
  const headSecurity = headOperation.security ?? headDocument.security ?? [];
  if (stableJson(baseSecurity) !== stableJson(headSecurity) && headSecurity.length > 0) {
    addChange(
      changes,
      `openapi:security:${method}:${route}`,
      `${operationLocation}: security requirements became stricter or changed`,
    );
  }
}

function compareOpenApi(baseDocument, headDocument) {
  const changes = new Map();
  for (const [route, basePath] of Object.entries(baseDocument.paths || {})) {
    const headPath = headDocument.paths?.[route];
    if (!headPath) {
      addChange(changes, `openapi:path-removed:${route}`, `OpenAPI path ${route} was removed`);
      continue;
    }
    for (const method of HTTP_METHODS) {
      if (basePath[method]) {
        compareOperation(baseDocument, headDocument, route, method, basePath, headPath, changes);
      }
    }
  }
  return [...changes.values()];
}

function tokenizeGraphql(source) {
  const tokens = [];
  let index = 0;
  while (index < source.length) {
    const char = source[index];
    if (/\s|,/.test(char)) {
      index += 1;
      continue;
    }
    if (char === "#") {
      while (index < source.length && source[index] !== "\n") index += 1;
      continue;
    }
    if (source.startsWith('"""', index)) {
      const end = source.indexOf('"""', index + 3);
      if (end < 0) throw new Error("unterminated GraphQL block string");
      tokens.push("__DESCRIPTION__");
      index = end + 3;
      continue;
    }
    if (char === '"') {
      index += 1;
      while (index < source.length) {
        if (source[index] === "\\") index += 2;
        else if (source[index] === '"') {
          index += 1;
          break;
        } else index += 1;
      }
      tokens.push("__STRING__");
      continue;
    }
    const name = source.slice(index).match(/^[_A-Za-z][_0-9A-Za-z]*/)?.[0];
    if (name) {
      tokens.push(name);
      index += name.length;
      continue;
    }
    if (source.startsWith("...", index)) {
      tokens.push("...");
      index += 3;
      continue;
    }
    if ("!$():=@[]{|&}".includes(char)) {
      tokens.push(char);
      index += 1;
      continue;
    }
    if (char === "." || char === "-" || /[0-9]/.test(char)) {
      const value = source.slice(index).match(/^-?(?:\d+(?:\.\d+)?|\.\d+)/)?.[0];
      if (value) {
        tokens.push(value);
        index += value.length;
        continue;
      }
    }
    throw new Error(`unsupported GraphQL token near ${JSON.stringify(source.slice(index, index + 20))}`);
  }
  return tokens;
}

class GraphqlParser {
  constructor(tokens) {
    this.tokens = tokens;
    this.index = 0;
  }

  peek(offset = 0) {
    return this.tokens[this.index + offset];
  }

  take(expected) {
    const token = this.tokens[this.index];
    if (expected && token !== expected) {
      throw new Error(`expected ${expected}, found ${token}`);
    }
    this.index += 1;
    return token;
  }

  takeName() {
    const token = this.take();
    if (!/^[_A-Za-z][_0-9A-Za-z]*$/.test(token || "")) {
      throw new Error(`expected GraphQL name, found ${token}`);
    }
    return token;
  }

  skipDescription() {
    if (["__DESCRIPTION__", "__STRING__"].includes(this.peek())) this.take();
  }

  skipBalanced(open, close) {
    this.take(open);
    let depth = 1;
    while (depth > 0) {
      const token = this.take();
      if (token === open) depth += 1;
      if (token === close) depth -= 1;
      if (!token) throw new Error(`unterminated ${open}${close} block`);
    }
  }

  skipDirectives() {
    while (this.peek() === "@") {
      this.take("@");
      this.takeName();
      if (this.peek() === "(") this.skipBalanced("(", ")");
    }
  }

  parseTypeReference() {
    let result;
    if (this.peek() === "[") {
      this.take("[");
      result = `[${this.parseTypeReference()}]`;
      this.take("]");
    } else {
      result = this.takeName();
    }
    if (this.peek() === "!") {
      this.take("!");
      result += "!";
    }
    return result;
  }

  skipDefaultValue(endToken) {
    let round = 0;
    let square = 0;
    let curly = 0;
    while (this.index < this.tokens.length) {
      const token = this.peek();
      if (round === 0 && square === 0 && curly === 0) {
        if (token === endToken || token === "@") return;
        if (/^[_A-Za-z][_0-9A-Za-z]*$/.test(token || "") && this.peek(1) === ":") return;
      }
      this.take();
      if (token === "(") round += 1;
      if (token === ")") round -= 1;
      if (token === "[") square += 1;
      if (token === "]") square -= 1;
      if (token === "{") curly += 1;
      if (token === "}") curly -= 1;
    }
  }

  parseInputValue(endToken) {
    this.skipDescription();
    const name = this.takeName();
    this.take(":");
    const type = this.parseTypeReference();
    let hasDefault = false;
    if (this.peek() === "=") {
      this.take("=");
      hasDefault = true;
      this.skipDefaultValue(endToken);
    }
    this.skipDirectives();
    return { name, type, hasDefault };
  }

  parseArguments() {
    const argumentsByName = {};
    this.take("(");
    while (this.peek() !== ")") {
      const argument = this.parseInputValue(")");
      argumentsByName[argument.name] = argument;
    }
    this.take(")");
    return argumentsByName;
  }

  parseFields(kind) {
    const fields = {};
    this.take("{");
    while (this.peek() !== "}") {
      this.skipDescription();
      const name = this.takeName();
      let args = {};
      if (kind !== "input" && this.peek() === "(") args = this.parseArguments();
      this.take(":");
      const type = this.parseTypeReference();
      let hasDefault = false;
      if (kind === "input" && this.peek() === "=") {
        this.take("=");
        hasDefault = true;
        this.skipDefaultValue("}");
      }
      this.skipDirectives();
      fields[name] = { type, args, hasDefault };
    }
    this.take("}");
    return fields;
  }

  skipUntilDefinition() {
    const keywords = new Set(["schema", "scalar", "type", "interface", "input", "enum", "union", "directive", "extend"]);
    while (this.index < this.tokens.length && !keywords.has(this.peek())) this.index += 1;
  }

  parseDocument() {
    const definitions = {};
    while (this.index < this.tokens.length) {
      this.skipDescription();
      if (this.peek() === "extend") this.take("extend");
      const kind = this.take();
      if (kind === "schema") {
        if (this.peek() === "{") this.skipBalanced("{", "}");
        else this.skipUntilDefinition();
        continue;
      }
      if (kind === "directive") {
        this.skipUntilDefinition();
        continue;
      }
      if (kind === "scalar") {
        const name = this.takeName();
        this.skipDirectives();
        definitions[name] = { kind, fields: {} };
        continue;
      }
      if (!["type", "interface", "input", "enum", "union"].includes(kind)) {
        this.skipUntilDefinition();
        continue;
      }

      const name = this.takeName();
      if (kind === "union") {
        this.skipDirectives();
        this.take("=");
        const members = [];
        while (this.index < this.tokens.length) {
          if (this.peek() === "|") this.take("|");
          if (!/^[_A-Za-z][_0-9A-Za-z]*$/.test(this.peek() || "")) break;
          members.push(this.takeName());
          if (this.peek() !== "|") break;
        }
        definitions[name] = { kind, members };
        continue;
      }
      if (kind === "enum") {
        this.skipDirectives();
        this.take("{");
        const values = [];
        while (this.peek() !== "}") {
          this.skipDescription();
          values.push(this.takeName());
          this.skipDirectives();
        }
        this.take("}");
        definitions[name] = { kind, values };
        continue;
      }

      while (this.peek() !== "{" && this.index < this.tokens.length) {
        if (this.peek() === "@") this.skipDirectives();
        else this.take();
      }
      const fields = this.parseFields(kind);
      const previous = definitions[name];
      definitions[name] = {
        kind,
        fields: { ...(previous?.fields || {}), ...fields },
      };
    }
    return definitions;
  }
}

function parseGraphql(source) {
  return new GraphqlParser(tokenizeGraphql(source)).parseDocument();
}

function isRequiredInput(field) {
  return field.type.endsWith("!") && !field.hasDefault;
}

function compareGraphql(baseSource, headSource) {
  const base = parseGraphql(baseSource);
  const head = parseGraphql(headSource);
  const changes = new Map();

  for (const [typeName, baseType] of Object.entries(base)) {
    const headType = head[typeName];
    if (!headType) {
      addChange(changes, `graphql:type-removed:${typeName}`, `GraphQL type ${typeName} was removed`);
      continue;
    }
    if (baseType.kind !== headType.kind) {
      addChange(
        changes,
        `graphql:type-kind:${typeName}`,
        `GraphQL type ${typeName} changed kind from ${baseType.kind} to ${headType.kind}`,
      );
      continue;
    }

    if (["type", "interface"].includes(baseType.kind)) {
      for (const [fieldName, baseField] of Object.entries(baseType.fields)) {
        const headField = headType.fields[fieldName];
        if (!headField) {
          addChange(
            changes,
            `graphql:field-removed:${typeName}.${fieldName}`,
            `GraphQL field ${typeName}.${fieldName} was removed`,
          );
          continue;
        }
        if (baseField.type !== headField.type) {
          addChange(
            changes,
            `graphql:field-type:${typeName}.${fieldName}`,
            `GraphQL field ${typeName}.${fieldName} changed type from ${baseField.type} to ${headField.type}`,
          );
        }
        for (const [argumentName, baseArgument] of Object.entries(baseField.args)) {
          const headArgument = headField.args[argumentName];
          if (!headArgument) {
            addChange(
              changes,
              `graphql:argument-removed:${typeName}.${fieldName}:${argumentName}`,
              `GraphQL argument ${typeName}.${fieldName}(${argumentName}:) was removed`,
            );
            continue;
          }
          if (baseArgument.type !== headArgument.type) {
            addChange(
              changes,
              `graphql:argument-type:${typeName}.${fieldName}:${argumentName}`,
              `GraphQL argument ${typeName}.${fieldName}(${argumentName}:) changed type from ${baseArgument.type} to ${headArgument.type}`,
            );
          }
          if (!isRequiredInput(baseArgument) && isRequiredInput(headArgument)) {
            addChange(
              changes,
              `graphql:argument-required:${typeName}.${fieldName}:${argumentName}`,
              `GraphQL argument ${typeName}.${fieldName}(${argumentName}:) became required`,
            );
          }
        }
        for (const [argumentName, headArgument] of Object.entries(headField.args)) {
          if (!(argumentName in baseField.args) && isRequiredInput(headArgument)) {
            addChange(
              changes,
              `graphql:required-argument-added:${typeName}.${fieldName}:${argumentName}`,
              `GraphQL field ${typeName}.${fieldName} added required argument ${argumentName}`,
            );
          }
        }
      }
    }

    if (baseType.kind === "input") {
      for (const [fieldName, baseField] of Object.entries(baseType.fields)) {
        const headField = headType.fields[fieldName];
        if (!headField) {
          addChange(
            changes,
            `graphql:input-field-removed:${typeName}.${fieldName}`,
            `GraphQL input field ${typeName}.${fieldName} was removed`,
          );
          continue;
        }
        if (baseField.type !== headField.type) {
          addChange(
            changes,
            `graphql:input-field-type:${typeName}.${fieldName}`,
            `GraphQL input field ${typeName}.${fieldName} changed type from ${baseField.type} to ${headField.type}`,
          );
        }
        if (!isRequiredInput(baseField) && isRequiredInput(headField)) {
          addChange(
            changes,
            `graphql:input-field-required:${typeName}.${fieldName}`,
            `GraphQL input field ${typeName}.${fieldName} became required`,
          );
        }
      }
      for (const [fieldName, headField] of Object.entries(headType.fields)) {
        if (!(fieldName in baseType.fields) && isRequiredInput(headField)) {
          addChange(
            changes,
            `graphql:required-input-field-added:${typeName}.${fieldName}`,
            `GraphQL input ${typeName} added required field ${fieldName}`,
          );
        }
      }
    }

    if (baseType.kind === "enum") {
      const headValues = new Set(headType.values);
      for (const value of baseType.values) {
        if (!headValues.has(value)) {
          addChange(
            changes,
            `graphql:enum-value-removed:${typeName}.${value}`,
            `GraphQL enum value ${typeName}.${value} was removed`,
          );
        }
      }
    }

    if (baseType.kind === "union") {
      const headMembers = new Set(headType.members);
      for (const member of baseType.members) {
        if (!headMembers.has(member)) {
          addChange(
            changes,
            `graphql:union-member-removed:${typeName}.${member}`,
            `GraphQL union member ${typeName}.${member} was removed`,
          );
        }
      }
    }
  }

  return [...changes.values()];
}

function validateExceptionRegister(register, file) {
  const failures = [];
  if (register.schema_version !== 1) failures.push(`${file}: schema_version must be 1`);
  if (!Array.isArray(register.exceptions)) failures.push(`${file}: exceptions must be an array`);
  for (const [index, entry] of (register.exceptions || []).entries()) {
    for (const field of ["id", "owner", "reason", "expires_on"]) {
      if (typeof entry[field] !== "string" || entry[field].trim() === "") {
        failures.push(`${file}: exceptions[${index}].${field} must be non-empty`);
      }
    }
    const expiry = Date.parse(`${entry.expires_on}T23:59:59Z`);
    if (!Number.isFinite(expiry)) failures.push(`${file}: ${entry.id} has invalid expires_on`);
    else if (Date.now() > expiry) failures.push(`${file}: ${entry.id} expired on ${entry.expires_on}`);
  }
  if (failures.length) throw new Error(failures.join("\n"));
}

function enforceExceptions(changes, register) {
  const approved = new Map(register.exceptions.map((entry) => [entry.id, entry]));
  const observed = new Set(changes.map((change) => change.id));
  const stale = [...approved.keys()].filter((id) => !observed.has(id));
  if (stale.length) {
    throw new Error(`stale API compatibility exception(s):\n${stale.map((id) => `- ${id}`).join("\n")}`);
  }
  return {
    approved: changes.filter((change) => approved.has(change.id)),
    rejected: changes.filter((change) => !approved.has(change.id)),
  };
}

function runSelfTest() {
  const baseOpenApi = {
    openapi: "3.1.0",
    paths: {
      "/items": {
        get: {
          parameters: [{ in: "query", name: "limit", required: false, schema: { type: "integer" } }],
          responses: { "200": { content: { "application/json": { schema: { type: "array", items: { type: "string" } } } } } },
        },
      },
    },
  };
  const removedPath = { openapi: "3.1.0", paths: {} };
  assert(compareOpenApi(baseOpenApi, removedPath).some((change) => change.id === "openapi:path-removed:/items"));

  const requiredParameter = structuredClone(baseOpenApi);
  requiredParameter.paths["/items"].get.parameters[0].required = true;
  assert(
    compareOpenApi(baseOpenApi, requiredParameter).some(
      (change) => change.id === "openapi:parameter-required:get:/items:query:limit",
    ),
  );

  const baseGraphql = `
    type Query { item(id: ID): Item }
    type Item { id: ID!, name: String }
    input ItemInput { name: String }
    enum Status { ACTIVE ARCHIVED }
  `;
  const additiveGraphql = `
    type Query { item(id: ID): Item, items: [Item!]! }
    type Item { id: ID!, name: String, description: String }
    input ItemInput { name: String, description: String }
    enum Status { ACTIVE ARCHIVED DRAFT }
  `;
  assert.equal(compareGraphql(baseGraphql, additiveGraphql).length, 0);

  const breakingGraphql = `
    type Query { item(id: ID!): Item }
    type Item { id: ID! }
    input ItemInput { name: String, slug: String! }
    enum Status { ACTIVE }
  `;
  const graphqlChanges = compareGraphql(baseGraphql, breakingGraphql);
  assert(graphqlChanges.some((change) => change.id === "graphql:argument-required:Query.item:id"));
  assert(graphqlChanges.some((change) => change.id === "graphql:field-removed:Item.name"));
  assert(graphqlChanges.some((change) => change.id === "graphql:required-input-field-added:ItemInput.slug"));
  assert(graphqlChanges.some((change) => change.id === "graphql:enum-value-removed:Status.ARCHIVED"));

  console.log("✔ API compatibility comparator self-test passed");
}

function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.selfTest) {
    runSelfTest();
    return;
  }
  if (!options.baseDir || !options.headDir || !options.exceptions) {
    throw new Error("usage: verify-api-compatibility.mjs --base-dir DIR --head-dir DIR --exceptions FILE");
  }

  const baseOpenApi = readJson(path.join(options.baseDir, "openapi.json"));
  const headOpenApi = readJson(path.join(options.headDir, "openapi.json"));
  const baseGraphql = readText(path.join(options.baseDir, "schema.graphql"));
  const headGraphql = readText(path.join(options.headDir, "schema.graphql"));
  const exceptionFile = path.resolve(options.exceptions);
  const register = readJson(exceptionFile);
  validateExceptionRegister(register, exceptionFile);

  const changes = [
    ...compareOpenApi(baseOpenApi, headOpenApi),
    ...compareGraphql(baseGraphql, headGraphql),
  ].sort((left, right) => left.id.localeCompare(right.id));
  const { approved, rejected } = enforceExceptions(changes, register);

  for (const change of approved) console.warn(`⚠ approved breaking change ${change.id}: ${change.message}`);
  if (rejected.length) {
    console.error("API compatibility verification failed:");
    for (const change of rejected) console.error(`✗ ${change.id}: ${change.message}`);
    process.exit(Math.min(rejected.length, 255));
  }

  console.log(
    `✔ API contracts are backward compatible (${approved.length} approved exception(s), ${changes.length} breaking change(s) observed)`,
  );
}

try {
  main();
} catch (error) {
  console.error(`API compatibility verification failed: ${error.message}`);
  process.exit(1);
}
