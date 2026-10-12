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
      if (entry is FtlMessage) {
        msgs[entry.id] = entry;
      } else if (entry is FtlTerm) {
        trms[entry.id] = entry;
      }
    }
    messages = Map.unmodifiable(msgs);
    terms = Map.unmodifiable(trms);
  }

  final List<FtlEntry> entries;

  /// Immutable lookup map of messages by ID.
  late final Map<String, FtlMessage> messages;

  /// Immutable lookup map of terms by ID.
  late final Map<String, FtlTerm> terms;
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

/// Term reference (e.g. `-brand-name` or `-brand-name.gender`).
class FtlTermReference extends FtlExpression {
  const FtlTermReference({
    required this.id,
    this.attribute,
    this.arguments,
  });

  final String id;
  final String? attribute;
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
