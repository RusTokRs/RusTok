import fs from 'node:fs';
import path from 'node:path';

function walk(dir, fileList = []) {
  const files = fs.readdirSync(dir);
  for (const file of files) {
    const filePath = path.join(dir, file);
    if (fs.statSync(filePath).isDirectory()) {
      if (file !== 'target' && file !== 'node_modules' && file !== '.git') {
        walk(filePath, fileList);
      }
    } else if (file.endsWith('.rs')) {
      fileList.push(filePath);
    }
  }
  return fileList;
}

function skipRustLiteral(source, index) {
  if (source.startsWith('//', index)) {
    const newline = source.indexOf('\n', index + 2);
    return newline === -1 ? source.length : newline + 1;
  }
  if (source.startsWith('/*', index)) {
    const end = source.indexOf('*/', index + 2);
    return end === -1 ? source.length : end + 2;
  }
  if (source[index] === 'r' || source[index] === 'b') {
    let quoteIndex = source[index] === 'b' && source[index + 1] === 'r' ? index + 2 : index + 1;
    let hashes = '';
    while (source[quoteIndex] === '#') {
      hashes += '#';
      quoteIndex++;
    }
    if (source[quoteIndex] === '"') {
      const end = source.indexOf(`"${hashes}`, quoteIndex + 1);
      return end === -1 ? source.length : end + hashes.length + 1;
    }
  }
  const quote = source[index];
  if (quote !== '"' && quote !== "'") return index;
  let cursor = index + 1;
  while (cursor < source.length) {
    if (source[cursor] === '\\') {
      cursor += 2;
    } else if (source[cursor] === quote) {
      return cursor + 1;
    } else {
      cursor++;
    }
  }
  return source.length;
}

function findRustToken(source, token, start) {
  let cursor = start;
  while (cursor < source.length) {
    const skipped = skipRustLiteral(source, cursor);
    if (skipped !== cursor) {
      cursor = skipped;
      continue;
    }
    if (
      source.startsWith(token, cursor) &&
      !/[A-Za-z0-9_]/.test(source[cursor - 1] ?? '') &&
      !/[A-Za-z0-9_]/.test(source[cursor + token.length] ?? '')
    ) {
      return cursor;
    }
    cursor++;
  }
  return -1;
}

function addUnsupportedDbBackendArms(source) {
  let cursor = 0;
  let result = '';

  while (cursor < source.length) {
    const matchIndex = findRustToken(source, 'match', cursor);
    if (matchIndex === -1) {
      result += source.slice(cursor);
      break;
    }
    result += source.slice(cursor, matchIndex);
    let openBrace = matchIndex;
    while (openBrace < source.length && source[openBrace] !== '{') {
      const skipped = skipRustLiteral(source, openBrace);
      openBrace = skipped === openBrace ? openBrace + 1 : skipped;
    }
    if (openBrace >= source.length) {
      result += source.slice(matchIndex);
      break;
    }

    let depth = 1;
    let closeBrace = openBrace + 1;
    while (closeBrace < source.length && depth > 0) {
      const skipped = skipRustLiteral(source, closeBrace);
      if (skipped !== closeBrace) {
        closeBrace = skipped;
        continue;
      }
      if (source[closeBrace] === '{') depth++;
      if (source[closeBrace] === '}') depth--;
      closeBrace++;
    }
    if (depth !== 0) {
      result += source.slice(matchIndex);
      break;
    }

    const matchBody = source.slice(openBrace + 1, closeBrace - 1);
    if (
      !/(?:Db|Database)Backend::[A-Za-z0-9_]+(?:\s*\|\s*(?:Db|Database)Backend::[A-Za-z0-9_]+)*(?:\s+if\s+[^=]+)?\s*=>/.test(
        matchBody,
      ) ||
      /(?:^|[\n,])\s*(?:_|[a-z][A-Za-z0-9_]*)\s*=>|(?:^|[\n,])\s*\(\s*(?:_|[a-z][A-Za-z0-9_]*)\s*,/.test(
        matchBody,
      )
    ) {
      result += source.slice(matchIndex, closeBrace);
    } else {
      const firstBackend = matchBody.search(/(?:Db|Database)Backend::/);
      const lineStart = matchBody.lastIndexOf('\n', firstBackend) + 1;
      const indentation = matchBody.slice(lineStart, firstBackend).match(/^\s*/)[0];
      result += `${source.slice(matchIndex, closeBrace - 1)}${indentation}_ => unreachable!("unsupported SeaORM database backend"),\n${source.slice(closeBrace - 1, closeBrace)}`;
    }
    cursor = closeBrace;
  }

  return result;
}

const targetDirs = ['crates', 'apps', 'ops'];
let updatedFiles = 0;

for (const targetDir of targetDirs) {
  const fullDir = path.resolve(process.cwd(), targetDir);
  if (!fs.existsSync(fullDir)) continue;
  const rsFiles = walk(fullDir);

  for (const filePath of rsFiles) {
    let content = fs.readFileSync(filePath, 'utf8');
    let original = content;

    // Pattern 1: .query_one(Statement:: -> .query_one_raw(Statement::
    content = content.replace(/\.query_one\(\s*Statement::/g, '.query_one_raw(Statement::');
    content = content.replace(/\.query_one\(\s*&Statement::/g, '.query_one_raw(Statement::');
    content = content.replace(/\.query_one\(\s*statement\s*\)/g, '.query_one_raw(statement)');
    content = content.replace(/\.query_one\(\s*stmt\s*\)/g, '.query_one_raw(stmt)');

    // Pattern 2: .query_all(Statement:: -> .query_all_raw(Statement::
    content = content.replace(/\.query_all\(\s*Statement::/g, '.query_all_raw(Statement::');
    content = content.replace(/\.query_all\(\s*&Statement::/g, '.query_all_raw(Statement::');
    content = content.replace(/\.query_all\(\s*statement\s*\)/g, '.query_all_raw(statement)');
    content = content.replace(/\.query_all\(\s*stmt\s*\)/g, '.query_all_raw(stmt)');

    // Pattern 3: .execute(Statement:: -> .execute_raw(Statement::
    content = content.replace(/\.execute\(\s*Statement::/g, '.execute_raw(Statement::');
    content = content.replace(/\.execute\(\s*&Statement::/g, '.execute_raw(Statement::');
    content = content.replace(/\.execute\(\s*statement\s*\)/g, '.execute_raw(statement)');
    content = content.replace(/\.execute\(\s*stmt\s*\)/g, '.execute_raw(stmt)');

    // Pattern 4: Unboxed Value / SqlValue variants in SeaORM 2.0 / SeaQuery 1.0
    // Uuid, ChronoDateTimeUtc, ChronoDateTime, ChronoDateTimeWithTimeZone, String, Bytes
    const unboxedTypes = [
      'Uuid',
      'ChronoDateTimeUtc',
      'ChronoDateTime',
      'ChronoDateTimeWithTimeZone',
      'ChronoDate',
      'ChronoTime',
      'String',
      'Bytes',
    ];

    for (const type of unboxedTypes) {
      // SqlValue::Type(Some(Box::new(expr))) -> SqlValue::Type(Some(expr))
      const sqlValRegex = new RegExp(`SqlValue::${type}\\(Some\\(Box::new\\(([^\\)]+)\\)\\)\\)`, 'g');
      content = content.replace(sqlValRegex, `SqlValue::${type}(Some($1))`);

      // Value::Type(Some(Box::new(expr))) -> Value::Type(Some(expr))
      const valRegex = new RegExp(`Value::${type}\\(Some\\(Box::new\\(([^\\)]+)\\)\\)\\)`, 'g');
      content = content.replace(valRegex, `Value::${type}(Some($1))`);

      // SqlValue::Type(expr.map(Box::new)) -> SqlValue::Type(expr)
      const sqlValMapRegex = new RegExp(`SqlValue::${type}\\(([^\\)]+)\\.map\\(Box::new\\)\\)`, 'g');
      content = content.replace(sqlValMapRegex, `SqlValue::${type}($1)`);

      // Value::Type(expr.map(Box::new)) -> Value::Type(expr)
      const valMapRegex = new RegExp(`Value::${type}\\(([^\\)]+)\\.map\\(Box::new\\)\\)`, 'g');
      content = content.replace(valMapRegex, `Value::${type}($1)`);
    }

    // DbBackend became non-exhaustive in SeaORM 2. Every platform SQL branch
    // must fail closed rather than accidentally select a dialect for a future
    // backend.
    content = content.replace(
      /^\s*_ => unreachable!\("unsupported SeaORM database backend"\),\r?\n/gm,
      '',
    );
    content = addUnsupportedDbBackendArms(content);

    if (content !== original) {
      fs.writeFileSync(filePath, content, 'utf8');
      updatedFiles++;
      console.log(`Updated: ${path.relative(process.cwd(), filePath)}`);
    }
  }
}

console.log(`\nFinished updating ${updatedFiles} files.`);
