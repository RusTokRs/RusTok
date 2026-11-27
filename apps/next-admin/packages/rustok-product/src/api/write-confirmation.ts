/** Require an explicit positive acknowledgement from a domain owner. */
export function requireOwnerWriteConfirmation(
  value: boolean | null | undefined,
  operation: string
): true {
  if (value !== true) {
    throw new Error(`${operation} was not confirmed by the owner`);
  }
  return true;
}

/** Require a non-null owner result for a mutation that creates or updates a resource. */
export function requireOwnerResult<T>(
  value: T | null | undefined,
  operation: string
): T {
  if (value === null || value === undefined) {
    throw new Error(`${operation} did not return a result from the owner`);
  }
  return value;
}

export function requireSchemaWriteConfirmation(
  value: boolean | null | undefined
): true {
  return requireOwnerWriteConfirmation(value, 'Product schema write');
}
