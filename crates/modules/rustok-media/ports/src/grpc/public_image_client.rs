use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::dto::{MediaImageDescriptor, MediaPublicImageAsset};
use crate::ports::MediaPublicImageReadPort;

use super::client::GrpcMediaProvider;

/// Public-image-only remote adapter. Binary image delivery remains on the Media HTTP route.
pub struct GrpcMediaPublicImageProvider {
    inner: GrpcMediaProvider,
    public_origin: Option<String>,
}

impl GrpcMediaPublicImageProvider {
    pub fn new(inner: GrpcMediaProvider, public_origin: Option<String>) -> Self {
        Self {
            inner,
            public_origin,
        }
    }
}

#[async_trait]
impl MediaPublicImageReadPort for GrpcMediaPublicImageProvider {
    async fn get_public_image_asset(
        &self,
        context: PortContext,
        media_id: Uuid,
        alt: Option<String>,
    ) -> Result<MediaPublicImageAsset, PortError> {
        let mut asset = self
            .inner
            .get_public_image_asset(context, media_id, alt)
            .await?;
        rebase_public_descriptor(asset.descriptor.as_mut(), self.public_origin.as_deref());
        Ok(asset)
    }
}

pub fn rebase_public_descriptor(
    descriptor: Option<&mut MediaImageDescriptor>,
    public_origin: Option<&str>,
) {
    let (Some(descriptor), Some(public_origin)) = (descriptor, public_origin) else {
        return;
    };
    let public_origin = public_origin.trim_end_matches('/');
    if descriptor.url.starts_with('/') && !descriptor.url.starts_with("//") {
        descriptor.url = format!("{public_origin}{}", descriptor.url);
    }
}

#[cfg(test)]
mod tests {
    use super::rebase_public_descriptor;
    use crate::dto::MediaImageDescriptor;

    #[test]
    fn public_origin_rebases_only_root_relative_descriptors() {
        let mut relative = MediaImageDescriptor::from_parts(
            "/api/media/public/images/asset/checksum",
            None,
            None,
            None,
            Some("image/png".to_string()),
        )
        .expect("relative descriptor");
        rebase_public_descriptor(Some(&mut relative), Some("https://media.example.com"));
        assert_eq!(
            relative.url,
            "https://media.example.com/api/media/public/images/asset/checksum"
        );

        let mut absolute = MediaImageDescriptor::from_parts(
            "https://cdn.example.com/image.png",
            None,
            None,
            None,
            Some("image/png".to_string()),
        )
        .expect("absolute descriptor");
        rebase_public_descriptor(Some(&mut absolute), Some("https://media.example.com"));
        assert_eq!(absolute.url, "https://cdn.example.com/image.png");
    }

    #[test]
    fn public_origin_with_trailing_slash_does_not_duplicate_slashes() {
        let mut relative = MediaImageDescriptor::from_parts(
            "/api/media/public/images/asset/checksum",
            None,
            None,
            None,
            Some("image/png".to_string()),
        )
        .expect("relative descriptor");
        rebase_public_descriptor(Some(&mut relative), Some("https://media.example.com/"));
        assert_eq!(
            relative.url,
            "https://media.example.com/api/media/public/images/asset/checksum"
        );
    }
}
