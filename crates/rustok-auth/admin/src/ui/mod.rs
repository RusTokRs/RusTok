mod auth_admin;
pub mod components;
pub mod leptos;
mod login;
mod oauth_apps;
mod profile;
mod register;
mod reset;
mod security;
mod user_details;
mod users;

pub use leptos::{
    AuthAdmin, Login, OAuthAppsPage, Profile, Register, ResetPassword, Security, UserDetails, Users,
};
