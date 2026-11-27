/** Reject a missing or negative schema mutation acknowledgement from the Product owner. */
export function requireSchemaWriteConfirmation(
  value: boolean | null | undefined
): true {
  if (value !== true) {
    throw new Error('Product schema write was not confirmed by the owner');
  }
  return true;
}
