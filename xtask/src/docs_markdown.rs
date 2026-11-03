use anyhow::{Context, Result};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag};
use std::ops::Range;

pub(crate) const MODULE_COLUMNS: &[&str] = &["Slug", "Crate", "Dependencies", "Role"];
pub(crate) const CORE_MODULES: Section = Section(HeadingLevel::H3, "Core Modules");
pub(crate) const OPTIONAL_MODULES: Section = Section(HeadingLevel::H3, "Optional Modules");

#[derive(Clone, Copy)]
pub(crate) struct Section(pub(crate) HeadingLevel, pub(crate) &'static str);

#[derive(Default)]
pub(crate) struct Inline {
    pub(crate) text: String,
    pub(crate) code: Vec<String>,
    pub(crate) links: Vec<String>,
}

impl Inline {
    fn read<'a>(events: impl IntoIterator<Item = OffsetEvent<'a>>) -> Self {
        let mut inline = Self::default();
        for (event, _) in events {
            match event {
                Event::Text(text) => inline.text.push_str(&text),
                Event::Code(code) => {
                    inline.text.push_str(&code);
                    inline.code.push(code.into_string());
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    inline.links.push(dest_url.into_string());
                }
                Event::SoftBreak | Event::HardBreak => inline.text.push(' '),
                _ => {}
            }
        }
        inline
    }
}

pub(crate) struct Row {
    pub(crate) span: Range<usize>,
    pub(crate) cells: Vec<Inline>,
}

pub(crate) struct Table {
    pub(crate) span: Range<usize>,
    pub(crate) columns: Vec<Inline>,
    pub(crate) rows: Vec<Row>,
}

impl Table {
    pub(crate) fn has_columns(&self, columns: &[&str]) -> bool {
        self.columns
            .iter()
            .map(|cell| cell.text.trim())
            .eq(columns.iter().copied())
    }
}

pub(crate) struct ListItem {
    pub(crate) span: Range<usize>,
    pub(crate) inline: Inline,
}

#[derive(Default)]
pub(crate) struct Document {
    headings: Vec<(HeadingLevel, String, Range<usize>)>,
    pub(crate) tables: Vec<Table>,
    pub(crate) items: Vec<ListItem>,
    pub(crate) links: Vec<String>,
    pub(crate) code: Vec<String>,
    len: usize,
}

type OffsetEvent<'a> = (Event<'a>, Range<usize>);

/// Consume a parser subtree after its Start event, leaving the matching End out.
fn children<'a>(events: &mut impl Iterator<Item = OffsetEvent<'a>>) -> Vec<OffsetEvent<'a>> {
    let mut depth = 0;
    events
        .take_while(|(event, _)| {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) if depth == 0 => return false,
                Event::End(_) => depth -= 1,
                _ => {}
            }
            true
        })
        .collect()
}

impl Document {
    pub(crate) fn parse(source: &str) -> Self {
        let flags = Options::ENABLE_TABLES | Options::ENABLE_YAML_STYLE_METADATA_BLOCKS;
        let mut document = Self {
            len: source.len(),
            ..Self::default()
        };
        let mut events = Parser::new_ext(source, flags).into_offset_iter();
        while let Some((Event::Start(tag), span)) =
            events.find(|(event, _)| matches!(event, Event::Start(_)))
        {
            let body = children(&mut events);
            document
                .links
                .extend(body.iter().filter_map(|(event, _)| match event {
                    Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.to_string()),
                    _ => None,
                }));
            document
                .code
                .extend(body.iter().filter_map(|(event, _)| match event {
                    Event::Code(code) => Some(code.to_string()),
                    _ => None,
                }));
            match tag {
                Tag::Heading { level, .. } => {
                    document
                        .headings
                        .push((level, Inline::read(body).text, span));
                }
                Tag::Table(_) => {
                    let mut table = Table {
                        span,
                        columns: Vec::new(),
                        rows: Vec::new(),
                    };
                    let mut rows = body.into_iter();
                    while let Some((Event::Start(kind @ (Tag::TableHead | Tag::TableRow)), span)) =
                        rows.next()
                    {
                        let mut fields = children(&mut rows).into_iter();
                        let mut cells = Vec::new();
                        while let Some((Event::Start(Tag::TableCell), _)) = fields.next() {
                            cells.push(Inline::read(children(&mut fields)));
                        }
                        if matches!(kind, Tag::TableHead) {
                            table.columns = cells;
                        } else {
                            table.rows.push(Row { span, cells });
                        }
                    }
                    document.tables.push(table);
                }
                Tag::List(None) => {
                    let mut items = body.into_iter();
                    while let Some((Event::Start(Tag::Item), span)) = items.next() {
                        document.items.push(ListItem {
                            span,
                            inline: Inline::read(children(&mut items)),
                        });
                    }
                }
                _ => {}
            }
        }
        document
    }

    pub(crate) fn links_to(&self, target: &str) -> bool {
        self.links
            .iter()
            .any(|link| link.split('#').next() == Some(target))
    }

    pub(crate) fn section(&self, Section(level, title): Section) -> Result<(usize, Range<usize>)> {
        let mut found = self
            .headings
            .iter()
            .enumerate()
            .filter(|(_, (l, text, _))| *l == level && text.trim() == title);
        let (index, (_, _, heading)) = found
            .next()
            .with_context(|| format!("Missing Markdown section '{title}'"))?;
        anyhow::ensure!(
            found.next().is_none(),
            "Ambiguous Markdown section '{title}'"
        );
        let end = self.headings[index + 1..]
            .iter()
            .find(|(next, _, _)| *next <= level)
            .map_or(self.len, |(_, _, span)| span.start);
        Ok((heading.end, heading.end..end))
    }

    pub(crate) fn table(&self, section: Section, columns: &[&str]) -> Result<&Table> {
        let (_, range) = self.section(section)?;
        let mut found = self
            .tables
            .iter()
            .filter(|table| range.contains(&table.span.start) && table.has_columns(columns));
        let table = found
            .next()
            .with_context(|| format!("Missing Markdown table in '{}'", section.1))?;
        anyhow::ensure!(
            found.next().is_none(),
            "Ambiguous Markdown table in '{}'",
            section.1
        );
        Ok(table)
    }
}
