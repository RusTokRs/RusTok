from pathlib import Path

path = Path("crates/rustok-seo/src/services/services_base.rs")
source = path.read_text()
old = '''        let service = Self::new(db, event_bus, registry);
        Ok(extensions
            .get::<SeoMediaAssetReadProvider>()
            .map(|provider| service.with_media_asset_read_port(provider.port()))
            .unwrap_or(service))
'''
new = '''        let service = Self::new(db, event_bus, registry);
        if let Some(provider) = extensions.get::<SeoMediaAssetReadProvider>() {
            Ok(service.with_media_asset_read_port(provider.port()))
        } else {
            Ok(service)
        }
'''
count = source.count(old)
if count != 1:
    raise RuntimeError(f"SEO service ownership fix: expected 1 match, got {count}")
path.write_text(source.replace(old, new, 1))
