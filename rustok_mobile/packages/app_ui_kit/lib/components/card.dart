// RusToK Mobile UI Kit - Card Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

/// Card container for grouping content and actions.
class RusTokCard extends StatelessWidget {
  final Widget child;
  final EdgeInsetsGeometry? padding;
  final EdgeInsetsGeometry? margin;
  final Color? backgroundColor;
  final BorderSide? borderSide;
  final BorderRadius? borderRadius;
  final VoidCallback? onTap;

  const RusTokCard({
    super.key,
    required this.child,
    this.padding,
    this.margin,
    this.backgroundColor,
    this.borderSide,
    this.borderRadius,
    this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final bg = backgroundColor ?? (isDark ? RusTokColors.darkBg : RusTokColors.lightBg);
    final border = borderSide ??
        BorderSide(
          color: isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder,
          width: 1.0,
        );
    final radius = borderRadius ?? RusTokRadius.borderMd;

    Widget content = Container(
      margin: margin,
      decoration: BoxDecoration(
        color: bg,
        borderRadius: radius,
        border: Border.fromBorderSide(border),
      ),
      child: ClipRRect(
        borderRadius: radius,
        child: Padding(
          padding: padding ?? const EdgeInsets.all(RusTokSpacing.space4),
          child: child,
        ),
      ),
    );

    if (onTap != null) {
      return InkWell(
        onTap: onTap,
        borderRadius: radius,
        child: content,
      );
    }

    return content;
  }
}

/// Header section of a RusTokCard containing title and optional action/description.
class RusTokCardHeader extends StatelessWidget {
  final Widget? title;
  final Widget? description;
  final Widget? action;

  const RusTokCardHeader({
    super.key,
    this.title,
    this.description,
    this.action,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: RusTokSpacing.space3),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                if (title != null) title!,
                if (description != null) ...[
                  const SizedBox(height: RusTokSpacing.space1),
                  description!,
                ],
              ],
            ),
          ),
          if (action != null) action!,
        ],
      ),
    );
  }
}

/// Title text within a RusTokCardHeader.
class RusTokCardTitle extends StatelessWidget {
  final String text;

  const RusTokCardTitle(this.text, {super.key});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final color = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;

    return Text(
      text,
      style: TextStyle(
        fontSize: 16.0,
        fontWeight: FontWeight.w600,
        color: color,
        letterSpacing: -0.2,
      ),
    );
  }
}

/// Description text within a RusTokCardHeader.
class RusTokCardDescription extends StatelessWidget {
  final String text;

  const RusTokCardDescription(this.text, {super.key});

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final color = isDark ? RusTokColors.darkMutedFg : RusTokColors.lightMutedFg;

    return Text(
      text,
      style: TextStyle(
        fontSize: 13.0,
        fontWeight: FontWeight.w400,
        color: color,
      ),
    );
  }
}

/// Content wrapper for a RusTokCard.
class RusTokCardContent extends StatelessWidget {
  final Widget child;

  const RusTokCardContent({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    return child;
  }
}

/// Footer actions container for a RusTokCard.
class RusTokCardFooter extends StatelessWidget {
  final Widget child;

  const RusTokCardFooter({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: RusTokSpacing.space4),
      child: child,
    );
  }
}
