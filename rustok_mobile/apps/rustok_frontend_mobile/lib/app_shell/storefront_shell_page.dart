import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../routes/storefront_router.dart';

class StorefrontShellPage extends StatelessWidget {
  const StorefrontShellPage({super.key, required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('RusTok Storefront')),
      body: SafeArea(child: child),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _selectedIndex(context),
        onDestinationSelected: (index) => context.go(_destinationPath(index)),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.home_outlined),
            selectedIcon: Icon(Icons.home),
            label: 'Home',
          ),
          NavigationDestination(
            icon: Icon(Icons.category_outlined),
            selectedIcon: Icon(Icons.category),
            label: 'Catalog',
          ),
          NavigationDestination(
            icon: Icon(Icons.shopping_cart_outlined),
            selectedIcon: Icon(Icons.shopping_cart),
            label: 'Cart',
          ),
          NavigationDestination(
            icon: Icon(Icons.person_outline),
            selectedIcon: Icon(Icons.person),
            label: 'Profile',
          ),
        ],
      ),
    );
  }

  int _selectedIndex(BuildContext context) {
    final path = GoRouterState.of(context).uri.path;
    if (path.startsWith(catalogPath)) {
      return 1;
    }
    if (path.startsWith(cartPath)) {
      return 2;
    }
    if (path.startsWith(profilePath)) {
      return 3;
    }
    return 0;
  }

  String _destinationPath(int index) {
    return switch (index) {
      1 => catalogPath,
      2 => cartPath,
      3 => profilePath,
      _ => homePath,
    };
  }
}
