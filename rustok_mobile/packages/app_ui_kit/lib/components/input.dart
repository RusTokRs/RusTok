// RusToK Mobile UI Kit - Input Component
// Part of RusToK Design System (FFA Compatible)

import 'package:flutter/material.dart';
import '../tokens/rustok_tokens.g.dart';

/// Single-line or multi-line text input field styled with RusToK tokens.
class RusTokInput extends StatelessWidget {
  final TextEditingController? controller;
  final String? initialValue;
  final String? hintText;
  final String? labelText;
  final String? errorText;
  final bool disabled;
  final bool obscureText;
  final Widget? prefix;
  final Widget? suffix;
  final ValueChanged<String>? onChanged;
  final ValueChanged<String>? onSubmitted;
  final int maxLines;
  final TextInputType? keyboardType;
  final FocusNode? focusNode;

  const RusTokInput({
    super.key,
    this.controller,
    this.initialValue,
    this.hintText,
    this.labelText,
    this.errorText,
    this.disabled = false,
    this.obscureText = false,
    this.prefix,
    this.suffix,
    this.onChanged,
    this.onSubmitted,
    this.maxLines = 1,
    this.keyboardType,
    this.focusNode,
  });

  @override
  Widget build(BuildContext context) {
    final isDark = Theme.of(context).brightness == Brightness.dark;
    final fgColor = isDark ? RusTokColors.darkFg : RusTokColors.lightFg;
    final mutedFg = isDark ? RusTokColors.darkMutedFg : RusTokColors.lightMutedFg;
    final borderColor = isDark ? RusTokColors.darkBorder : RusTokColors.lightBorder;
    final dangerColor = isDark ? RusTokColors.darkDanger : RusTokColors.lightDanger;
    final bgColor = isDark ? RusTokColors.darkBg : RusTokColors.lightBg;

    final hasError = errorText != null && errorText!.isNotEmpty;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        if (labelText != null) ...[
          Text(
            labelText!,
            style: TextStyle(
              fontSize: 13.0,
              fontWeight: FontWeight.w500,
              color: hasError ? dangerColor : fgColor,
            ),
          ),
          const SizedBox(height: RusTokSpacing.space1),
        ],
        TextFormField(
          controller: controller,
          initialValue: initialValue,
          enabled: !disabled,
          obscureText: obscureText,
          maxLines: maxLines,
          keyboardType: keyboardType,
          focusNode: focusNode,
          onChanged: onChanged,
          onFieldSubmitted: onSubmitted,
          style: TextStyle(
            fontSize: 14.0,
            color: disabled ? mutedFg : fgColor,
          ),
          decoration: InputDecoration(
            isDense: true,
            filled: true,
            fillColor: disabled
                ? (isDark ? RusTokColors.darkMuted : RusTokColors.lightMuted)
                : bgColor,
            hintText: hintText,
            hintStyle: TextStyle(
              fontSize: 14.0,
              color: mutedFg,
            ),
            prefixIcon: prefix,
            suffixIcon: suffix,
            contentPadding: const EdgeInsets.symmetric(
              horizontal: RusTokSpacing.space3,
              vertical: RusTokSpacing.space3,
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: RusTokRadius.borderMd,
              borderSide: BorderSide(
                color: hasError ? dangerColor : borderColor,
                width: 1.0,
              ),
            ),
            focusedBorder: OutlineInputBorder(
              borderRadius: RusTokRadius.borderMd,
              borderSide: BorderSide(
                color: hasError ? dangerColor : fgColor,
                width: 1.5,
              ),
            ),
            disabledBorder: OutlineInputBorder(
              borderRadius: RusTokRadius.borderMd,
              borderSide: BorderSide(
                color: borderColor.withValues(alpha: 0.5),
                width: 1.0,
              ),
            ),
            errorBorder: OutlineInputBorder(
              borderRadius: RusTokRadius.borderMd,
              borderSide: BorderSide(
                color: dangerColor,
                width: 1.0,
              ),
            ),
            focusedErrorBorder: OutlineInputBorder(
              borderRadius: RusTokRadius.borderMd,
              borderSide: BorderSide(
                color: dangerColor,
                width: 1.5,
              ),
            ),
          ),
        ),
        if (hasError) ...[
          const SizedBox(height: RusTokSpacing.space1),
          Text(
            errorText!,
            style: TextStyle(
              fontSize: 12.0,
              color: dangerColor,
              fontWeight: FontWeight.w400,
            ),
          ),
        ],
      ],
    );
  }
}
