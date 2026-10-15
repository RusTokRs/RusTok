// Composes a Rust module facade with the files it textually includes.
//
// `apps/server/src/services/comments_provider_runtime.rs` used to `include!("<file>.rs")` its
// modules, so a verifier that read that one file saw the code of every part it composes. The
// facade now declares them as `#[path = "<file>.rs"] mod <name>;`, which is the same
// composition expressed with real modules, but a plain file read only sees the declarations.
// This helper restores the view the verifiers were written against: it reads the entry file
// and, recursively, every file the entry declares with a `#[path = "..."]` attribute or an
// `include!("...")` macro, resolving each target relative to the file that declares it.

const DIRECTIVE = /#\[path = "([^"]+)"\]|include!\("([^"]+)"\)/g;

const normalize = (path) => {
  const parts = [];
  for (const segment of path.split('/')) {
    if (segment === '' || segment === '.') continue;
    if (segment === '..') {
      parts.pop();
      continue;
    }
    parts.push(segment);
  }
  return parts.join('/');
};

const dirname = (file) => {
  const index = file.lastIndexOf('/');
  return index < 0 ? '' : file.slice(0, index);
};

const resolveRelative = (fromFile, target) => {
  const base = dirname(fromFile);
  return normalize(base === '' ? target : `${base}/${target}`);
};

// `extraPaths` are owners a verifier asserts through the same composed view even though the
// facade no longer reaches them (for example a file that a sibling module now holds). They are
// appended after the transitive closure so the entry file keeps its place in the composition.
export const readComposedRustSource = (read, entryPath, ...extraPaths) => {
  const seen = new Set();
  const parts = [];
  const visit = (file) => {
    if (file === '' || seen.has(file)) return;
    seen.add(file);
    const source = read(file);
    parts.push(source);
    for (const match of source.matchAll(DIRECTIVE)) {
      visit(resolveRelative(file, match[1] ?? match[2]));
    }
  };
  visit(entryPath);
  for (const extra of extraPaths) {
    if (seen.has(extra)) continue;
    seen.add(extra);
    parts.push(read(extra));
  }
  return parts.join('\n');
};
