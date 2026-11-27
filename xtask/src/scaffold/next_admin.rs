use std::{fs, path::Path};

use anyhow::{ensure, Context, Result};

use super::{options::ScaffoldOptions, PlannedFile};

pub(super) const HOST_CONFIG: &str = "apps/next-admin/tsconfig.json";
pub(super) const HOST_REGISTRY: &str = "apps/next-admin/src/modules/index.ts";
pub(super) const CATALOGS: [&str; 2] = [
    "apps/next-admin/messages/en.ftl",
    "apps/next-admin/messages/ru.ftl",
];

pub(super) fn render(
    root: &Path,
    module_dir: &Path,
    opts: &ScaffoldOptions,
    stem: &str,
    owner_files: &[PlannedFile],
) -> Result<Vec<PlannedFile>> {
    let route = opts.kebab_name();
    let field = super::options::graphql_field_prefix(stem);
    let package_root = root.join("apps/next-admin/packages").join(&route);
    let source = |relative: &str| -> Result<&str> {
        let path = module_dir.join(relative);
        owner_files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.content.as_str())
            .with_context(|| format!("Missing generated admin source {}", path.display()))
    };
    let fragment = source("contracts/graphql/admin_record_fields.graphql")?;
    let mut documents = String::from("// Built from the owner's GraphQL operation documents.\n");
    for (name, filename) in [
        ("LIST", "admin_records"),
        ("GET", "admin_record"),
        ("CREATE", "create_admin_record"),
        ("SAVE", "save_admin_record_title"),
        ("PUBLISH", "set_admin_record_publication"),
    ] {
        let document = source(&format!("contracts/graphql/{filename}.graphql"))?;
        let value = serde_json::to_string(&format!("{document}\n{fragment}"))?;
        documents.push_str(&format!("export const {name} = {value};\n"));
    }
    for (name, filename) in [
        ("SETTINGS", "tenant_module_settings"),
        ("SAVE_SETTINGS", "update_module_settings"),
    ] {
        let document = source(&format!("admin/contracts/graphql/{filename}.graphql"))?;
        documents.push_str(&format!(
            "export const {name} = {};\n",
            serde_json::to_string(document)?
        ));
    }

    let mut files = [
        ("package.json", include_str!("next_admin/package.json.in")),
        ("README.md", include_str!("next_admin/README.md.in")),
        ("src/index.ts", include_str!("next_admin/index.ts.in")),
        ("src/model.ts", include_str!("next_admin/model.ts.in")),
        (
            "src/transport.ts",
            include_str!("next_admin/transport.ts.in"),
        ),
        (
            "src/pages/records.tsx",
            include_str!("next_admin/records.tsx.in"),
        ),
        (
            "src/components/record-editor.tsx",
            include_str!("next_admin/editor.tsx.in"),
        ),
        (
            "src/components/record-table.tsx",
            include_str!("next_admin/table.tsx.in"),
        ),
        (
            "src/pages/settings.tsx",
            include_str!("next_admin/settings.tsx.in"),
        ),
        (
            "src/components/settings-form.tsx",
            include_str!("next_admin/settings_form.tsx.in"),
        ),
    ]
    .into_iter()
    .map(|(path, content)| PlannedFile {
        path: package_root.join(path),
        original: None,
        content: content
            .replace("__SLUG__", &opts.slug)
            .replace("__ROUTE__", &route)
            .replace("__MODULE_FIELD__", &field)
            .replace(
                "__DEFAULT_PAGE_SIZE__",
                &super::persistence::DEFAULT_PAGE_SIZE.to_string(),
            )
            .replace(
                "__MAX_PAGE_SIZE__",
                &super::persistence::MAX_PAGE_SIZE.to_string(),
            )
            .replace("__PAGE_SIZE_KEY__", super::persistence::PAGE_SIZE_KEY)
            .replace(
                "__PUBLIC_DISCOVERY_KEY__",
                super::persistence::PUBLIC_DISCOVERY_KEY,
            ),
    })
    .collect::<Vec<_>>();

    files.push(PlannedFile {
        path: package_root.join("src/operations.ts"),
        original: None,
        content: documents,
    });
    let english_catalog = source("admin/locales/en.ftl")?;
    let keys = |catalog: &str| -> Vec<String> {
        catalog
            .lines()
            .filter_map(|line| line.split_once(" = "))
            .map(|(key, _)| key.to_owned())
            .collect()
    };
    let english_keys = keys(english_catalog);
    let message_keys = english_keys
        .iter()
        .map(|key| format!("  | {key:?}"))
        .collect::<Vec<_>>()
        .join("\n");
    files.push(PlannedFile {
        path: package_root.join("src/messages.ts"),
        original: None,
        content: format!("export type MessageKey =\n{message_keys};\n\nexport type MessageArgs = Record<MessageKey, Record<string, never>>;\n"),
    });

    for (locale, target) in ["en", "ru"].into_iter().zip(CATALOGS) {
        let catalog = source(&format!("admin/locales/{locale}.ftl"))?;
        ensure!(
            keys(catalog) == english_keys,
            "Generated admin catalog {locale} keys differ from English"
        );
        files.push(edit_host_file(root, target, |original| {
            let mut messages = format!("\n# {route}\n");
            for line in catalog.lines() {
                if let Some((key, _)) = line.split_once(" = ") {
                    let id = format!("{route}-{key}");
                    check_new_message(original, locale, &id)?;
                    messages.push_str(&format!("{route}-{line}\n"));
                } else {
                    messages.push_str(line);
                    messages.push('\n');
                }
            }
            if opts.mounts_record_admin() {
                for key in [
                    "records-heading",
                    "settings-heading",
                    "settings-records-nav",
                ] {
                    let id = format!("app-nav-{route}-{key}");
                    check_new_message(original, locale, &id)?;
                    messages.push_str(&format!("{id} = {{ {route}-{key} }}\n"));
                }
            }
            Ok(format!(
                "{original}{}",
                match_line_endings(original, &messages)
            ))
        })?);
    }
    if opts.mounts_record_admin() {
        files.extend(render_mount(root, &package_root, opts, &route)?);
    }
    Ok(files)
}

fn check_new_message(original: &str, locale: &str, id: &str) -> Result<()> {
    ensure!(
        !original.lines().any(|line| line
            .split_once('=')
            .is_some_and(|(key, _)| key.trim_end() == id)),
        "Next Admin locale {locale} already contains {id}"
    );
    Ok(())
}

fn edit_host_file(
    root: &Path,
    relative: &str,
    edit: impl FnOnce(&str) -> Result<String>,
) -> Result<PlannedFile> {
    let path = root.join(relative);
    super::ensure_no_symlink_ancestors(root, &path)?;
    let original = fs::read_to_string(&path)
        .with_context(|| format!("Cannot read Next Admin registration {}", path.display()))?;
    let content = edit(&original)?;
    Ok(PlannedFile {
        path,
        content,
        original: Some(original.into_bytes()),
    })
}

fn match_line_endings(original: &str, addition: &str) -> String {
    if original.contains("\r\n") {
        addition.replace('\n', "\r\n")
    } else {
        addition.to_owned()
    }
}

fn render_mount(
    root: &Path,
    package_root: &Path,
    opts: &ScaffoldOptions,
    route: &str,
) -> Result<Vec<PlannedFile>> {
    let package = format!("@rustok/{route}-admin");
    let package_register = format!("{package}/register");
    let routes = root.join("apps/next-admin/src/app/dashboard").join(route);
    let title = serde_json::to_string(&opts.title)?;
    let mut files = [
        (
            package_root.join("src/register.ts"),
            include_str!("next_admin/register.ts.in"),
        ),
        (
            routes.join("layout.tsx"),
            include_str!("next_admin/layout.tsx.in"),
        ),
        (
            routes.join("page.tsx"),
            include_str!("next_admin/record_route.tsx.in"),
        ),
        (
            routes.join("records/page.tsx"),
            "export { default } from '../page';\n",
        ),
        (
            routes.join("settings/page.tsx"),
            include_str!("next_admin/settings_route.tsx.in"),
        ),
        (
            routes.join("settings/records/page.tsx"),
            include_str!("next_admin/settings_route.tsx.in"),
        ),
    ]
    .into_iter()
    .map(|(path, template)| {
        let show_records = path.ends_with("settings/records/page.tsx");
        PlannedFile {
            path,
            original: None,
            content: template
                .replace("__SLUG__", &opts.slug)
                .replace("__ROUTE__", route)
                .replace(
                    "__SHOW_RECORD_SETTINGS__",
                    if show_records { "true" } else { "false" },
                )
                .replace("__TITLE__", &title),
        }
    })
    .collect::<Vec<_>>();

    files.push(edit_host_file(root, HOST_REGISTRY, |original| {
        let import = format!("import '{package_register}';");
        let import_double_quotes = format!("import \"{package_register}\";");
        ensure!(
            !original.lines().any(|line| {
                let line = line.trim();
                line == import || line == import_double_quotes
            }),
            "Next Admin package is already registered: {package}"
        );
        let anchor = "\n\nexport type {";
        let anchor = match_line_endings(original, anchor);
        let (before, after) = original
            .split_once(&anchor)
            .context("Next Admin module registry export block is missing")?;
        Ok(format!(
            "{before}{}{anchor}{after}",
            match_line_endings(original, &format!("\n{import}"))
        ))
    })?);
    files.push(edit_host_file(root, HOST_CONFIG, |original| {
        let config: serde_json::Value = serde_json::from_str(original)?;
        let paths = config.pointer("/compilerOptions/paths").and_then(serde_json::Value::as_object)
            .context("Next Admin TypeScript configuration must define compilerOptions.paths")?;
        ensure!(!paths.contains_key(&package) && !paths.contains_key(&package_register), "Next Admin TypeScript aliases already exist: {package}");
        let anchor = "\"paths\": {";
        let (before, after) = original.split_once(anchor).context("Next Admin TypeScript aliases anchor is missing")?;
        let aliases = format!("\n      \"{package}\": [\"./packages/{route}/src/index.ts\"],\n      \"{package_register}\": [\"./packages/{route}/src/register.ts\"],");
        let content = format!("{before}{anchor}{}{after}", match_line_endings(original, &aliases));
        let updated: serde_json::Value = serde_json::from_str(&content)?;
        ensure!(updated["compilerOptions"]["paths"][&package_register][0] == format!("./packages/{route}/src/register.ts"), "Next Admin aliases must be inside compilerOptions.paths");
        Ok(content)
    })?);
    Ok(files)
}
