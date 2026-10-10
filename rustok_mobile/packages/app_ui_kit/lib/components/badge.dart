// RusToK Mobile UI Kit - Badge Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

enum RusTokBadgeVariant {
  defaultVariant,
  secondary,
  destructive,
  outline,
  success,
  warning,
}

class RusTokBadge extends StatelessWidget {
  final Widget? child;
  final String? label;
  final RusTokBadgeVariant variant;

  const RusTokBadge({
    super.key,
    this.child,
    this.label,
    this.variant = RusTokBadgeVariant.defaultVariant,
  }) : assert(child != null || label != null, 'Either child or label must be provided');

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;

    Color bgColor;
    Color fgColor;
    BorderSide? borderSide;

    switch (variant) {
      case RusTokBadgeVariant.defaultVariant:
        bgColor = isDark ? RusTokColors.darkPrimary : RusTokColors.lightPrimary;
        fgColor = isDark ? RusTokColors.darkPrimaryFg : RusTokColors.lightPrimaryFg;
        break;
      case RusTokBadgeVariant.secondary:
        bgColor = isDark ? RusTokColors.darkMuted : RusTokColors.lightMuted;
        fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
        break;
      case RusTokBadgeVariant.destructive:
        bgColor = isDark ? RusTokColors.darkDanger : RusTokColors.lightDanger;
        fgColor = isDark ? RusTokColors.darkDangerFg : RusTokColors.lightDangerFg;
        break;
      case RusTokBadgeVariant.outline:
        bgColor = Colors.transparent;
        fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
        borderSide = BorderSide(
          color: isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder,
          width: 1.0,
        );
        break;
      case RusTokBadgeVariant.success:
        bgColor = const Color(0xFF10B981).withValues(alpha: 0.15);
        fgColor = const Color(0xFF10B981);
        break;
      case RusTokBadgeVariant.warning:
        bgColor = const Color(0xFFF59E0B).withValues(alpha: 0.15);
        fgColor = const Color(0xFFF59E0B);
        break;
    }

    return Container(
      padding: const EdgeInsets.symmetric(
        horizontal: RusTokSpacing.space2,
        vertical: 2.0,
      ),
      decoration: BoxDecoration(
        color: bgColor,
        borderRadius: RusTokRadius.borderSm,
        border: borderSide != null ? Border.fromBorderSide(borderSide) : null,
      ),
      child: child ?? Text(
        label!,
        style: TextStyle(
          fontSize: 11.0,
          fontWeight: FontWeight.w500,
          color: fgColor,
        ),
      ),
    );
  }
}
