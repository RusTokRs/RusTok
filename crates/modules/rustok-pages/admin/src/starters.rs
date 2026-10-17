use fly::{
    DocumentKind, GrapesJsCodec, ProjectDocument, UniqueIdGenerator,
};
use rustok_ui_core::normalize_ui_text;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageStarterDescriptor {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub template_key: String,
    pub preview_badge: String,
    pub icon: String,
}

pub fn page_starters() -> Vec<PageStarterDescriptor> {
    vec![
        PageStarterDescriptor {
            id: "blank".to_string(),
            title: "Start Blank".to_string(),
            description: "Start with an empty canvas and build custom sections from scratch.".to_string(),
            category: "general".to_string(),
            template_key: "default".to_string(),
            preview_badge: "Minimal".to_string(),
            icon: "file-text".to_string(),
        },
        PageStarterDescriptor {
            id: "saas_landing".to_string(),
            title: "SaaS Landing Page".to_string(),
            description: "Complete modern product landing with hero, features grid, pricing table, FAQ accordion, and CTA.".to_string(),
            category: "landing".to_string(),
            template_key: "default".to_string(),
            preview_badge: "Popular".to_string(),
            icon: "rocket".to_string(),
        },
        PageStarterDescriptor {
            id: "ecommerce_showcase".to_string(),
            title: "Product Showcase".to_string(),
            description: "High-impact product launch page with spotlight columns, testimonials, and buy action.".to_string(),
            category: "ecommerce".to_string(),
            template_key: "default".to_string(),
            preview_badge: "Store".to_string(),
            icon: "shopping-bag".to_string(),
        },
        PageStarterDescriptor {
            id: "lead_capture".to_string(),
            title: "Lead Generation & Webinar".to_string(),
            description: "Focused conversion landing page with value proposition, contact capture form, and social proof.".to_string(),
            category: "lead_generation".to_string(),
            template_key: "default".to_string(),
            preview_badge: "Lead Gen".to_string(),
            icon: "user-check".to_string(),
        },
        PageStarterDescriptor {
            id: "link_in_bio".to_string(),
            title: "Link in Bio Hub".to_string(),
            description: "Mobile-first creator or brand hub with avatar, biography, and interactive link action cards.".to_string(),
            category: "personal".to_string(),
            template_key: "default".to_string(),
            preview_badge: "Mobile".to_string(),
            icon: "link".to_string(),
        },
        PageStarterDescriptor {
            id: "company_contact".to_string(),
            title: "Company & Contact".to_string(),
            description: "Corporate company overview with contact form, mission statement, and FAQ accordion.".to_string(),
            category: "company".to_string(),
            template_key: "default".to_string(),
            preview_badge: "Corporate".to_string(),
            icon: "building".to_string(),
        },
    ]
}

pub fn build_starter_preset(starter_id: &str, title: &str) -> ProjectDocument {
    let normalized = normalize_ui_text(title);
    let title = normalized.as_deref().unwrap_or("New Page");

    let mut doc = match starter_id {
        "saas_landing" => build_saas_landing_doc(title),
        "ecommerce_showcase" => build_ecommerce_showcase_doc(title),
        "lead_capture" => build_lead_capture_doc(title),
        "link_in_bio" => build_link_in_bio_doc(title),
        "company_contact" => build_company_contact_doc(title),
        _ => build_blank_doc(title),
    };

    doc.set_document_kind(DocumentKind::StarterPreset);
    doc
}

pub fn instantiate_starter_preset(starter_id: &str, title: &str) -> Result<Value, String> {
    let preset = build_starter_preset(starter_id, title);
    let mut generator = UniqueIdGenerator::new("page");
    let page_doc = preset.clone_as_page(&mut generator);
    GrapesJsCodec::encode_value(&page_doc).map_err(|err| err.to_string())
}

pub fn instantiate_starter_preset_pretty(starter_id: &str, title: &str) -> Result<String, String> {
    let value = instantiate_starter_preset(starter_id, title)?;
    serde_json::to_string_pretty(&value).map_err(|err| err.to_string())
}

fn wrap_page_document(title: &str, components: Vec<Value>) -> ProjectDocument {
    let val = json!({
        "assets": [],
        "styles": [],
        "pages": [
            {
                "id": "main",
                "name": title,
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "components": components
                }
            }
        ]
    });
    GrapesJsCodec::decode_value(val).expect("page document must decode")
}

fn build_blank_doc(title: &str) -> ProjectDocument {
    wrap_page_document(
        title,
        vec![
            json!({
                "id": "heading",
                "type": "text",
                "content": format!("<h1>{}</h1>", escape_html(title))
            }),
            json!({
                "id": "intro",
                "type": "text",
                "content": "<p>Build this page with Fly.</p>"
            }),
        ],
    )
}

fn build_saas_landing_doc(title: &str) -> ProjectDocument {
    let hero = json!({
        "id": "sec-hero",
        "type": "section",
        "style": { "padding": "96px 24px", "background": "#f8fafc", "text-align": "center" },
        "components": [{
            "id": "cont-hero",
            "type": "container",
            "style": { "max-width": "960px", "margin": "0 auto" },
            "components": [
                { "id": "hero-headline", "type": "heading", "tagName": "h1", "content": escape_html(title), "style": { "font-size": "56px", "line-height": "1.1", "margin": "0 0 20px", "color": "#0f172a" } },
                { "id": "hero-subhead", "type": "text", "content": "The unified platform for high-performance websites and digital commerce.", "style": { "font-size": "20px", "line-height": "1.6", "color": "#475569", "margin": "0 auto 32px", "max-width": "720px" } },
                { "id": "hero-cta", "type": "button", "tagName": "a", "content": "Get Started Free", "style": { "display": "inline-block", "padding": "14px 32px", "background": "#2563eb", "color": "#ffffff", "border-radius": "10px", "font-weight": "600", "text-decoration": "none" } }
            ]
        }]
    });

    let features = json!({
        "id": "sec-features",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#ffffff" },
        "components": [{
            "id": "cont-features",
            "type": "container",
            "style": { "max-width": "1120px", "margin": "0 auto" },
            "components": [
                { "id": "feat-head", "type": "heading", "content": "Engineered for speed and flexibility", "style": { "text-align": "center", "font-size": "36px", "margin": "0 0 48px", "color": "#0f172a" } },
                {
                    "id": "feat-grid",
                    "type": "grid",
                    "style": { "display": "grid", "grid-template-columns": "repeat(auto-fit, minmax(280px, 1fr))", "gap": "28px" },
                    "components": [
                        feature_card("card-1", "Lightning Fast SSR", "Compiled Rust components and headless edge rendering delivering sub-10ms TTFB."),
                        feature_card("card-2", "Fluid Architecture", "Zero domain logic in UI adapters: robust, testable, and maintainable across frameworks."),
                        feature_card("card-3", "Enterprise Security", "Tenant isolation, role-based access control, and cryptographic tamper-evident audits.")
                    ]
                }
            ]
        }]
    });

    let pricing = json!({
        "id": "sec-pricing",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#f8fafc" },
        "components": [{
            "id": "cont-pricing",
            "type": "container",
            "style": { "max-width": "1120px", "margin": "0 auto" },
            "components": [
                { "id": "price-head", "type": "heading", "content": "Transparent Pricing", "style": { "text-align": "center", "font-size": "36px", "margin": "0 0 12px", "color": "#0f172a" } },
                { "id": "price-sub", "type": "text", "content": "Scale effortlessly from your first launch to millions of visitors.", "style": { "text-align": "center", "color": "#64748b", "margin": "0 0 48px" } },
                pricing_table_node("pricing-table")
            ]
        }]
    });

    let faq = json!({
        "id": "sec-faq",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#ffffff" },
        "components": [{
            "id": "cont-faq",
            "type": "container",
            "style": { "max-width": "800px", "margin": "0 auto" },
            "components": [
                { "id": "faq-head", "type": "heading", "content": "Frequently Asked Questions", "style": { "text-align": "center", "font-size": "36px", "margin": "0 0 40px", "color": "#0f172a" } },
                faq_accordion_node("faq-accordion")
            ]
        }]
    });

    let cta = json!({
        "id": "sec-cta",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#2563eb", "color": "#ffffff", "text-align": "center" },
        "components": [{
            "id": "cont-cta",
            "type": "container",
            "style": { "max-width": "800px", "margin": "0 auto" },
            "components": [
                { "id": "cta-head", "type": "heading", "content": "Ready to transform your digital experience?", "style": { "font-size": "40px", "margin": "0 0 16px", "color": "#ffffff" } },
                { "id": "cta-sub", "type": "text", "content": "Join hundreds of fast-growing teams building with RusToK today.", "style": { "font-size": "18px", "color": "#bfdbfe", "margin": "0 0 32px" } },
                { "id": "cta-btn", "type": "button", "tagName": "a", "content": "Start Building Today", "style": { "display": "inline-block", "padding": "16px 36px", "background": "#ffffff", "color": "#2563eb", "border-radius": "10px", "font-weight": "700", "text-decoration": "none" } }
            ]
        }]
    });

    wrap_page_document(title, vec![hero, features, pricing, faq, cta])
}

fn build_ecommerce_showcase_doc(title: &str) -> ProjectDocument {
    let hero = json!({
        "id": "sec-eco-hero",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#f8fafc" },
        "components": [{
            "id": "cont-eco-hero",
            "type": "container",
            "style": { "max-width": "1120px", "margin": "0 auto" },
            "components": [{
                "id": "row-eco-hero",
                "type": "row",
                "style": { "display": "flex", "gap": "48px", "align-items": "center", "flex-wrap": "wrap" },
                "components": [
                    {
                        "id": "col-eco-text",
                        "type": "column",
                        "style": { "flex": "1 1 480px" },
                        "components": [
                            { "id": "eco-badge", "type": "text", "tagName": "span", "content": "NEW RELEASE", "style": { "display": "inline-block", "padding": "4px 12px", "background": "#dbeafe", "color": "#1d4ed8", "border-radius": "9999px", "font-size": "12px", "font-weight": "700", "margin-bottom": "16px" } },
                            { "id": "eco-head", "type": "heading", "tagName": "h1", "content": escape_html(title), "style": { "font-size": "48px", "line-height": "1.15", "margin": "0 0 16px", "color": "#0f172a" } },
                            { "id": "eco-body", "type": "text", "content": "Designed with precision craftsmanship and premium sustainable materials.", "style": { "font-size": "18px", "line-height": "1.6", "color": "#475569", "margin": "0 0 32px" } },
                            { "id": "eco-buy", "type": "button", "tagName": "a", "content": "Shop Collection — $129", "style": { "display": "inline-block", "padding": "14px 32px", "background": "#0f172a", "color": "#ffffff", "border-radius": "10px", "font-weight": "600", "text-decoration": "none" } }
                        ]
                    },
                    {
                        "id": "col-eco-media",
                        "type": "column",
                        "style": { "flex": "1 1 480px" },
                        "components": [
                            { "id": "eco-img", "type": "image", "tagName": "img", "attributes": { "src": "/assets/product-showcase.webp", "alt": "Product showcase preview" }, "style": { "width": "100%", "border-radius": "16px", "box-shadow": "0 10px 25px -5px rgba(0,0,0,0.1)" } }
                        ]
                    }
                ]
            }]
        }]
    });

    let testimonials = json!({
        "id": "sec-testimonials",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#ffffff" },
        "components": [{
            "id": "cont-testimonials",
            "type": "container",
            "style": { "max-width": "1120px", "margin": "0 auto" },
            "components": [
                { "id": "test-head", "type": "heading", "content": "Loved by over 10,000 customers", "style": { "text-align": "center", "font-size": "36px", "margin": "0 0 48px", "color": "#0f172a" } },
                {
                    "id": "test-grid",
                    "type": "grid",
                    "style": { "display": "grid", "grid-template-columns": "repeat(auto-fit, minmax(300px, 1fr))", "gap": "24px" },
                    "components": [
                        testimonial_card("test-1", "Absolute game changer for our daily workflow. The build quality is unmatched.", "Alex Rivera", "Creative Director"),
                        testimonial_card("test-2", "Speed, simplicity, and attention to detail. Could not recommend more highly.", "Elena Rostov", "Lead Architect")
                    ]
                }
            ]
        }]
    });

    wrap_page_document(title, vec![hero, testimonials])
}

fn build_lead_capture_doc(title: &str) -> ProjectDocument {
    let hero = json!({
        "id": "sec-lead-hero",
        "type": "section",
        "style": { "padding": "96px 24px", "background": "#0f172a", "color": "#ffffff", "text-align": "center" },
        "components": [{
            "id": "cont-lead-hero",
            "type": "container",
            "style": { "max-width": "800px", "margin": "0 auto" },
            "components": [
                { "id": "lead-badge", "type": "text", "tagName": "span", "content": "EXCLUSIVE MASTERCLASS", "style": { "display": "inline-block", "padding": "4px 14px", "background": "#1e293b", "color": "#38bdf8", "border-radius": "9999px", "font-size": "13px", "font-weight": "700", "margin-bottom": "20px" } },
                { "id": "lead-head", "type": "heading", "tagName": "h1", "content": escape_html(title), "style": { "font-size": "48px", "line-height": "1.15", "margin": "0 0 20px", "color": "#ffffff" } },
                { "id": "lead-sub", "type": "text", "content": "Reserve your spot to discover how leading engineering teams scale multi-tenant architectures without technical debt.", "style": { "font-size": "18px", "line-height": "1.6", "color": "#94a3b8", "margin": "0 auto 36px", "max-width": "640px" } }
            ]
        }]
    });

    let form_section = json!({
        "id": "sec-lead-form",
        "type": "section",
        "style": { "padding": "64px 24px", "background": "#f8fafc" },
        "components": [{
            "id": "cont-lead-form",
            "type": "container",
            "style": { "max-width": "600px", "margin": "0 auto", "padding": "40px", "background": "#ffffff", "border-radius": "16px", "box-shadow": "0 10px 25px -5px rgba(0,0,0,0.06)", "border": "1px solid #e2e8f0" },
            "components": [
                { "id": "form-title", "type": "heading", "content": "Register for Access", "style": { "font-size": "24px", "margin": "0 0 8px", "color": "#0f172a" } },
                { "id": "form-desc", "type": "text", "content": "Instant invitation sent to your corporate inbox.", "style": { "font-size": "14px", "color": "#64748b", "margin": "0 0 24px" } },
                {
                    "id": "capture-form",
                    "type": "form",
                    "tagName": "form",
                    "style": { "display": "flex", "flex-direction": "column", "gap": "16px" },
                    "components": [
                        { "id": "f-name", "type": "input", "tagName": "input", "attributes": { "type": "text", "placeholder": "Full Name", "required": "true" }, "style": { "padding": "12px 16px", "border": "1px solid #cbd5e1", "border-radius": "8px", "font-size": "15px" } },
                        { "id": "f-email", "type": "input", "tagName": "input", "attributes": { "type": "email", "placeholder": "Work Email", "required": "true" }, "style": { "padding": "12px 16px", "border": "1px solid #cbd5e1", "border-radius": "8px", "font-size": "15px" } },
                        { "id": "f-sub", "type": "submit", "tagName": "button", "content": "Claim Free Seat", "style": { "padding": "14px 24px", "background": "#2563eb", "color": "#ffffff", "border": "0", "border-radius": "8px", "font-size": "16px", "font-weight": "600", "cursor": "pointer" } }
                    ]
                }
            ]
        }]
    });

    wrap_page_document(title, vec![hero, form_section])
}

fn build_link_in_bio_doc(title: &str) -> ProjectDocument {
    let container = json!({
        "id": "bio-container",
        "type": "container",
        "style": { "max-width": "460px", "margin": "0 auto", "text-align": "center" },
        "components": [
            {
                "id": "bio-avatar",
                "type": "image",
                "tagName": "img",
                "attributes": {
                    "src": "/assets/avatar.webp",
                    "alt": "Profile avatar"
                },
                "style": { "width": "96px", "height": "96px", "border-radius": "9999px", "margin": "0 auto 20px", "border": "3px solid #38bdf8" }
            },
            { "id": "bio-title", "type": "heading", "tagName": "h1", "content": escape_html(title), "style": { "font-size": "26px", "color": "#ffffff", "margin": "0 0 8px" } },
            { "id": "bio-desc", "type": "text", "content": "Software architect, systems engineer & digital craftsman.", "style": { "font-size": "15px", "color": "#94a3b8", "margin": "0 0 32px", "line-height": "1.5" } },
            {
                "id": "bio-links",
                "type": "container",
                "style": { "display": "flex", "flex-direction": "column", "gap": "14px" },
                "components": [
                    link_card_button("link-1", "🚀 Latest Project Launch", "https://example.com/project"),
                    link_card_button("link-2", "📖 Engineering Architecture Blog", "https://example.com/blog"),
                    link_card_button("link-3", "🎙️ Systems & Rust Podcast", "https://example.com/podcast"),
                    link_card_button("link-4", "✉️ Contact & Inquiries", "mailto:hello@example.com")
                ]
            }
        ]
    });

    let val = json!({
        "assets": [],
        "styles": [],
        "pages": [
            {
                "id": "main",
                "name": title,
                "component": {
                    "id": "root",
                    "type": "wrapper",
                    "style": { "background": "#0f172a", "min-height": "100vh", "padding": "64px 20px" },
                    "components": [container]
                }
            }
        ]
    });
    GrapesJsCodec::decode_value(val).expect("link in bio document is valid")
}

fn build_company_contact_doc(title: &str) -> ProjectDocument {
    let hero = json!({
        "id": "sec-corp-hero",
        "type": "section",
        "style": { "padding": "80px 24px", "background": "#f8fafc", "text-align": "center" },
        "components": [{
            "id": "cont-corp-hero",
            "type": "container",
            "style": { "max-width": "800px", "margin": "0 auto" },
            "components": [
                { "id": "corp-head", "type": "heading", "tagName": "h1", "content": escape_html(title), "style": { "font-size": "44px", "margin": "0 0 16px", "color": "#0f172a" } },
                { "id": "corp-sub", "type": "text", "content": "We partner with visionary enterprises worldwide. Reach out to discuss your technical challenges and custom deployments.", "style": { "font-size": "18px", "color": "#475569", "line-height": "1.6" } }
            ]
        }]
    });

    let content = json!({
        "id": "sec-corp-content",
        "type": "section",
        "style": { "padding": "72px 24px", "background": "#ffffff" },
        "components": [{
            "id": "cont-corp-content",
            "type": "container",
            "style": { "max-width": "1120px", "margin": "0 auto" },
            "components": [{
                "id": "row-corp",
                "type": "row",
                "style": { "display": "flex", "gap": "56px", "flex-wrap": "wrap" },
                "components": [
                    {
                        "id": "col-corp-info",
                        "type": "column",
                        "style": { "flex": "1 1 420px" },
                        "components": [
                            { "id": "info-h2", "type": "heading", "content": "Headquarters & Support", "style": { "font-size": "24px", "margin": "0 0 16px", "color": "#0f172a" } },
                            { "id": "info-p1", "type": "text", "content": "Global Technology Center\n100 Innovation Way\nSan Francisco, CA 94105", "style": { "line-height": "1.7", "color": "#64748b", "margin": "0 0 24px" } },
                            { "id": "info-p2", "type": "text", "content": "Email: enterprise@rustok.dev\nPhone: +1 (800) 555-0199", "style": { "line-height": "1.7", "color": "#64748b" } }
                        ]
                    },
                    {
                        "id": "col-corp-form",
                        "type": "column",
                        "style": { "flex": "1 1 480px" },
                        "components": [
                            {
                                "id": "corp-contact-form",
                                "type": "form",
                                "tagName": "form",
                                "style": { "display": "flex", "flex-direction": "column", "gap": "16px" },
                                "components": [
                                    { "id": "cf-name", "type": "input", "tagName": "input", "attributes": { "type": "text", "placeholder": "Your Name", "required": "true" }, "style": { "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" } },
                                    { "id": "cf-email", "type": "input", "tagName": "input", "attributes": { "type": "email", "placeholder": "Work Email", "required": "true" }, "style": { "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" } },
                                    { "id": "cf-msg", "type": "textarea", "tagName": "textarea", "attributes": { "placeholder": "How can we help your organization?", "rows": 4 }, "style": { "padding": "12px", "border": "1px solid #cbd5e1", "border-radius": "8px" } },
                                    { "id": "cf-btn", "type": "submit", "tagName": "button", "content": "Send Inquiry", "style": { "padding": "14px 28px", "background": "#2563eb", "color": "#ffffff", "border": "0", "border-radius": "8px", "font-weight": "600", "cursor": "pointer" } }
                                ]
                            }
                        ]
                    }
                ]
            }]
        }]
    });

    wrap_page_document(title, vec![hero, content])
}

fn feature_card(id: &str, title: &str, body: &str) -> Value {
    json!({
        "id": id,
        "type": "column",
        "style": { "padding": "28px", "border": "1px solid #e2e8f0", "border-radius": "16px", "background": "#ffffff" },
        "components": [
            { "id": format!("{id}-title"), "type": "heading", "content": title, "style": { "font-size": "20px", "margin": "0 0 10px", "color": "#0f172a" } },
            { "id": format!("{id}-body"), "type": "text", "content": body, "style": { "color": "#64748b", "line-height": "1.6", "font-size": "15px" } }
        ]
    })
}

fn testimonial_card(id: &str, quote: &str, author: &str, role: &str) -> Value {
    json!({
        "id": id,
        "type": "column",
        "style": { "padding": "28px", "border": "1px solid #e2e8f0", "border-radius": "16px", "background": "#f8fafc" },
        "components": [
            { "id": format!("{id}-quote"), "type": "text", "content": format!("“{}”", quote), "style": { "font-style": "italic", "color": "#334155", "line-height": "1.6", "margin": "0 0 16px" } },
            { "id": format!("{id}-author"), "type": "text", "content": format!("<strong>{}</strong> — {}", author, role), "style": { "font-size": "14px", "color": "#64748b" } }
        ]
    })
}

fn link_card_button(id: &str, label: &str, href: &str) -> Value {
    json!({
        "id": id,
        "type": "button",
        "tagName": "a",
        "content": label,
        "attributes": { "href": href, "target": "_blank", "rel": "noopener noreferrer" },
        "style": {
            "display": "block",
            "padding": "16px 20px",
            "background": "#1e293b",
            "color": "#ffffff",
            "border-radius": "12px",
            "font-weight": "600",
            "font-size": "15px",
            "text-decoration": "none",
            "transition": "all 0.2s ease",
            "border": "1px solid #334155"
        }
    })
}

fn pricing_table_node(id: &str) -> Value {
    json!({
        "id": id,
        "type": "pricing_table",
        "tagName": "div",
        "style": { "display": "grid", "grid-template-columns": "repeat(auto-fit, minmax(280px, 1fr))", "gap": "24px", "width": "100%" },
        "components": [
            {
                "id": format!("{id}-tier-1"),
                "type": "column",
                "tagName": "div",
                "style": { "padding": "32px", "border": "1px solid #e2e8f0", "border-radius": "16px", "background": "#ffffff" },
                "components": [
                    { "id": format!("{id}-t1-name"), "type": "heading", "content": "Starter", "style": { "font-size": "20px", "margin": "0 0 8px", "color": "#0f172a" } },
                    { "id": format!("{id}-t1-price"), "type": "text", "content": "$29 <span style='font-size:14px;color:#64748b;'>/ month</span>", "style": { "font-size": "32px", "font-weight": "700", "margin": "0 0 16px" } },
                    { "id": format!("{id}-t1-btn"), "type": "button", "tagName": "a", "content": "Choose Starter", "style": { "display": "block", "text-align": "center", "padding": "10px", "background": "#f1f5f9", "color": "#0f172a", "border-radius": "8px", "font-weight": "600", "text-decoration": "none" } }
                ]
            },
            {
                "id": format!("{id}-tier-2"),
                "type": "column",
                "tagName": "div",
                "style": { "padding": "32px", "border": "2px solid #2563eb", "border-radius": "16px", "background": "#ffffff", "position": "relative" },
                "components": [
                    { "id": format!("{id}-t2-name"), "type": "heading", "content": "Professional", "style": { "font-size": "20px", "margin": "0 0 8px", "color": "#2563eb" } },
                    { "id": format!("{id}-t2-price"), "type": "text", "content": "$99 <span style='font-size:14px;color:#64748b;'>/ month</span>", "style": { "font-size": "32px", "font-weight": "700", "margin": "0 0 16px" } },
                    { "id": format!("{id}-t2-btn"), "type": "button", "tagName": "a", "content": "Choose Pro", "style": { "display": "block", "text-align": "center", "padding": "10px", "background": "#2563eb", "color": "#ffffff", "border-radius": "8px", "font-weight": "600", "text-decoration": "none" } }
                ]
            }
        ]
    })
}

fn faq_accordion_node(id: &str) -> Value {
    json!({
        "id": id,
        "type": "accordion",
        "tagName": "div",
        "style": { "display": "flex", "flex-direction": "column", "gap": "12px", "width": "100%" },
        "components": [
            {
                "id": format!("{id}-item-1"),
                "type": "container",
                "tagName": "div",
                "style": { "border": "1px solid #e2e8f0", "border-radius": "12px", "overflow": "hidden" },
                "components": [
                    {
                        "id": format!("{id}-trigger-1"),
                        "type": "button",
                        "tagName": "button",
                        "content": "Can I customize the code and design?",
                        "attributes": { "type": "button", "aria-expanded": "true" },
                        "flyAction": { "kind": "toggle_accordion", "target_id": format!("{id}-body-1"), "exclusive": true },
                        "style": { "display": "flex", "width": "100%", "justify-content": "space-between", "padding": "16px 20px", "background": "#f8fafc", "border": "0", "font-size": "16px", "font-weight": "600", "cursor": "pointer", "text-align": "left" }
                    },
                    {
                        "id": format!("{id}-body-1"),
                        "type": "container",
                        "tagName": "div",
                        "attributes": { "data-fly-accordion-body": "true" },
                        "style": { "padding": "16px 20px", "background": "#ffffff", "border-top": "1px solid #f1f5f9", "color": "#475569", "font-size": "15px", "line-height": "1.6" },
                        "components": [
                            { "id": format!("{id}-p-1"), "type": "text", "content": "Yes, every component is fully interactive and customizable via visual canvas or direct code." }
                        ]
                    }
                ]
            },
            {
                "id": format!("{id}-item-2"),
                "type": "container",
                "tagName": "div",
                "style": { "border": "1px solid #e2e8f0", "border-radius": "12px", "overflow": "hidden" },
                "components": [
                    {
                        "id": format!("{id}-trigger-2"),
                        "type": "button",
                        "tagName": "button",
                        "content": "Is multi-tenant and multi-locale supported?",
                        "attributes": { "type": "button", "aria-expanded": "false" },
                        "flyAction": { "kind": "toggle_accordion", "target_id": format!("{id}-body-2"), "exclusive": true },
                        "style": { "display": "flex", "width": "100%", "justify-content": "space-between", "padding": "16px 20px", "background": "#f8fafc", "border": "0", "font-size": "16px", "font-weight": "600", "cursor": "pointer", "text-align": "left" }
                    },
                    {
                        "id": format!("{id}-body-2"),
                        "type": "container",
                        "tagName": "div",
                        "attributes": { "data-fly-accordion-body": "true" },
                        "style": { "display": "none", "padding": "16px 20px", "background": "#ffffff", "border-top": "1px solid #f1f5f9", "color": "#475569", "font-size": "15px", "line-height": "1.6" },
                        "components": [
                            { "id": format!("{id}-p-2"), "type": "text", "content": "Yes, RusToK natively isolates tenants and manages locale-keyed translations and landing revisions." }
                        ]
                    }
                ]
            }
        ]
    })
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use fly::{RegistrySet, ValidationLimits, validate_project};
    use std::collections::HashSet;

    #[test]
    fn starter_catalog_has_all_canonical_presets() {
        let starters = page_starters();
        assert_eq!(starters.len(), 6);
        let ids: Vec<&str> = starters.iter().map(|s| s.id.as_str()).collect();
        assert!(ids.contains(&"blank"));
        assert!(ids.contains(&"saas_landing"));
        assert!(ids.contains(&"ecommerce_showcase"));
        assert!(ids.contains(&"lead_capture"));
        assert!(ids.contains(&"link_in_bio"));
        assert!(ids.contains(&"company_contact"));
    }

    #[test]
    fn each_starter_preset_validates_as_valid_fly_document() {
        let registries = RegistrySet::with_builtins();
        for starter in page_starters() {
            let doc = build_starter_preset(&starter.id, "Test Title");
            assert_eq!(doc.document_kind(), DocumentKind::StarterPreset);
            let report = validate_project(&doc, &registries, ValidationLimits::default());
            assert!(
                report.is_valid(),
                "Starter preset `{}` failed validation: {:?}",
                starter.id,
                report.diagnostics
            );
        }
    }

    #[test]
    fn instantiate_starter_preset_sets_page_kind_and_mints_fresh_unique_ids() {
        let value1 = instantiate_starter_preset("saas_landing", "My SaaS").unwrap();
        let doc1 = GrapesJsCodec::decode_value(value1).expect("instantiated doc must decode");

        assert_eq!(doc1.document_kind(), DocumentKind::Page);

        let mut ids1 = HashSet::new();
        doc1.project.visit_components(|c, _, _| {
            if let Some(id) = c.id.as_ref() {
                assert!(ids1.insert(id.clone()), "Duplicate id {id} in instantiated page");
                assert!(!id.starts_with("sec-"), "Preset ID {id} was not remapped");
            }
        });

        let value2 = instantiate_starter_preset("saas_landing", "My SaaS").unwrap();
        let doc2 = GrapesJsCodec::decode_value(value2).expect("second instantiated doc must decode");

        let mut ids2 = HashSet::new();
        doc2.project.visit_components(|c, _, _| {
            if let Some(id) = c.id.as_ref() {
                ids2.insert(id.clone());
            }
        });

        // Intersection between two separate instantiations must be completely empty!
        let shared: Vec<_> = ids1.intersection(&ids2).collect();
        assert!(
            shared.is_empty(),
            "Instantiations must share zero IDs, but found shared: {shared:?}"
        );
    }

    #[test]
    fn instantiate_starter_preset_contains_no_page_content_slot() {
        let registries = RegistrySet::with_builtins();
        for starter in page_starters() {
            let val = instantiate_starter_preset(&starter.id, "Demo").unwrap();
            let doc = GrapesJsCodec::decode_value(val).unwrap();
            let report = validate_project(&doc, &registries, ValidationLimits::default());
            assert!(
                report.is_valid(),
                "Instantiated page `{}` failed validation: {:?}",
                starter.id,
                report.diagnostics
            );
        }
    }
}
