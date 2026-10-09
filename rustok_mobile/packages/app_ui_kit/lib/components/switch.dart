// RusToK Mobile UI Kit - Switch Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

/// Toggle switch styled with RusToK design tokens.
class RusTokSwitch extends StatelessWidget {
  final bool value;
  final ValueChanged<bool>? onChanged;
  final bool disabled;
  final String? label;

  const RusTokSwitch({
    super.key,
    required this.value,
    this.onChanged,
    this.disabled = false,
    this.label,
  });

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final primaryColor = isDark ? RusTokColors.darkPrimary : RusTokColors.lightPrimary;
    final primaryFg = isDark ? RusTokColors.darkPrimaryFg : RusTokColors.lightPrimaryFg;
    final mutedColor = isDark ? RusTokColors.darkMuted : RusTokColors.lightMuted;
    final mutedFg = isDark ? RusTokColors.darkMutedFg : RusTokColors.lightMutedFg;
    final fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;

    final track = GestureDetector(
      onTap: disabled ? null : () => onChanged?.call(!value),
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeInOut,
        width: 44.0,
        height: 24.0,
        padding: const EdgeInsets.all(2.0),
        decoration: BoxDecoration(
          borderRadius: BorderRadius.circular(12.0),
          color: value
              ? (disabled ? mutedFg : primaryColor)
              : (disabled ? mutedColor.withValues(alpha: 0.5) : mutedColor),
          border: Border.all(
            color: isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder,
            width: 1.0,
          ),
        ),
        alignment: value ? Alignment.centerRight : Alignment.centerLeft,
        child: Container(
          width: 18.0,
          height: 18.0,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            color: value ? primaryFg : (isDark ? RusTokColors.darkFg : RusTokColors.lightFg),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withValues(alpha: 0.15),
                blurRadius: 2.0,
                offset: const Offset(0, 1),
              ),
            ],
          ),
        ),
      ),
    );

    if (label != null) {
      return GestureDetector(
        onTap: disabled ? null : () => onChanged?.call(!value),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            track,
            const SizedBox(width: RusTokSpacing.space2),
            Text(
              label!,
              style: TextStyle(
                fontSize: 14.0,
                color: disabled ? mutedFg : fgColor,
              ),
            ),
          ],
        ),
      );
    }

    return track;
  }
}
