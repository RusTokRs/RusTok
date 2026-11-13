import 'package:flutter/material.dart';

import 'module_entry_adapter.dart';

const _explicitIconMap = <String, IconData>{
  'account_tree': Icons.account_tree_outlined,
  'apartment': Icons.apartment_outlined,
  'article': Icons.article_outlined,
  'chat': Icons.chat_bubble_outline,
  'extension': Icons.extension_outlined,
  'forum': Icons.forum_outlined,
  'inventory': Icons.inventory_outlined,
  'inventory_2': Icons.inventory_2_outlined,
  'people': Icons.people_outline,
  'perm_media': Icons.perm_media_outlined,
  'receipt_long': Icons.receipt_long_outlined,
  'search': Icons.search,
  'shield': Icons.shield_outlined,
  'travel_explore': Icons.travel_explore_outlined,
};

const _genericModuleFallbacks = <String, IconData>{
  'rustok_channel': Icons.hub_outlined,
  'rustok_commerce': Icons.storefront_outlined,
  'rustok_fulfillment': Icons.local_shipping_outlined,
  'rustok_index': Icons.view_list_outlined,
  'rustok_outbox': Icons.outbox_outlined,
  'rustok_pages': Icons.web_stories_outlined,
  'rustok_pricing': Icons.sell_outlined,
  'rustok_region': Icons.public_outlined,
};

IconData iconForModuleRoute(ModuleRouteEntry route) {
  final explicit = _explicitIconMap[route.navIcon];
  if (explicit != null && route.navIcon != 'module') {
    return explicit;
  }

  return _genericModuleFallbacks[route.moduleKey] ??
      _routeSegmentFallback(route.routeSegment) ??
      explicit ??
      Icons.extension_outlined;
}

IconData? _routeSegmentFallback(String routeSegment) {
  return switch (routeSegment) {
    'channels' => Icons.hub_outlined,
    'commerce' => Icons.storefront_outlined,
    'fulfillment' => Icons.local_shipping_outlined,
    'index' => Icons.view_list_outlined,
    'outbox' => Icons.outbox_outlined,
    'pages' => Icons.web_stories_outlined,
    'pricing' => Icons.sell_outlined,
    'regions' => Icons.public_outlined,
    _ => null,
  };
}
