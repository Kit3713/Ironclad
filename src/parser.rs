use pest::Parser;
use pest_derive::Parser;

use crate::ast::*;
use crate::errors::{IroncladError, Result};

#[derive(Parser)]
#[grammar = "storage.pest"]
pub struct StorageParser;

/// Parse an Ironclad storage source string into an AST
pub fn parse_storage(input: &str) -> Result<StorageFile> {
    let pairs = StorageParser::parse(Rule::file, input).map_err(|e| IroncladError::ParseError {
        message: e.to_string(),
        span: None,
    })?;

    let mut declarations = Vec::new();
    let mut selinux = None;
    for pair in pairs {
        if pair.as_rule() == Rule::file {
            for inner in pair.into_inner() {
                match inner.as_rule() {
                    Rule::top_level_decl => {
                        let child = inner.into_inner().next().unwrap();
                        match child.as_rule() {
                            Rule::storage_decl => {
                                let decl = parse_storage_decl(child, input)?;
                                declarations.push(decl);
                            }
                            Rule::selinux_block => {
                                selinux = Some(parse_selinux_block(child, input)?);
                            }
                            _ => {}
                        }
                    }
                    Rule::EOI => {}
                    _ => {}
                }
            }
        }
    }

    Ok(StorageFile {
        declarations,
        selinux,
    })
}

fn make_span(pair: &pest::iterators::Pair<'_, Rule>, input: &str) -> Span {
    let pest_span = pair.as_span();
    let start = pest_span.start();
    let end = pest_span.end();
    let (line, col) = line_col(input, start);
    Span {
        start,
        end,
        line,
        col,
    }
}

fn line_col(input: &str, pos: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in input.char_indices() {
        if i >= pos {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn parse_storage_decl(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<StorageDecl> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::disk_block => Ok(StorageDecl::Disk(parse_disk_block(inner, input)?)),
        Rule::mdraid_block => Ok(StorageDecl::MdRaid(parse_mdraid_block(inner, input)?)),
        Rule::zpool_block => Ok(StorageDecl::Zpool(parse_zpool_block(inner, input)?)),
        Rule::stratis_block => Ok(StorageDecl::Stratis(parse_stratis_block(inner, input)?)),
        Rule::multipath_block => Ok(StorageDecl::Multipath(parse_multipath_block(inner, input)?)),
        Rule::iscsi_block => Ok(StorageDecl::Iscsi(parse_iscsi_block(inner, input)?)),
        Rule::nfs_block => Ok(StorageDecl::Nfs(parse_nfs_block(inner, input)?)),
        Rule::tmpfs_block => Ok(StorageDecl::Tmpfs(parse_tmpfs_block(inner, input)?)),
        _ => Err(IroncladError::ParseError {
            message: format!("unexpected rule: {:?}", inner.as_rule()),
            span: Some(make_span(&inner, input)),
        }),
    }
}

// ─── Disk ────────────────────────────────────────────────────

fn parse_disk_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<DiskBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let device = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut children = Vec::new();

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::partition_child => {
                let child_inner = item.into_inner().next().unwrap();
                children.push(parse_partition_child(child_inner, input)?);
            }
            _ => {}
        }
    }

    Ok(DiskBlock {
        device,
        properties,
        children,
        span,
    })
}

fn parse_partition_child(
    pair: pest::iterators::Pair<'_, Rule>,
    input: &str,
) -> Result<PartitionChild> {
    match pair.as_rule() {
        Rule::fs_block => Ok(PartitionChild::Filesystem(Box::new(parse_fs_block(
            pair, input,
        )?))),
        Rule::luks_block => Ok(PartitionChild::Luks(parse_luks_block(pair, input)?)),
        Rule::integrity_block => Ok(PartitionChild::Integrity(parse_integrity_block(
            pair, input,
        )?)),
        Rule::lvm_block => Ok(PartitionChild::Lvm(parse_lvm_block(pair, input)?)),
        Rule::raw_block => Ok(PartitionChild::Raw(parse_raw_block(pair, input)?)),
        Rule::swap_block => Ok(PartitionChild::Swap(parse_swap_block(pair, input)?)),
        _ => Err(IroncladError::ParseError {
            message: format!("unexpected partition child: {:?}", pair.as_rule()),
            span: Some(make_span(&pair, input)),
        }),
    }
}

// ─── mdraid ──────────────────────────────────────────────────

fn parse_mdraid_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<MdRaidBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut children = Vec::new();

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::mdraid_child => {
                let child_inner = item.into_inner().next().unwrap();
                children.push(parse_partition_child(child_inner, input)?);
            }
            _ => {}
        }
    }

    Ok(MdRaidBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── Filesystem ──────────────────────────────────────────────

fn parse_fs_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<FsBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let fs_kw = inner.next().unwrap();
    let fs_type = match fs_kw.as_str() {
        "ext4" => FsType::Ext4,
        "xfs" => FsType::Xfs,
        "btrfs" => FsType::Btrfs,
        "fat32" => FsType::Fat32,
        "ntfs" => FsType::Ntfs,
        other => {
            return Err(IroncladError::ParseError {
                message: format!("unknown filesystem type: {other}"),
                span: Some(make_span(&fs_kw, input)),
            });
        }
    };

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut subvolumes = Vec::new();
    let mut mount_block = None;

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::subvol_block => subvolumes.push(parse_subvol_block(item, input)?),
            Rule::mount_block_ext => mount_block = Some(parse_mount_block_ext(item, input)?),
            _ => {}
        }
    }

    Ok(FsBlock {
        fs_type,
        name,
        properties,
        subvolumes,
        mount_block,
        span,
    })
}

// ─── Subvolume ───────────────────────────────────────────────

fn parse_subvol_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<SubvolBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut mount_block = None;

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::mount_block_ext => mount_block = Some(parse_mount_block_ext(item, input)?),
            _ => {}
        }
    }

    Ok(SubvolBlock {
        name,
        properties,
        mount_block,
        span,
    })
}

// ─── LUKS ────────────────────────────────────────────────────

fn parse_luks_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<LuksBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let kw = inner.next().unwrap();
    let version = match kw.as_str() {
        "luks2" => LuksVersion::Luks2,
        "luks1" => LuksVersion::Luks1,
        _ => LuksVersion::Luks2,
    };

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut children = Vec::new();

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::luks_child => {
                let child_inner = item.into_inner().next().unwrap();
                match child_inner.as_rule() {
                    Rule::fs_block => children.push(LuksChild::Filesystem(Box::new(
                        parse_fs_block(child_inner, input)?,
                    ))),
                    Rule::lvm_block => {
                        children.push(LuksChild::Lvm(parse_lvm_block(child_inner, input)?))
                    }
                    Rule::swap_block => {
                        children.push(LuksChild::Swap(parse_swap_block(child_inner, input)?))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(LuksBlock {
        version,
        name,
        properties,
        children,
        span,
    })
}

// ─── LVM ─────────────────────────────────────────────────────

fn parse_lvm_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<LvmBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut children = Vec::new();

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::lvm_child => {
                let child_inner = item.into_inner().next().unwrap();
                match child_inner.as_rule() {
                    Rule::fs_block => children.push(LvmChild::Filesystem(Box::new(
                        parse_fs_block(child_inner, input)?,
                    ))),
                    Rule::swap_block => {
                        children.push(LvmChild::Swap(parse_swap_block(child_inner, input)?))
                    }
                    Rule::thin_block => {
                        children.push(LvmChild::Thin(parse_thin_block(child_inner, input)?))
                    }
                    Rule::vdo_block => {
                        children.push(LvmChild::Vdo(parse_vdo_block(child_inner, input)?))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(LvmBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── Thin Pool ───────────────────────────────────────────────

fn parse_thin_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<ThinBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    let mut children = Vec::new();

    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::thin_child => {
                let child_inner = item.into_inner().next().unwrap();
                match child_inner.as_rule() {
                    Rule::fs_block => children.push(ThinChild::Filesystem(Box::new(
                        parse_fs_block(child_inner, input)?,
                    ))),
                    Rule::swap_block => {
                        children.push(ThinChild::Swap(parse_swap_block(child_inner, input)?))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    Ok(ThinBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── Swap ────────────────────────────────────────────────────

fn parse_swap_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<SwapBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    for item in body.into_inner() {
        if item.as_rule() == Rule::property {
            properties.push(parse_property(item, input)?);
        }
    }

    Ok(SwapBlock {
        name,
        properties,
        span,
    })
}

// ─── Raw ─────────────────────────────────────────────────────

fn parse_raw_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<RawBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();

    let mut properties = Vec::new();
    for item in body.into_inner() {
        if item.as_rule() == Rule::property {
            properties.push(parse_property(item, input)?);
        }
    }

    Ok(RawBlock {
        name,
        properties,
        span,
    })
}

// ─── Mount Block (Extended) ──────────────────────────────────

fn parse_mount_block_ext(
    pair: pest::iterators::Pair<'_, Rule>,
    input: &str,
) -> Result<MountBlockExt> {
    let span = make_span(&pair, input);

    let mut mount = MountBlockExt {
        target: None,
        options: Vec::new(),
        automount: None,
        timeout: None,
        requires: Vec::new(),
        before: Vec::new(),
        context: None,
        fscontext: None,
        defcontext: None,
        rootcontext: None,
        span,
    };

    for prop in pair.into_inner() {
        if prop.as_rule() != Rule::mount_property {
            continue;
        }
        let inner = prop.into_inner().next().unwrap();
        match inner.as_rule() {
            Rule::mount_target_prop => {
                let val = inner.into_inner().next().unwrap();
                mount.target = Some(val.as_str().to_string());
            }
            Rule::mount_options_prop => {
                let arr = inner.into_inner().next().unwrap();
                mount.options = parse_string_array(arr);
            }
            Rule::mount_automount_prop => {
                let val = inner.into_inner().next().unwrap();
                mount.automount = Some(val.as_str() == "true");
            }
            Rule::mount_timeout_prop => {
                let val = inner.into_inner().next().unwrap();
                mount.timeout = val.as_str().parse().ok();
            }
            Rule::mount_requires_prop => {
                let arr = inner.into_inner().next().unwrap();
                mount.requires = parse_string_array(arr);
            }
            Rule::mount_before_prop => {
                let arr = inner.into_inner().next().unwrap();
                mount.before = parse_string_array(arr);
            }
            Rule::mount_context_prop => {
                let ctx = inner.into_inner().next().unwrap();
                mount.context = Some(parse_selinux_context(ctx)?);
            }
            Rule::mount_fscontext_prop => {
                let ctx = inner.into_inner().next().unwrap();
                mount.fscontext = Some(parse_selinux_context(ctx)?);
            }
            Rule::mount_defcontext_prop => {
                let ctx = inner.into_inner().next().unwrap();
                mount.defcontext = Some(parse_selinux_context(ctx)?);
            }
            Rule::mount_rootcontext_prop => {
                let ctx = inner.into_inner().next().unwrap();
                mount.rootcontext = Some(parse_selinux_context(ctx)?);
            }
            _ => {}
        }
    }

    Ok(mount)
}

fn parse_string_array(pair: pest::iterators::Pair<'_, Rule>) -> Vec<String> {
    pair.into_inner()
        .map(|item| {
            let s = item.as_str();
            // Strip surrounding quotes if present
            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                s[1..s.len() - 1].to_string()
            } else {
                s.to_string()
            }
        })
        .collect()
}

// ─── Properties ──────────────────────────────────────────────

fn parse_property(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<Property> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();

    let key = inner.next().unwrap().as_str().to_string();
    let value_pair = inner.next().unwrap();
    let value = parse_value(value_pair, input)?;

    // A bare path like `/exports/data` matches mount_expr in the grammar.
    // If the key isn't "mount" and the mount has no options/context, demote to Path.
    let value = match value {
        Value::Mount(ref m) if key != "mount" && m.options.is_empty() && m.context.is_none() => {
            Value::Path(m.target.clone())
        }
        other => other,
    };

    Ok(Property { key, value, span })
}

fn parse_value(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<Value> {
    // The `value` rule wraps one of several alternatives
    let inner = if pair.as_rule() == Rule::value {
        pair.into_inner().next().unwrap()
    } else {
        pair
    };

    match inner.as_rule() {
        Rule::mount_expr => {
            let mount = parse_mount_expr(inner)?;
            Ok(Value::Mount(mount))
        }
        Rule::array_value => {
            let items: Vec<Value> = inner
                .into_inner()
                .map(|item| parse_array_item(item, input))
                .collect::<Result<Vec<_>>>()?;
            Ok(Value::Array(items))
        }
        Rule::size_value => {
            let s = inner.as_str();
            let (amount, unit) = parse_size_str(s)?;
            Ok(Value::Size(SizeValue { amount, unit }))
        }
        Rule::percentage => {
            let s = inner.as_str();
            let num: u64 =
                s.trim_end_matches('%')
                    .parse()
                    .map_err(|_| IroncladError::ParseError {
                        message: format!("invalid percentage: {s}"),
                        span: Some(make_span(&inner, input)),
                    })?;
            Ok(Value::Percentage(num))
        }
        Rule::remaining_kw => Ok(Value::Remaining),
        Rule::boolean => Ok(Value::Boolean(inner.as_str() == "true")),
        Rule::integer => {
            let n: i64 = inner
                .as_str()
                .parse()
                .map_err(|_| IroncladError::ParseError {
                    message: format!("invalid integer: {}", inner.as_str()),
                    span: Some(make_span(&inner, input)),
                })?;
            Ok(Value::Integer(n))
        }
        Rule::string_literal => {
            let s = inner.as_str();
            // Strip quotes
            let unquoted = &s[1..s.len() - 1];
            Ok(Value::String(unquoted.to_string()))
        }
        Rule::device_path => Ok(Value::DevicePath(inner.as_str().to_string())),
        Rule::url_string => Ok(Value::Url(inner.as_str().to_string())),
        Rule::ident_value => {
            let s = inner.as_str();
            // If it's purely numeric, treat as integer
            if let Ok(n) = s.parse::<i64>() {
                Ok(Value::Integer(n))
            } else {
                Ok(Value::Ident(s.to_string()))
            }
        }
        _ => Err(IroncladError::ParseError {
            message: format!(
                "unexpected value rule: {:?} = {:?}",
                inner.as_rule(),
                inner.as_str()
            ),
            span: Some(make_span(&inner, input)),
        }),
    }
}

fn parse_array_item(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<Value> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::size_value => {
            let s = inner.as_str();
            let (amount, unit) = parse_size_str(s)?;
            Ok(Value::Size(SizeValue { amount, unit }))
        }
        Rule::string_literal => {
            let s = inner.as_str();
            Ok(Value::String(s[1..s.len() - 1].to_string()))
        }
        Rule::device_path => Ok(Value::DevicePath(inner.as_str().to_string())),
        Rule::boolean => Ok(Value::Boolean(inner.as_str() == "true")),
        Rule::integer => {
            let n: i64 = inner
                .as_str()
                .parse()
                .map_err(|_| IroncladError::ParseError {
                    message: format!("invalid integer: {}", inner.as_str()),
                    span: Some(make_span(&inner, input)),
                })?;
            Ok(Value::Integer(n))
        }
        Rule::ident_value => {
            let s = inner.as_str();
            if let Ok(n) = s.parse::<i64>() {
                Ok(Value::Integer(n))
            } else {
                Ok(Value::Ident(s.to_string()))
            }
        }
        _ => Err(IroncladError::ParseError {
            message: format!("unexpected array item: {:?}", inner.as_rule()),
            span: Some(make_span(&inner, input)),
        }),
    }
}

fn parse_size_str(s: &str) -> Result<(u64, SizeUnit)> {
    let unit_start = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    let amount: u64 = s[..unit_start]
        .parse()
        .map_err(|_| IroncladError::ParseError {
            message: format!("invalid size number: {s}"),
            span: None,
        })?;
    let unit_str = &s[unit_start..];
    let unit = match unit_str {
        "B" => SizeUnit::B,
        "K" | "KB" => SizeUnit::K,
        "M" | "MB" => SizeUnit::M,
        "G" | "GB" => SizeUnit::G,
        "T" | "TB" => SizeUnit::T,
        _ => {
            return Err(IroncladError::ParseError {
                message: format!("unknown size unit: {unit_str}"),
                span: None,
            });
        }
    };
    Ok((amount, unit))
}

// ─── Integrity ──────────────────────────────────────────────

fn parse_integrity_block(
    pair: pest::iterators::Pair<'_, Rule>,
    input: &str,
) -> Result<IntegrityBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut children = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::integrity_child => {
                let c = item.into_inner().next().unwrap();
                match c.as_rule() {
                    Rule::fs_block => children.push(IntegrityChild::Filesystem(Box::new(
                        parse_fs_block(c, input)?,
                    ))),
                    Rule::lvm_block => {
                        children.push(IntegrityChild::Lvm(parse_lvm_block(c, input)?))
                    }
                    Rule::swap_block => {
                        children.push(IntegrityChild::Swap(parse_swap_block(c, input)?))
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(IntegrityBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── VDO ─────────────────────────────────────────────────────

fn parse_vdo_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<VdoBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut children = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::vdo_child => {
                let c = item.into_inner().next().unwrap();
                match c.as_rule() {
                    Rule::fs_block => {
                        children.push(VdoChild::Filesystem(Box::new(parse_fs_block(c, input)?)))
                    }
                    Rule::swap_block => children.push(VdoChild::Swap(parse_swap_block(c, input)?)),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    Ok(VdoBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── ZFS Pool ────────────────────────────────────────────────

fn parse_zpool_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<ZpoolBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut vdevs = Vec::new();
    let mut datasets = Vec::new();
    let mut zvols = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::vdev_block => vdevs.push(parse_vdev_block(item, input)?),
            Rule::dataset_block => datasets.push(parse_dataset_block(item, input)?),
            Rule::zvol_block => zvols.push(parse_zvol_block(item, input)?),
            _ => {}
        }
    }
    Ok(ZpoolBlock {
        name,
        properties,
        vdevs,
        datasets,
        zvols,
        span,
    })
}

fn parse_vdev_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<VdevBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let mut properties = Vec::new();
    for item in inner {
        if item.as_rule() == Rule::property {
            properties.push(parse_property(item, input)?);
        }
    }
    Ok(VdevBlock {
        name,
        properties,
        span,
    })
}

fn parse_dataset_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<DatasetBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut children = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::dataset_block => children.push(parse_dataset_block(item, input)?),
            _ => {}
        }
    }
    Ok(DatasetBlock {
        name,
        properties,
        children,
        span,
    })
}

fn parse_zvol_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<ZvolBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut children = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::swap_block => children.push(ZvolChild::Swap(parse_swap_block(item, input)?)),
            Rule::fs_block => children.push(ZvolChild::Filesystem(Box::new(parse_fs_block(
                item, input,
            )?))),
            Rule::luks_block => children.push(ZvolChild::Luks(parse_luks_block(item, input)?)),
            _ => {}
        }
    }
    Ok(ZvolBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── Stratis ─────────────────────────────────────────────────

fn parse_stratis_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<StratisBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut filesystems = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::stratis_fs_block => filesystems.push(parse_stratis_fs(item, input)?),
            _ => {}
        }
    }
    Ok(StratisBlock {
        name,
        properties,
        filesystems,
        span,
    })
}

fn parse_stratis_fs(
    pair: pest::iterators::Pair<'_, Rule>,
    input: &str,
) -> Result<StratisFilesystem> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut mount_block = None;
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::mount_block_ext => mount_block = Some(parse_mount_block_ext(item, input)?),
            _ => {}
        }
    }
    Ok(StratisFilesystem {
        name,
        properties,
        mount_block,
        span,
    })
}

// ─── Multipath ───────────────────────────────────────────────

fn parse_multipath_block(
    pair: pest::iterators::Pair<'_, Rule>,
    input: &str,
) -> Result<MultipathBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut paths = Vec::new();
    let mut children = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::path_block => paths.push(parse_path_block(item, input)?),
            Rule::multipath_child => {
                let c = item.into_inner().next().unwrap();
                children.push(parse_partition_child(c, input)?);
            }
            _ => {}
        }
    }
    Ok(MultipathBlock {
        name,
        properties,
        paths,
        children,
        span,
    })
}

fn parse_path_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<PathBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let device = inner.next().unwrap().as_str().to_string();
    let mut properties = Vec::new();
    for item in inner {
        if item.as_rule() == Rule::property {
            properties.push(parse_property(item, input)?);
        }
    }
    Ok(PathBlock {
        device,
        properties,
        span,
    })
}

// ─── iSCSI ───────────────────────────────────────────────────

fn parse_iscsi_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<IscsiBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut children = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::iscsi_child => {
                let c = item.into_inner().next().unwrap();
                children.push(parse_partition_child(c, input)?);
            }
            _ => {}
        }
    }
    Ok(IscsiBlock {
        name,
        properties,
        children,
        span,
    })
}

// ─── NFS ─────────────────────────────────────────────────────

fn parse_nfs_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<NfsBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut mount_block = None;
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::mount_block_ext => mount_block = Some(parse_mount_block_ext(item, input)?),
            _ => {}
        }
    }
    Ok(NfsBlock {
        name,
        properties,
        mount_block,
        span,
    })
}

// ─── tmpfs ───────────────────────────────────────────────────

fn parse_tmpfs_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<TmpfsBlock> {
    let span = make_span(&pair, input);
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let body = inner.next().unwrap();
    let mut properties = Vec::new();
    let mut mount_block = None;
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::mount_block_ext => mount_block = Some(parse_mount_block_ext(item, input)?),
            _ => {}
        }
    }
    Ok(TmpfsBlock {
        name,
        properties,
        mount_block,
        span,
    })
}

// ─── SELinux System Block ────────────────────────────────────

fn parse_selinux_block(pair: pest::iterators::Pair<'_, Rule>, input: &str) -> Result<SelinuxBlock> {
    let span = make_span(&pair, input);
    let body = pair.into_inner().next().unwrap();
    let mut properties = Vec::new();
    let mut users = Vec::new();
    let mut roles = Vec::new();
    let mut booleans = Vec::new();
    for item in body.into_inner() {
        match item.as_rule() {
            Rule::property => properties.push(parse_property(item, input)?),
            Rule::selinux_user_block => {
                let s = make_span(&item, input);
                let mut inner = item.into_inner();
                let name = inner.next().unwrap().as_str().to_string();
                let mut props = Vec::new();
                for p in inner {
                    if p.as_rule() == Rule::property {
                        props.push(parse_property(p, input)?);
                    }
                }
                users.push(SelinuxUserDecl {
                    name,
                    properties: props,
                    span: s,
                });
            }
            Rule::selinux_role_block => {
                let s = make_span(&item, input);
                let mut inner = item.into_inner();
                let name = inner.next().unwrap().as_str().to_string();
                let mut props = Vec::new();
                for p in inner {
                    if p.as_rule() == Rule::property {
                        props.push(parse_property(p, input)?);
                    }
                }
                roles.push(SelinuxRoleDecl {
                    name,
                    properties: props,
                    span: s,
                });
            }
            Rule::selinux_booleans_block => {
                for p in item.into_inner() {
                    if p.as_rule() == Rule::property {
                        booleans.push(parse_property(p, input)?);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(SelinuxBlock {
        properties,
        users,
        roles,
        booleans,
        span,
    })
}

// ─── Mount Expression (inline) ───────────────────────────────

fn parse_mount_expr(pair: pest::iterators::Pair<'_, Rule>) -> Result<MountExpr> {
    let mut target = String::new();
    let mut options = Vec::new();
    let mut context = None;

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::path_value => {
                target = inner.as_str().to_string();
            }
            Rule::mount_options_bracket => {
                for opt in inner.into_inner() {
                    if opt.as_rule() == Rule::mount_option {
                        options.push(opt.as_str().to_string());
                    }
                }
            }
            Rule::mount_inline_context => {
                let ctx_pair = inner.into_inner().next().unwrap();
                context = Some(parse_selinux_context(ctx_pair)?);
            }
            _ => {}
        }
    }

    Ok(MountExpr {
        target,
        options,
        context,
    })
}

// ─── SELinux Context ─────────────────────────────────────────

fn parse_selinux_context(pair: pest::iterators::Pair<'_, Rule>) -> Result<SelinuxContext> {
    let raw = pair.as_str().to_string();

    // Parse the four colon-separated fields: user:role:type:range
    let parts: Vec<&str> = raw.splitn(4, ':').collect();
    if parts.len() < 4 {
        return Err(IroncladError::ParseError {
            message: format!(
                "SELinux context must have exactly 4 colon-separated fields (user:role:type:range), got {}: {raw}",
                parts.len()
            ),
            span: None,
        });
    }

    let user = parts[0].to_string();
    let role = parts[1].to_string();
    let typ = parts[2].to_string();
    let range_str = parts[3];

    let range = parse_mls_range(range_str)?;

    Ok(SelinuxContext {
        user,
        role,
        typ,
        range,
        raw,
    })
}

fn parse_mls_range(s: &str) -> Result<MlsRange> {
    // Format: sensitivity(-sensitivity)?(:category_set)?
    let (sens_part, cats) = if let Some(colon_pos) = s.find(':') {
        (&s[..colon_pos], Some(s[colon_pos + 1..].to_string()))
    } else {
        (s, None)
    };

    let (low, high) = if let Some(dash_pos) = sens_part.find('-') {
        let low_str = &sens_part[..dash_pos];
        let high_str = &sens_part[dash_pos + 1..];
        (
            parse_sensitivity(low_str)?,
            Some(parse_sensitivity(high_str)?),
        )
    } else {
        (parse_sensitivity(sens_part)?, None)
    };

    Ok(MlsRange {
        low,
        high,
        categories: cats,
    })
}

fn parse_sensitivity(s: &str) -> Result<Sensitivity> {
    if !s.starts_with('s') {
        return Err(IroncladError::ParseError {
            message: format!("sensitivity must start with 's': {s}"),
            span: None,
        });
    }
    let level: u32 = s[1..].parse().map_err(|_| IroncladError::ParseError {
        message: format!("invalid sensitivity level: {s}"),
        span: None,
    })?;
    Ok(Sensitivity { level })
}
