use crate::FileList;
use crate::Opt;
use chrono::DateTime;
use std::collections::BTreeMap;
use std::fs::*;

pub fn file_list(opt: &Opt) -> anyhow::Result<FileList> {
    let mut list = BTreeMap::new();

    for entry in read_dir(&opt.path)? {
        let entry = entry?;

        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => continue, // ignore names that aren't valid utf-8
        };

        let date = DateTime::parse_from_rfc3339(&name)?;
        list.insert(date, name);
    }

    Ok(list)
}
