use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Hero,
    #[default]
    Text,
    Image,
    Gallery,
    Cta,
    Features,
    Testimonials,
    Pricing,
    Faq,
    Contact,
    ProductGrid,
    Newsletter,
    Video,
    Html,
    Spacer,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HeroBlockData {
    pub title: String,
    pub subtitle: Option<String>,
    pub background_image_url: Option<String>,
    pub cta_label: Option<String>,
    pub cta_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TextBlockData {
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageBlockData {
    pub src: String,
    pub alt: Option<String>,
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GalleryBlockData {
    pub images: Vec<ImageBlockData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CtaBlockData {
    pub title: String,
    pub description: Option<String>,
    pub button_label: String,
    pub button_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FeaturesBlockData {
    pub title: Option<String>,
    pub items: Vec<FeatureItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FeatureItem {
    pub title: String,
    pub description: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TestimonialsBlockData {
    pub title: Option<String>,
    pub items: Vec<TestimonialItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TestimonialItem {
    pub quote: String,
    pub author: String,
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PricingBlockData {
    pub title: Option<String>,
    pub plans: Vec<PricingPlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PricingPlan {
    pub name: String,
    pub price: String,
    pub period: Option<String>,
    pub features: Vec<String>,
    pub cta_label: Option<String>,
    pub cta_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FaqBlockData {
    pub title: Option<String>,
    pub items: Vec<FaqItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FaqItem {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ContactBlockData {
    pub title: Option<String>,
    pub description: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductGridBlockData {
    pub title: Option<String>,
    pub product_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewsletterBlockData {
    pub title: Option<String>,
    pub description: Option<String>,
    pub submit_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VideoBlockData {
    pub provider: String,
    pub url: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct HtmlBlockData {
    pub html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SpacerBlockData {
    pub size: Option<String>,
    pub height_px: Option<u16>,
}

#[derive(Debug, Clone)]
pub enum BlockPayload {
    Hero(HeroBlockData),
    Text(TextBlockData),
    Image(ImageBlockData),
    Gallery(GalleryBlockData),
    Cta(CtaBlockData),
    Features(FeaturesBlockData),
    Testimonials(TestimonialsBlockData),
    Pricing(PricingBlockData),
    Faq(FaqBlockData),
    Contact(ContactBlockData),
    ProductGrid(ProductGridBlockData),
    Newsletter(NewsletterBlockData),
    Video(VideoBlockData),
    Html(HtmlBlockData),
    Spacer(SpacerBlockData),
}

impl BlockPayload {
    pub fn from_block_type(block_type: &BlockType, data: Value) -> Result<Self, serde_json::Error> {
        Ok(match block_type {
            BlockType::Hero => Self::Hero(serde_json::from_value(data)?),
            BlockType::Text => Self::Text(serde_json::from_value(data)?),
            BlockType::Image => Self::Image(serde_json::from_value(data)?),
            BlockType::Gallery => Self::Gallery(serde_json::from_value(data)?),
            BlockType::Cta => Self::Cta(serde_json::from_value(data)?),
            BlockType::Features => Self::Features(serde_json::from_value(data)?),
            BlockType::Testimonials => Self::Testimonials(serde_json::from_value(data)?),
            BlockType::Pricing => Self::Pricing(serde_json::from_value(data)?),
            BlockType::Faq => Self::Faq(serde_json::from_value(data)?),
            BlockType::Contact => Self::Contact(serde_json::from_value(data)?),
            BlockType::ProductGrid => Self::ProductGrid(serde_json::from_value(data)?),
            BlockType::Newsletter => Self::Newsletter(serde_json::from_value(data)?),
            BlockType::Video => Self::Video(serde_json::from_value(data)?),
            BlockType::Html => Self::Html(serde_json::from_value(data)?),
            BlockType::Spacer => Self::Spacer(serde_json::from_value(data)?),
        })
    }

    pub fn into_value(self) -> Result<Value, serde_json::Error> {
        match self {
            Self::Hero(v) => serde_json::to_value(v),
            Self::Text(v) => serde_json::to_value(v),
            Self::Image(v) => serde_json::to_value(v),
            Self::Gallery(v) => serde_json::to_value(v),
            Self::Cta(v) => serde_json::to_value(v),
            Self::Features(v) => serde_json::to_value(v),
            Self::Testimonials(v) => serde_json::to_value(v),
            Self::Pricing(v) => serde_json::to_value(v),
            Self::Faq(v) => serde_json::to_value(v),
            Self::Contact(v) => serde_json::to_value(v),
            Self::ProductGrid(v) => serde_json::to_value(v),
            Self::Newsletter(v) => serde_json::to_value(v),
            Self::Video(v) => serde_json::to_value(v),
            Self::Html(v) => serde_json::to_value(v),
            Self::Spacer(v) => serde_json::to_value(v),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateBlockInput {
    pub block_type: BlockType,
    pub position: i32,
    pub data: serde_json::Value,
    pub translations: Option<Vec<BlockTranslationInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockTranslationInput {
    pub locale: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateBlockInput {
    pub position: Option<i32>,
    pub data: Option<serde_json::Value>,
    pub translations: Option<Vec<BlockTranslationInput>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BlockResponse {
    pub id: Uuid,
    pub block_type: BlockType,
    pub position: i32,
    pub data: serde_json::Value,
    pub translations: Option<Vec<BlockTranslationInput>>,
}
