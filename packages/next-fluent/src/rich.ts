import React from 'react';
import type { RichTranslationValues } from './types';

interface ASTNode {
  tag?: string;
  children: (React.ReactNode | string)[];
}

/**
 * Parses a formatted string containing markup tags like `<link>text</link>` or `<br/>`
 * and maps them to React elements or tag render functions provided in `values`.
 */
export function parseRichText(
  text: string,
  values?: RichTranslationValues
): React.ReactNode {
  if (!values || !text.includes('<')) {
    return text;
  }

  // Check if any tag key in values corresponds to a function or React element
  const hasInteractiveTags = Object.keys(values).some(
    (k) => typeof values[k] === 'function' || React.isValidElement(values[k])
  );
  if (!hasInteractiveTags) {
    return text;
  }

  // Regex matches:
  // - Open tag: <tag>
  // - Close tag: </tag>
  // - Self-closing tag: <tag/>
  const tagRegex = /<\/?([a-zA-Z][a-zA-Z0-9_-]*)\s*\/?>/g;

  const root: ASTNode = { children: [] };
  const stack: ASTNode[] = [root];
  let lastIndex = 0;
  let match: RegExpExecArray | null;

  while ((match = tagRegex.exec(text)) !== null) {
    const [fullMatch, tagName] = match;
    const matchIndex = match.index;

    // Push text before this tag
    if (matchIndex > lastIndex) {
      const textChunk = text.slice(lastIndex, matchIndex);
      stack[stack.length - 1].children.push(textChunk);
    }
    lastIndex = tagRegex.lastIndex;

    const isClose = fullMatch.startsWith('</');
    const isSelfClosing = fullMatch.endsWith('/>');

    if (isSelfClosing) {
      const renderFnOrEl = values[tagName];
      if (typeof renderFnOrEl === 'function') {
        stack[stack.length - 1].children.push(renderFnOrEl(null));
      } else if (React.isValidElement(renderFnOrEl)) {
        stack[stack.length - 1].children.push(renderFnOrEl);
      } else {
        stack[stack.length - 1].children.push(fullMatch);
      }
    } else if (isClose) {
      // Check if this closes the currently open tag
      if (stack.length > 1 && stack[stack.length - 1].tag === tagName) {
        const finishedNode = stack.pop()!;
        const renderFnOrEl = values[tagName];
        const innerChildren =
          finishedNode.children.length === 1
            ? finishedNode.children[0]
            : React.createElement(React.Fragment, null, ...finishedNode.children);

        if (typeof renderFnOrEl === 'function') {
          stack[stack.length - 1].children.push(renderFnOrEl(innerChildren));
        } else if (React.isValidElement(renderFnOrEl)) {
          stack[stack.length - 1].children.push(
            React.cloneElement(renderFnOrEl, undefined, innerChildren)
          );
        } else {
          stack[stack.length - 1].children.push(
            `<${tagName}>`,
            innerChildren,
            `</${tagName}>`
          );
        }
      } else {
        // Unmatched close tag, retain as text
        stack[stack.length - 1].children.push(fullMatch);
      }
    } else {
      // Open tag
      if (tagName in values) {
        stack.push({ tag: tagName, children: [] });
      } else {
        stack[stack.length - 1].children.push(fullMatch);
      }
    }
  }

  // Push trailing text
  if (lastIndex < text.length) {
    stack[stack.length - 1].children.push(text.slice(lastIndex));
  }

  // Unclosed tags on stack
  while (stack.length > 1) {
    const unclosed = stack.pop()!;
    stack[stack.length - 1].children.push(
      `<${unclosed.tag}>`,
      ...unclosed.children
    );
  }

  if (root.children.length === 0) return '';
  if (root.children.length === 1) return root.children[0];
  return React.createElement(React.Fragment, null, ...root.children);
}
