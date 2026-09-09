import { readFileSync } from 'node:fs';

const providerRegistrySchemaUrl = new URL(
  '../../../crates/libs/rustok-fba/contracts/provider-registry-v1.schema.json',
  import.meta.url,
);

export const FBA_PROVIDER_REGISTRY_SCHEMA_PATH =
  'crates/libs/rustok-fba/contracts/provider-registry-v1.schema.json';

export class FbaRegistryValidationError extends Error {
  constructor(message) {
    super(message);
    this.name = 'FbaRegistryValidationError';
  }
}

const readProviderRegistrySchema = () =>
  JSON.parse(readFileSync(providerRegistrySchemaUrl, 'utf8'));

const display = (value) => JSON.stringify(value);

function resolveReference(schema, reference) {
  if (!reference.startsWith('#/')) {
    throw new FbaRegistryValidationError(`unsupported FBA schema reference ${reference}`);
  }

  return reference
    .slice(2)
    .split('/')
    .reduce((current, segment) => current?.[segment], schema);
}

function matchesType(value, type) {
  if (type === 'array') return Array.isArray(value);
  if (type === 'object') return value !== null && typeof value === 'object' && !Array.isArray(value);
  if (type === 'integer') return Number.isInteger(value);
  return typeof value === type;
}

function validateSchema(value, rule, schema, location, violations) {
  if (rule.$ref) {
    const referencedRule = resolveReference(schema, rule.$ref);
    if (!referencedRule) {
      violations.push(`${location}: unresolved schema reference ${rule.$ref}`);
      return;
    }
    validateSchema(value, referencedRule, schema, location, violations);
    return;
  }

  if (Object.hasOwn(rule, 'const') && value !== rule.const) {
    violations.push(`${location}: expected ${display(rule.const)}, received ${display(value)}`);
    return;
  }

  if (rule.enum && !rule.enum.includes(value)) {
    violations.push(`${location}: expected one of ${display(rule.enum)}, received ${display(value)}`);
    return;
  }

  if (rule.type && !matchesType(value, rule.type)) {
    violations.push(`${location}: expected ${rule.type}, received ${Array.isArray(value) ? 'array' : typeof value}`);
    return;
  }

  if (rule.type === 'string' && rule.minLength && value.length < rule.minLength) {
    violations.push(`${location}: must not be empty`);
  }

  if (rule.type === 'array') {
    if (rule.minItems && value.length < rule.minItems) {
      violations.push(`${location}: requires at least ${rule.minItems} item(s)`);
    }
    if (rule.uniqueItems) {
      const values = new Set(value.map((item) => JSON.stringify(item)));
      if (values.size !== value.length) violations.push(`${location}: values must be unique`);
    }
    if (rule.items) {
      value.forEach((item, index) => validateSchema(item, rule.items, schema, `${location}[${index}]`, violations));
    }
  }

  if (rule.type === 'object') {
    for (const property of rule.required ?? []) {
      if (!Object.hasOwn(value, property)) {
        violations.push(`${location}: missing required property ${property}`);
      }
    }
    for (const [property, propertyRule] of Object.entries(rule.properties ?? {})) {
      if (Object.hasOwn(value, property)) {
        validateSchema(value[property], propertyRule, schema, `${location}.${property}`, violations);
      }
    }
  }
}

export function validateProviderRegistryBaseline({
  registry,
  expectedModule,
  expectedContractVersion,
  allowedStatuses,
}) {
  const schema = readProviderRegistrySchema();
  const violations = [];
  validateSchema(registry, schema, schema, '$', violations);

  if (registry?.module !== expectedModule) {
    violations.push(`$.module: expected ${expectedModule}, received ${display(registry?.module)}`);
  }
  if (registry?.contract_version !== expectedContractVersion) {
    violations.push(
      `$.contract_version: expected ${expectedContractVersion}, received ${display(registry?.contract_version)}`,
    );
  }
  if (!allowedStatuses.includes(registry?.status)) {
    violations.push(`$.status: ${display(registry?.status)} is not allowed for ${expectedModule}`);
  }

  const declaredOperations = new Set(registry?.ports?.flatMap((port) => port.operations ?? []) ?? []);
  for (const testCase of registry?.contract_tests?.cases ?? []) {
    if (!declaredOperations.has(testCase.operation)) {
      violations.push(
        `$.contract_tests.cases: ${expectedModule}.${testCase.operation} is not declared by a provider port`,
      );
    }
  }

  if (violations.length > 0) {
    throw new FbaRegistryValidationError(
      `${expectedModule} provider registry baseline failed:\n${violations.map((violation) => `- ${violation}`).join('\n')}`,
    );
  }
}
