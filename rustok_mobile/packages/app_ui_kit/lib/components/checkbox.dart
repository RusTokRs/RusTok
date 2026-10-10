// RusToK Mobile UI Kit - Checkbox Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

/// Checkbox input styled with RusToK design tokens.
class RusTokCheckbox extends StatelessWidget {
  final bool value;
  final ValueChanged<bool?>? onChanged;
  final bool disabled;
  final String? label;
  final Widget? labelWidget;

  const RusTokCheckbox({
    super.key,
    required this.value,
    this.onChanged,
    this.disabled = false,
    this.label,
    this.labelWidget,
  });

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final primaryColor = isDark ? RusTokColors.darkPrimary : RusTokColors.lightPrimary;
    final primaryFg = isDark ? RusTokColors.darkPrimaryFg : RusTokColors.lightPrimaryFg;
    final borderColor = isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder;
    final fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
    final mutedFg = isDark ? RusTokColors.darkMutedFg : RusTokColors.lightMutedFg;

    final box = InkWell(
      onTap: disabled ? null : () => onChanged?.call(!value),
      borderRadius: RusTokRadius.borderSm,
      child: Container(
        width: 18.0,
        height: 18.0,
        decoration: BoxDecoration(
          color: value
              ? (disabled ? mutedFg : primaryColor)
              : Colors.transparent,
          borderRadius: RusTokRadius.borderSm,
          border: Border.all(
            color: value
                ? (disabled ? mutedFg : primaryColor)
                : borderColor,
            width: 1.5,
          ),
        ),
        alignment: Alignment.center,
        child: value
            ? Icon(
                Icons.check,
                size: 13.0,
                color: primaryFg,
              )
            : null,
      ),
    );

    if (label != null || labelWidget != null) {
      return GestureDetector(
        onTap: disabled ? null : () => onChanged?.call(!value),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            box,
            const SizedBox(width: RusTokSpacing.space2),
            if (labelWidget != null)
              labelWidget!
            else
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

    return box;
  }
}
