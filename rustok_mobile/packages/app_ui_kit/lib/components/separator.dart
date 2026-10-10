// RusToK Mobile UI Kit - Separator Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

/// Divider line separating content, supporting horizontal or vertical orientation.
class RusTokSeparator extends StatelessWidget {
  final Axis orientation;
  final double thickness;
  final Color? color;
  final EdgeInsetsGeometry? margin;

  const RusTokSeparator({
    super.key,
    this.orientation = Axis.horizontal,
    this.thickness = 1.0,
    this.color,
    this.margin,
  });

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final divColor = color ?? (isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder);

    if (orientation == Axis.horizontal) {
      return Container(
        margin: margin,
        height: thickness,
        color: divColor,
      );
    } else {
      return Container(
        margin: margin,
        width: thickness,
        color: divColor,
      );
    }
  }
}
