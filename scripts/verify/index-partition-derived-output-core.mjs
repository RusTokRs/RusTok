import { randomUUID } from 'node:crypto';
import {
  closeSync,
  fstatSync,
  fsyncSync,
  linkSync,
  lstatSync,
  openSync,
  realpathSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import path from 'node:path';

const identityOf = (stat) => `${stat.dev}:${stat.ino}`;

const requireNonEmptyString = (value, label) => {
  if (typeof value !== 'string' || value.length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
};

const ensureOutsideRoot = (root, filename, label) => {
  const relative = path.relative(root, filename);
  const inside = relative === ''
    || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
  if (inside) throw new Error(`${label} must stay outside the retained bundle root`);
};

const removePublishedIdentity = (filename, expectedIdentity) => {
  try {
    const current = lstatSync(filename, { bigint: true });
    if (!current.isSymbolicLink() && current.isFile() && identityOf(current) === expectedIdentity) {
      unlinkSync(filename);
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
};

const removeTemporaryFile = (filename) => {
  try {
    unlinkSync(filename);
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
};

export const renderDerivedJson = (value) => {
  const rendered = JSON.stringify(value, null, 2);
  if (typeof rendered !== 'string') throw new Error('derived output must be JSON serializable');
  return `${rendered}\n`;
};

export const publishDerivedJsonOutsideRetainedBundle = ({
  root,
  outputPath,
  value,
  label = 'derived output',
}) => {
  const resolvedRoot = path.resolve(requireNonEmptyString(root, 'root'));
  const rootStat = lstatSync(resolvedRoot, { bigint: true });
  if (rootStat.isSymbolicLink() || !rootStat.isDirectory()) {
    throw new Error('retained bundle root must be a regular non-symlink directory');
  }
  const canonicalRoot = realpathSync.native(resolvedRoot);

  const resolvedOutputPath = path.resolve(requireNonEmptyString(outputPath, 'outputPath'));
  ensureOutsideRoot(resolvedRoot, resolvedOutputPath, label);

  const parentPath = path.dirname(resolvedOutputPath);
  const parentStat = lstatSync(parentPath, { bigint: true });
  if (parentStat.isSymbolicLink() || !parentStat.isDirectory()) {
    throw new Error(`${label} parent must be a regular non-symlink directory`);
  }
  const canonicalParent = realpathSync.native(parentPath);
  ensureOutsideRoot(canonicalRoot, canonicalParent, `${label} parent`);

  const bytes = Buffer.from(renderDerivedJson(value), 'utf8');
  const temporaryPath = path.join(
    parentPath,
    `.${path.basename(resolvedOutputPath)}.${process.pid}.${randomUUID()}.tmp`,
  );

  let descriptor = null;
  let temporaryIdentity = null;
  let published = false;
  try {
    descriptor = openSync(temporaryPath, 'wx', 0o600);
    writeFileSync(descriptor, bytes);
    fsyncSync(descriptor);
    const temporaryStat = fstatSync(descriptor, { bigint: true });
    if (!temporaryStat.isFile()) throw new Error(`${label} temporary output must be a regular file`);
    temporaryIdentity = identityOf(temporaryStat);
    closeSync(descriptor);
    descriptor = null;

    try {
      linkSync(temporaryPath, resolvedOutputPath);
    } catch (error) {
      if (error.code === 'EEXIST') {
        throw new Error(`${label} already exists; refusing to overwrite`);
      }
      throw error;
    }
    published = true;

    const outputStat = lstatSync(resolvedOutputPath, { bigint: true });
    if (outputStat.isSymbolicLink()
        || !outputStat.isFile()
        || identityOf(outputStat) !== temporaryIdentity) {
      throw new Error(`${label} changed while it was being published`);
    }
    const canonicalOutput = realpathSync.native(resolvedOutputPath);
    ensureOutsideRoot(canonicalRoot, canonicalOutput, label);

    const currentParentStat = lstatSync(parentPath, { bigint: true });
    if (currentParentStat.isSymbolicLink()
        || !currentParentStat.isDirectory()
        || identityOf(currentParentStat) !== identityOf(parentStat)
        || realpathSync.native(parentPath) !== canonicalParent) {
      throw new Error(`${label} parent changed while it was being published`);
    }

    removeTemporaryFile(temporaryPath);
    return {
      path: resolvedOutputPath,
      bytes: bytes.length,
    };
  } catch (error) {
    if (descriptor !== null) closeSync(descriptor);
    if (published && temporaryIdentity !== null) {
      removePublishedIdentity(resolvedOutputPath, temporaryIdentity);
    }
    removeTemporaryFile(temporaryPath);
    throw error;
  }
};
