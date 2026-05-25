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
    expect(blog.surfaceKind, MobileSurfaceKind.admin);
    expect(blog.localeNamespace, isNull);
    expect(blog.permissions, isEmpty);
    expect(blog.path, '/modules/blog');
    expect(blog.navTitle, 'Blog');
    expect(blog.childRoutes, hasLength(2));
    expect(blog.childRoutes.first.subpath, 'new');
    expect(blog.childRoutes.first.path, '/modules/blog/new');
    expect(blog.childRoutes.first.navLabel, 'Add Blog Post');
    expect(blog.childRoutes.last.navLabel, 'All Posts');
  });

  test('skips invalid module and child segments after sanitization', () {
    final entries = <MobileModuleEntry>[
      const MobileModuleEntry(
        moduleKey: ' ',
        routeSegment: '/blog/',
        nav: MobileNavMeta(title: 'Blog', icon: 'article'),
      ),
      const MobileModuleEntry(
        moduleKey: 'rustok_pages',
        routeSegment: '///',
        nav: MobileNavMeta(title: 'Pages', icon: 'module'),
      ),
      const MobileModuleEntry(
        moduleKey: 'rustok_search',
        routeSegment: '/search/',
        nav: MobileNavMeta(title: 'Search', icon: 'search'),
        childPages: [
          MobileChildPage(subpath: '///', title: 'Invalid child'),
          MobileChildPage(subpath: '/analytics/', title: 'Analytics'),
        ],
      ),
    ];

    final adapted = adaptModuleEntries(entries);

    expect(adapted, hasLength(1));
    final search = adapted.single;
    expect(search.moduleKey, 'rustok_search');
    expect(search.path, '/modules/search');
    expect(search.childRoutes, hasLength(1));
    expect(search.childRoutes.single.path, '/modules/search/analytics');
  });

  test('deduplicates conflicting module keys, route segments, and child subpaths', () {
    final entries = <MobileModuleEntry>[
      const MobileModuleEntry(
        moduleKey: ' rustok_blog ',
        routeSegment: '/blog/',
        nav: MobileNavMeta(title: 'Blog', icon: 'article'),
        childPages: [
          MobileChildPage(subpath: 'posts', title: 'Posts A'),
          MobileChildPage(subpath: '/posts/', title: 'Posts B'),
          MobileChildPage(subpath: 'new', title: 'New Post'),
        ],
      ),
      const MobileModuleEntry(
        moduleKey: 'rustok_blog',
        routeSegment: '/blog-v2/',
        nav: MobileNavMeta(title: 'Blog Duplicate Key', icon: 'article'),
      ),
      const MobileModuleEntry(
        moduleKey: 'rustok_blog_alt',
        routeSegment: '/blog/',
        nav: MobileNavMeta(title: 'Blog Duplicate Route', icon: 'article'),
      ),
    ];

    final adapted = adaptModuleEntries(entries);

    expect(adapted, hasLength(1));
    final blog = adapted.single;
    expect(blog.moduleKey, 'rustok_blog');
    expect(blog.routeSegment, 'blog');
    expect(blog.childRoutes, hasLength(2));
    expect(blog.childRoutes.map((c) => c.subpath), ['posts', 'new']);
  });


  test('normalizes case and rejects non-canonical segments', () {
    final entries = <MobileModuleEntry>[
      const MobileModuleEntry(
        moduleKey: 'rustok_media',
        routeSegment: '/Media/',
        nav: MobileNavMeta(title: 'Media', icon: 'perm_media'),
        childPages: [
          MobileChildPage(subpath: 'Library', title: 'Library'),
          MobileChildPage(subpath: 'bad/path', title: 'Invalid Nested'),
          MobileChildPage(subpath: 'bad path', title: 'Invalid Space'),
        ],
      ),
    ];

    final adapted = adaptModuleEntries(entries);

    expect(adapted, hasLength(1));
    final media = adapted.single;
    expect(media.routeSegment, 'media');
    expect(media.path, '/modules/media');
    expect(media.childRoutes, hasLength(1));
    expect(media.childRoutes.single.subpath, 'library');
    expect(media.childRoutes.single.path, '/modules/media/library');
  });



  test('returns adaptation report with rejected module and child counters', () {
    final entries = <MobileModuleEntry>[
      const MobileModuleEntry(
        moduleKey: 'rustok_valid',
        routeSegment: 'valid',
        nav: MobileNavMeta(title: 'Valid', icon: 'module'),
        childPages: [
          MobileChildPage(subpath: 'good', title: 'Good Child'),
          MobileChildPage(subpath: 'bad/child', title: 'Bad Child'),
          MobileChildPage(subpath: 'good', title: 'Duplicate Child'),
        ],
      ),
      const MobileModuleEntry(
        moduleKey: '',
        routeSegment: 'invalid',
        nav: MobileNavMeta(title: 'Invalid Module', icon: 'module'),
      ),
      const MobileModuleEntry(
        moduleKey: 'rustok_valid',
        routeSegment: 'valid-duplicate',
        nav: MobileNavMeta(title: 'Duplicate Key', icon: 'module'),
      ),
    ];

    final report = adaptModuleEntriesWithReport(entries);

    expect(report.routes, hasLength(1));
    expect(report.rejectedModuleEntries, 2);
    expect(report.rejectedChildEntries, 2);
  });

  test('preserves frozen registry metadata for FFA-safe host wiring', () {
    final entries = <MobileModuleEntry>[
      const MobileModuleEntry(
        moduleKey: 'rustok_modules',
        surfaceKind: MobileSurfaceKind.admin,
        routeSegment: 'modules',
        localeNamespace: 'modules',
        permissions: ['modules.read', 'modules.write'],
        nav: MobileNavMeta(title: 'Modules', icon: 'extension'),
      ),
    ];

    final adapted = adaptModuleEntries(entries).single;

    expect(adapted.surfaceKind, MobileSurfaceKind.admin);
    expect(adapted.localeNamespace, 'modules');
    expect(adapted.permissions, ['modules.read', 'modules.write']);
  });

}
