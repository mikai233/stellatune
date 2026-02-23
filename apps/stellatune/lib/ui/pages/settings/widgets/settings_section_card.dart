import 'package:flutter/material.dart';

class SettingsSectionCard extends StatelessWidget {
  const SettingsSectionCard({
    super.key,
    required this.title,
    required this.children,
    this.trailing,
    this.padding = const EdgeInsets.all(12),
    this.headerBottomSpacing = 12,
  });

  final String title;
  final List<Widget> children;
  final Widget? trailing;
  final EdgeInsetsGeometry padding;
  final double headerBottomSpacing;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: padding,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    title,
                    style: Theme.of(context).textTheme.titleMedium,
                  ),
                ),
                ?trailing,
              ],
            ),
            if (children.isNotEmpty) SizedBox(height: headerBottomSpacing),
            ...children,
          ],
        ),
      ),
    );
  }
}
