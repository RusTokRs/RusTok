import 'package:flutter/material.dart';
import 'package:rustok_flutter_i18n/rustok_flutter_i18n.dart';

void main() {
  final catalog = FluentCatalog(useIsolating: false);
  catalog.registerFtl('en', '''
app-title = RusToK Mobile
greeting = Hello, { \$name }!
counter = { \$count ->
    [0] Click the button to start
    [one] Clicked once
   *[other] Clicked { \$count } times
}
''');

  final messages = UiMessages(catalog: catalog, defaultLocale: 'en');

  runApp(
    FluentScope(
      messages: messages,
      currentLocale: 'en',
      child: const DemoApp(),
    ),
  );
}

class DemoApp extends StatefulWidget {
  const DemoApp({super.key});

  @override
  State<DemoApp> createState() => _DemoAppState();
}

class _DemoAppState extends State<DemoApp> {
  int _counter = 0;

  @override
  Widget build(BuildContext context) {
    return DirectionalityHandler(
      child: MaterialApp(
        title: context.t('app-title'),
        home: Scaffold(
          appBar: AppBar(title: Text(context.t('app-title'))),
          body: Center(
            child: Column(
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text(context.t('greeting', args: {'name': 'Developer'})),
                const SizedBox(height: 12),
                Text(context.t('counter', args: {'count': _counter})),
              ],
            ),
          ),
          floatingActionButton: FloatingActionButton(
            onPressed: () => setState(() => _counter++),
            child: const Icon(Icons.add),
          ),
        ),
      ),
    );
  }
}
