use crate::error::ParseError;
use crate::matcher::glob_match;
use clang::{Entity, EntityKind, EntityVisitResult, Type};
use colored::Colorize;
use std::fmt::Write;

#[derive(Debug)]
pub struct Field<'a> {
    entity: Entity<'a>,
    #[allow(unused)]
    semantic_parent: Entity<'a>,
    name: String,
    type_: Type<'a>,
    offset: usize,
    size: usize,
    alignment: usize,
}

impl<'a> Field<'a> {
    #[must_use] pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }
    #[allow(unused)]
    #[must_use] pub const fn get_semantic_parent(&self) -> &Entity<'a> {
        &self.semantic_parent
    }
    #[must_use] pub fn get_name(&self) -> &str {
        &self.name
    }
    #[must_use] pub const fn get_type(&self) -> &Type<'a> {
        &self.type_
    }
    #[must_use] pub fn get_canonical_type(&self) -> Type<'a> {
        self.type_.get_canonical_type()
    }
    #[must_use] pub const fn get_offset(&self) -> usize {
        self.offset
    }
    #[must_use] pub const fn get_offset_bytes(&self) -> usize {
        self.offset / 8
    }
    #[must_use] pub const fn get_size(&self) -> usize {
        self.size
    }
    #[must_use] pub const fn get_alignment(&self) -> usize {
        self.alignment
    }

    fn get_underlying_type(&self) -> Type<'a> {
        let canonical = self.get_canonical_type();
        // Follow pointer types to their pointee
        if let Some(pointee) = canonical.get_pointee_type() {
            pointee.get_canonical_type()
        } else {
            canonical
        }
    }

    #[must_use] pub fn has_children(&self) -> bool {
        Some(self.get_underlying_type())
            .and_then(|t| t.get_fields())
            .is_some_and(|fields| !fields.is_empty())
    }

    #[must_use] pub fn get_child_fields(&self) -> Vec<Self> {
        Some(self.get_underlying_type())
            .and_then(|t| t.get_declaration())
            .map(|decl| collect_fields(&decl))
            .unwrap_or_default()
    }
}

impl<'a> TryFrom<(Entity<'a>, &Entity<'a>)> for Field<'a> {
    type Error = ParseError;

    fn try_from((entity, parent): (Entity<'a>, &Entity<'a>)) -> Result<Self, Self::Error> {
        if entity.get_kind() != EntityKind::FieldDecl {
            return Err(ParseError::NotFieldDecl);
        }

        let type_ = entity.get_type().ok_or(ParseError::NoType)?;
        let name = entity.get_name().ok_or(ParseError::NoName)?;
        let semantic_parent = entity.get_semantic_parent().ok_or(ParseError::NoType)?;

        let parent_type = parent.get_type().ok_or(ParseError::NoType)?;
        let offset = parent_type
            .get_offsetof(&name)
            .map_err(|_| ParseError::NoOffset)?;
        let size = type_.get_sizeof().map_err(|_| ParseError::NoSize)?;
        let alignment = type_.get_alignof().map_err(|_| ParseError::NoAlignment)?;

        Ok(Self {
            entity,
            semantic_parent,
            name,
            type_,
            offset,
            size,
            alignment,
        })
    }
}

#[derive(Debug)]
pub struct Struct<'a> {
    entity: Entity<'a>,
    name: String,
    fields: Vec<Field<'a>>,
}

impl<'a> Struct<'a> {
    #[must_use] pub const fn get_entity(&self) -> &Entity<'a> {
        &self.entity
    }
    #[must_use] pub fn get_name(&self) -> &str {
        &self.name
    }
    #[must_use] pub fn get_fields(&self) -> &[Field<'a>] {
        &self.fields
    }

    fn get_source_file(&self) -> Option<String> {
        self.entity
            .get_location()
            .and_then(|loc| loc.get_file_location().file)
            .map(|f| f.get_path())
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
    }

    #[must_use] pub fn display(&self, depth: usize, field_filter: Option<&str>) -> String {
        let mut out = String::new();

        // Header
        let file_info = self
            .get_source_file()
            .map(|f| format!(" {}", f.dimmed()))
            .unwrap_or_default();
        let _ = writeln!(out, "{}{}", self.name.cyan().bold(), file_info);

        write_fields(&mut out, &self.fields, depth, 0, "", field_filter);

        // Footer
        if let Some(Ok(size)) = self.entity.get_type().map(|t| t.get_sizeof()) {
            let _ = writeln!(out, "{}", format!("╰─ {size} bytes").dimmed());
        }

        out
    }
}

fn write_fields(
    out: &mut String,
    fields: &[Field],
    max_depth: usize,
    current_depth: usize,
    prefix: &str,
    field_filter: Option<&str>,
) {
    let filtered: Vec<_> = fields
        .iter()
        .filter(|f| field_filter.is_none_or(|pat| glob_match(f.get_name(), pat, false)))
        .collect();

    let count = filtered.len();

    for (i, field) in filtered.iter().enumerate() {
        let is_last = i == count - 1;
        let connector = if is_last { "╰─" } else { "├─" };
        let child_prefix = if is_last { "   " } else { "│  " };

        let offset = format!("+{:#05x}", field.get_offset_bytes());
        let size = format!("{:>3}", field.get_size());
        let name = field.get_name();
        let type_name = field.get_type().get_display_name();

        let name_styled = if field_filter.is_some() {
            name.white().bold().underline()
        } else {
            name.white().bold()
        };

        let _ = writeln!(
            out,
            "{}{} {} {} {}  {}",
            prefix,
            connector.dimmed(),
            offset.yellow(),
            format!("[{size}]").green(),
            name_styled,
            type_name.cyan()
        );

        if current_depth < max_depth && field.has_children() {
            let child_fields = field.get_child_fields();
            if !child_fields.is_empty() {
                let new_prefix = format!("{prefix}{child_prefix}");
                write_fields(
                    out,
                    &child_fields,
                    max_depth,
                    current_depth + 1,
                    &new_prefix,
                    None,
                );
            }
        }
    }
}

impl<'a> TryFrom<Entity<'a>> for Struct<'a> {
    type Error = ParseError;

    fn try_from(entity: Entity<'a>) -> Result<Self, Self::Error> {
        if !matches!(
            entity.get_kind(),
            EntityKind::ClassDecl | EntityKind::StructDecl
        ) {
            return Err(ParseError::NotStructOrClass);
        }

        let name = entity.get_name().ok_or(ParseError::NoName)?;
        let fields = collect_fields(&entity);

        Ok(Self {
            entity,
            name,
            fields,
        })
    }
}

fn collect_fields<'a>(entity: &Entity<'a>) -> Vec<Field<'a>> {
    let mut fields = Vec::new();
    entity.visit_children(|child, _| {
        if child.get_kind() == EntityKind::FieldDecl {
            if let Ok(field) = Field::try_from((child, entity)) {
                fields.push(field);
            }
        }
        EntityVisitResult::Continue
    });
    fields
}
