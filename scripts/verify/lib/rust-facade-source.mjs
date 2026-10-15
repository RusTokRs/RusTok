// Composes a Rust module facade with the files it declares.
//
// `apps/server/src/services/comments_provider_runtime.rs` used to `include!("<file>.rs")` its
// modules, so a verifier that read that one file saw the code of every part it composes. The
// facade now declares them as `#[path = "<file>.rs"] mod <name>;`, which is the same
// composition expressed with real modules, but a plain file read only sees the declarations.
// This helper restores the view the verifiers were written against: it reads the entry file
// and, recursively, every file the entry declares with a `#[path = "..."]` attribute or an
// `include!("...")` macro, resolving each target relative to the file that declares it.
//
// A `read` callback that throws for an unknown path (the verifiers pass one built on
// `fs.readFileSync`) keeps the composition fail-loud: a declaration pointing at a file that no
// longer exists must break the guard instead of silently shrinking the composed view.

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

export const readComposedRustSource = (read, entryPath) => {
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
  return parts.join('\n');
};
