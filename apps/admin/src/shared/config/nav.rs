#[derive(Clone, Copy)]
pub struct NavSection {
    pub label: &'static str,
    pub items: &'static [NavItem],
}

#[derive(Clone, Copy)]
pub struct NavItem {
    pub label_key: &'static str,
    pub href: &'static str,
    pub icon: &'static str,
}

pub const NAV_SECTIONS: &[NavSection] = &[
    NavSection {
        label: "Overview",
        items: &[NavItem {
            label_key: "app.nav.dashboard",
            href: "/dashboard",
            icon: "grid",
        }],
    },
    NavSection {
        label: "Management",
        items: &[NavItem {
            label_key: "app.nav.users",
            href: "/users",
            icon: "users",
        }],
    },
    NavSection {
        label: "Account",
        items: &[
            NavItem {
                label_key: "app.nav.profile",
                href: "/profile",
                icon: "user",
            },
            NavItem {
                label_key: "app.nav.security",
                href: "/security",
                icon: "lock",
            },
        ],
    },
];
