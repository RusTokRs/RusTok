import 'package:app_module_contracts/app_module_contracts.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:rustok_admin_mobile/registry/module_entry_adapter.dart';

void main() {
  test('adapts module entries and child routes into canonical module paths', () {
    final entries = <MobileModuleEntry>[
      const MobileModuleEntry(
        moduleKey: 'rustok_blog',
        routeSegment: '/blog/',
        nav: MobileNavMeta(title: 'Blog', icon: 'article'),
        childPages: [
          MobileChildPage(subpath: '/new/', title: 'Add Blog Post'),
          MobileChildPage(
            subpath: 'posts',
            title: 'All Blog Posts',
            navLabel: 'All Posts',
          ),
        ],
      ),
    ];

    final adapted = adaptModuleEntries(entries);

    expect(adapted, hasLength(1));
    final blog = adapted.first;
    expect(blog.routeSegment, 'blog');
    expect(blog.path, '/modules/blog');
    expect(blog.navTitle, 'Blog');
    expect(blog.childRoutes, hasLength(2));
    expect(blog.childRoutes.first.subpath, 'new');
    expect(blog.childRoutes.first.path, '/modules/blog/new');
    expect(blog.childRoutes.first.navLabel, 'Add Blog Post');
    expect(blog.childRoutes.last.navLabel, 'All Posts');
  });
}
