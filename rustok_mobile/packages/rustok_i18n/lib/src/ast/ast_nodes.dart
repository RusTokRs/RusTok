/// Base interface for all Fluent AST nodes.
sealed class FtlNode {
  const FtlNode();
}

/// A parsed Fluent Translation List (.ftl) resource.
class FtlResource extends FtlNode {
  FtlResource({required this.entries}) {
    final msgs = <String, FtlMessage>{};
    final trms = <String, FtlTerm>{};
    for (final entry in entries) {
      // First definition wins, matching Fluent's duplicate-ID rule.
      if (entry is FtlMessage) {
        msgs.putIfAbsent(entry.id, () => entry);
      } else if (entry is FtlTerm) {
        trms.putIfAbsent(entry.id, () => entry);
      }
    }
    messages = Map.unmodifiable(msgs);
    terms = Map.unmodifiable(trms);
  }

  final List<FtlEntry> entries;

  /// Immutable lookup map of messages by ID (first definition wins).
  late final Map<String, FtlMessage> messages;

  /// Immutable lookup map of terms by ID (first definition wins).
  late final Map<String, FtlTerm> terms;

  /// Entries that could not be parsed. Parsing continues after them.
  Iterable<FtlJunk> get junk => entries.whereType<FtlJunk>();
}

/// Abstract base for top-level entries in an FTL resource.
sealed class FtlEntry extends FtlNode {
  const FtlEntry();
}

/// An entry describing a localizable message (e.g. `save-button = Save`).
class FtlMessage extends FtlEntry {
  const FtlMessage({
    required this.id,
    this.value,
    this.attributes = const <FtlAttribute>[],
    this.comment,
  });

  final String id;
  final FtlPattern? value;
  final List<FtlAttribute> attributes;
  final String? comment;

  FtlAttribute? findAttribute(String attrName) {
    for (final attr in attributes) {
      if (attr.id == attrName) return attr;
    }
    return null;
  }
}

/// An entry describing a reusable brand or terminology term (e.g. `-brand-name = RusToK`).
class FtlTerm extends FtlEntry {
  const FtlTerm({
    required this.id,
    required this.value,
    this.attributes = const <FtlAttribute>[],
    this.comment,
  });

  final String id;
  final FtlPattern value;
  final List<FtlAttribute> attributes;
  final String? comment;

  FtlAttribute? findAttribute(String attrName) {
    for (final attr in attributes) {
      if (attr.id == attrName) return attr;
    }
    return null;
  }
}

/// An attribute attached to a message or term (e.g. `.placeholder = Enter your email`).
class FtlAttribute extends FtlNode {
  const FtlAttribute({
    required this.id,
    required this.value,
  });

  final String id;
  final FtlPattern value;
}

/// A comment line or block.
class FtlComment extends FtlEntry {
  const FtlComment({required this.content});

  final String content;
}

/// Unparseable top-level content ("junk" in the Fluent specification).
///
/// The parser records junk and continues with the next entry instead of
/// aborting the whole resource.
class FtlJunk extends FtlEntry {
  const FtlJunk({required this.content, this.message});

  /// The raw source text of the junk entry.
  final String content;

  /// Human-readable parse error, if known.
  final String? message;
}

/// A pattern of text and placeable elements forming a translation value.
class FtlPattern extends FtlNode {
  const FtlPattern({required this.elements});

  final List<FtlPatternElement> elements;
}

/// Element inside an [FtlPattern].
sealed class FtlPatternElement extends FtlNode {
  const FtlPatternElement();
}

/// Plain text segment inside a pattern.
class FtlTextElement extends FtlPatternElement {
  const FtlTextElement(this.value);

  final String value;
}

/// A placeable expression wrapped in `{ ... }`.
class FtlPlaceable extends FtlPatternElement {
  const FtlPlaceable(this.expression);

  final FtlExpression expression;
}

/// Abstract expression inside a placeable.
sealed class FtlExpression extends FtlNode {
  const FtlExpression();
}

/// String literal inside an expression (e.g. `"hello"`).
class FtlStringLiteral extends FtlExpression {
  const FtlStringLiteral(this.value);

  final String value;
}

/// Number literal inside an expression (e.g. `42` or `3.14`).
class FtlNumberLiteral extends FtlExpression {
  const FtlNumberLiteral(this.value);

  final num value;
}

/// Variable reference (e.g. `$userName` or `$count`).
class FtlVariableReference extends FtlExpression {
  const FtlVariableReference(this.id);

  final String id;
}

/// Term reference (e.g. `-brand-name`, `-brand-name.gender` or `-brand(case: "gen")`).
class FtlTermReference extends FtlExpression {
  const FtlTermReference({
    required this.id,
    this.attribute,
    this.arguments,
  });

  final String id;
  final String? attribute;

  /// Named literal arguments passed to the term, or `null` when none were given.
  final Map<String, FtlExpression>? arguments;
}

/// Message reference (e.g. `shared-title` or `shared-title.aria-label`).
class FtlMessageReference extends FtlExpression {
  const FtlMessageReference({
    required this.id,
    this.attribute,
  });

  final String id;
  final String? attribute;
}

/// Built-in function call (e.g. `NUMBER($count, minimumFractionDigits: 2)`).
class FtlFunctionReference extends FtlExpression {
  const FtlFunctionReference({
    required this.name,
    this.positional = const <FtlExpression>[],
    this.named = const <String, FtlExpression>{},
  });

  final String name;
  final List<FtlExpression> positional;
  final Map<String, FtlExpression> named;
}

/// Select expression with variant branches (e.g. `{ $count -> [one] ... *[other] ... }`).
class FtlSelectExpression extends FtlExpression {
  const FtlSelectExpression({
    required this.selector,
    required this.variants,
  });

  final FtlExpression selector;
  final List<FtlSelectVariant> variants;

  FtlSelectVariant get defaultVariant =>
      variants.firstWhere((v) => v.isDefault, orElse: () => variants.last);
}

/// A variant arm in a select expression.
class FtlSelectVariant extends FtlNode {
  const FtlSelectVariant({
    required this.key,
    required this.value,
    this.isDefault = false,
  });

  final String key;
  final FtlPattern value;
  final bool isDefault;
}
