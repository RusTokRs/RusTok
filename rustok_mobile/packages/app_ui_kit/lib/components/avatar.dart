// RusToK Mobile UI Kit - Avatar Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

/// Avatar component for user profiles or resource thumbnails.
class RusTokAvatar extends StatelessWidget {
  final ImageProvider? image;
  final String? initials;
  final Widget? fallbackIcon;
  final double size;
  final BorderRadius? borderRadius;

  const RusTokAvatar({
    super.key,
    this.image,
    this.initials,
    this.fallbackIcon,
    this.size = 40.0,
    this.borderRadius,
  });

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final bg = isDark ? RusTokColors.darkMuted : RusTokColors.lightMuted;
    final fg = isDark ? RusTokColors.darkMutedFg : RusTokColors.lightMutedFg;
    final border = isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder;
    final radius = borderRadius ?? BorderRadius.circular(size / 2);

    return Container(
      width: size,
      height: size,
      decoration: BoxDecoration(
        color: bg,
        borderRadius: radius,
        border: Border.all(color: border, width: 1.0),
        image: image != null
            ? DecorationImage(
                image: image!,
                fit: BoxFit.cover,
              )
            : null,
      ),
      alignment: Alignment.center,
      child: image == null
          ? (initials != null
              ? Text(
                  initials!,
                  style: TextStyle(
                    fontSize: size * 0.4,
                    fontWeight: FontWeight.w600,
                    color: isDark ? RusTokColors.darkFg : RusTokColors.lightFg,
                  ),
                )
              : (fallbackIcon ??
                  Icon(
                    Icons.person,
                    size: size * 0.5,
                    color: fg,
                  )))
          : null,
    );
  }
}
