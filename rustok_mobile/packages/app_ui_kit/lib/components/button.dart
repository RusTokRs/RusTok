// RusToK Mobile UI Kit - Button Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

enum RusTokButtonVariant {
  defaultVariant,
  destructive,
  outline,
  secondary,
  ghost,
  link,
}

enum RusTokButtonSize {
  sm,
  md,
  lg,
}

class RusTokButton extends StatelessWidget {
  final Widget? child;
  final String? text;
  final VoidCallback? onPressed;
  final RusTokButtonVariant variant;
  final RusTokButtonSize size;
  final bool disabled;
  final bool loading;
  final Widget? icon;

  const RusTokButton({
    super.key,
    this.child,
    this.text,
    this.onPressed,
    this.variant = RusTokButtonVariant.defaultVariant,
    this.size = RusTokButtonSize.md,
    this.disabled = false,
    this.loading = false,
    this.icon,
  }) : assert(child != null || text != null, 'Either child or text must be provided');

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final isInteractive = !disabled && !loading && onPressed != null;

    // Resolve colors based on variant and brightness
    Color bgColor;
    Color fgColor;
    BorderSide? borderSide;

    switch (variant) {
      case RusTokButtonVariant.defaultVariant:
        bgColor = isDark ? RusTokColors.darkPrimary : RusTokColors.lightPrimary;
        fgColor = isDark ? RusTokColors.darkPrimaryFg : RusTokColors.lightPrimaryFg;
        break;
      case RusTokButtonVariant.destructive:
        bgColor = isDark ? RusTokColors.darkDanger : RusTokColors.lightDanger;
        fgColor = isDark ? RusTokColors.darkDangerFg : RusTokColors.lightDangerFg;
        break;
      case RusTokButtonVariant.outline:
        bgColor = Colors.transparent;
        fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
        borderSide = BorderSide(
          color: isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder,
          width: 1.0,
        );
        break;
      case RusTokButtonVariant.secondary:
        bgColor = isDark ? RusTokColors.darkMuted : RusTokColors.lightMuted;
        fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
        break;
      case RusTokButtonVariant.ghost:
        bgColor = Colors.transparent;
        fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
        break;
      case RusTokButtonVariant.link:
        bgColor = Colors.transparent;
        fgColor = isDark ? RusTokColors.darkPrimary : RusTokColors.lightPrimary;
        break;
    }

    if (!isInteractive) {
      bgColor = bgColor.withValues(alpha: 0.5);
      fgColor = fgColor.withValues(alpha: 0.5);
    }

    // Resolve size padding and height
    final double height;
    final EdgeInsets padding;
    final double fontSize;

    switch (size) {
      case RusTokButtonSize.sm:
        height = 32.0;
        padding = const EdgeInsets.symmetric(horizontal: RusTokSpacing.space3);
        fontSize = 12.0;
        break;
      case RusTokButtonSize.md:
        height = 40.0;
        padding = const EdgeInsets.symmetric(horizontal: RusTokSpacing.space4);
        fontSize = 14.0;
        break;
      case RusTokButtonSize.lg:
        height = 48.0;
        padding = const EdgeInsets.symmetric(horizontal: RusTokSpacing.space6);
        fontSize = 16.0;
        break;
    }

    Widget content = child ?? Text(
      text!,
      style: TextStyle(
        fontSize: fontSize,
        fontWeight: FontWeight.w500,
        color: fgColor,
        decoration: variant == RusTokButtonVariant.link ? TextDecoration.underline : TextDecoration.none,
      ),
    );

    if (loading) {
      content = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: fontSize,
            height: fontSize,
            child: CircularProgressIndicator(
              strokeWidth: 2.0,
              valueColor: AlwaysStoppedAnimation<Color>(fgColor),
            ),
          ),
          const SizedBox(width: RusTokSpacing.space2),
          content,
        ],
      );
    } else if (icon != null) {
      content = Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          icon!,
          const SizedBox(width: RusTokSpacing.space2),
          content,
        ],
      );
    }

    return Material(
      color: bgColor,
      shape: RoundedRectangleBorder(
        borderRadius: RusTokRadius.borderMd,
        side: borderSide ?? BorderSide.none,
      ),
      child: InkWell(
        onTap: isInteractive ? onPressed : null,
        borderRadius: RusTokRadius.borderMd,
        child: Container(
          height: height,
          padding: padding,
          alignment: Alignment.center,
          child: content,
        ),
      ),
    );
  }
}
